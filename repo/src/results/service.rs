use std::collections::HashMap;

use chrono::Utc;
use sea_orm::sea_query::{Expr, Func};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait,
    FromQueryResult, QueryFilter, QueryOrder, QuerySelect, TransactionError, TransactionTrait,
};

use crate::audit;
use crate::entity::enums::{CorrectionStatus, ResultUnit, ReviewDecision, ReviewedState};
use crate::entity::{
    event as event_entity,
    result::{self as result_entity, ActiveModel as ResultActiveModel},
    result_arbitration::{self as arbitration_entity, ActiveModel as ArbitrationActiveModel},
    result_correction::{self as correction_entity, ActiveModel as CorrectionActiveModel},
    result_review::{self as review_entity, ActiveModel as ReviewActiveModel},
};
use crate::errors::{AppError, AppResult};

use super::{
    ArbitrateRequest, ArbitrationResponse, CorrectionResponse, RankEntry, RankingsQuery,
    RankingsResponse, RequestCorrectionRequest, ResolveCorrectionRequest, ResultResponse,
    ReviewResponse, SubmitResultRequest, SubmitReviewRequest,
};

// ── Unit helpers ──────────────────────────────────────────────────────────────

fn parse_unit(s: &str) -> AppResult<ResultUnit> {
    match s {
        "milliseconds" => Ok(ResultUnit::Milliseconds),
        "feet" => Ok(ResultUnit::Feet),
        "inches" => Ok(ResultUnit::Inches),
        "seconds" => Ok(ResultUnit::Seconds),
        "meters" => Ok(ResultUnit::Meters),
        "kilometers" => Ok(ResultUnit::Kilometers),
        "kilograms" => Ok(ResultUnit::Kilograms),
        "points" => Ok(ResultUnit::Points),
        other => Err(AppError::BadRequest(format!(
            "Unknown unit '{other}'. Valid values: milliseconds, feet, inches, \
             seconds, meters, kilometers, kilograms, points"
        ))),
    }
}

fn unit_str(u: &ResultUnit) -> &'static str {
    match u {
        ResultUnit::Milliseconds => "milliseconds",
        ResultUnit::Feet => "feet",
        ResultUnit::Inches => "inches",
        ResultUnit::Seconds => "seconds",
        ResultUnit::Meters => "meters",
        ResultUnit::Kilometers => "kilometers",
        ResultUnit::Kilograms => "kilograms",
        ResultUnit::Points => "points",
    }
}

fn parse_review_decision(s: &str) -> AppResult<ReviewDecision> {
    match s {
        "approved" => Ok(ReviewDecision::Approved),
        "rejected" => Ok(ReviewDecision::Rejected),
        _ => Err(AppError::BadRequest(
            "decision must be 'approved' or 'rejected'".into(),
        )),
    }
}

fn decision_str(d: &ReviewDecision) -> &'static str {
    match d {
        ReviewDecision::Approved => "approved",
        ReviewDecision::Rejected => "rejected",
    }
}

fn reviewed_state_str(s: &ReviewedState) -> &'static str {
    match s {
        ReviewedState::Pending => "pending",
        ReviewedState::Approved => "approved",
        ReviewedState::Rejected => "rejected",
    }
}

fn correction_status_str(s: &CorrectionStatus) -> &'static str {
    match s {
        CorrectionStatus::Pending => "pending",
        CorrectionStatus::Approved => "approved",
        CorrectionStatus::Rejected => "rejected",
    }
}

/// Time units rank ascending (lower = faster); all others descending (higher = better).
fn is_ascending(unit: &ResultUnit) -> bool {
    matches!(unit, ResultUnit::Milliseconds | ResultUnit::Seconds)
}

fn tx_err(e: TransactionError<AppError>) -> AppError {
    match e {
        TransactionError::Transaction(app_err) => app_err,
        TransactionError::Connection(db_err) => AppError::Internal(db_err.to_string()),
    }
}

// ── Model → response conversions ──────────────────────────────────────────────

fn model_to_result_response(m: &result_entity::Model) -> ResultResponse {
    ResultResponse {
        id: m.id,
        event_id: m.event_id,
        participant_id: m.participant_id,
        attempt_no: m.attempt_no,
        value_numeric: m.value_numeric,
        unit: unit_str(&m.unit_enum).to_owned(),
        reviewed_state: reviewed_state_str(&m.reviewed_state).to_owned(),
        entered_by: m.entered_by,
        created_at: m.created_at.to_rfc3339(),
        updated_at: m.updated_at.to_rfc3339(),
    }
}

fn model_to_review_response(m: &review_entity::Model) -> ReviewResponse {
    ReviewResponse {
        id: m.id,
        result_id: m.result_id,
        referee_id: m.referee_id,
        decision: decision_str(&m.decision).to_owned(),
        comment: m.comment.clone(),
        reviewed_at: m.reviewed_at.to_rfc3339(),
        created_at: m.created_at.to_rfc3339(),
    }
}

fn model_to_arbitration_response(m: &arbitration_entity::Model) -> ArbitrationResponse {
    ArbitrationResponse {
        id: m.id,
        result_id: m.result_id,
        arbitrated_by: m.arbitrated_by,
        decision: decision_str(&m.decision).to_owned(),
        comment: m.comment.clone(),
        created_at: m.created_at.to_rfc3339(),
    }
}

fn model_to_correction_response(m: &correction_entity::Model) -> CorrectionResponse {
    CorrectionResponse {
        id: m.id,
        result_id: m.result_id,
        corrected_value: m.corrected_value,
        corrected_unit: unit_str(&m.corrected_unit).to_owned(),
        requested_by: m.requested_by,
        reason: m.reason.clone(),
        status: correction_status_str(&m.status).to_owned(),
        resolved_by: m.resolved_by,
        resolved_at: m.resolved_at.map(|t| t.to_rfc3339()),
        created_at: m.created_at.to_rfc3339(),
    }
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Load a result belonging to the given event, returning 404 on either miss.
async fn load_result_for_event(
    conn: &DatabaseConnection,
    event_id: i64,
    result_id: i64,
) -> AppResult<result_entity::Model> {
    let model = result_entity::Entity::find_by_id(result_id)
        .one(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Result {result_id} not found")))?;

    if model.event_id != event_id {
        return Err(AppError::NotFound(format!(
            "Result {result_id} does not belong to event {event_id}"
        )));
    }
    Ok(model)
}

/// Derive the new `reviewed_state` after a set of reviews is cast.
///
/// Rules:
/// - If any review is `rejected` → whole result is rejected.
/// - If all reviews are `approved` AND count ≥ required → approved.
/// - Otherwise stay pending (waiting for more reviews or arbitration).
fn derive_reviewed_state(
    reviews: &[review_entity::Model],
    is_championship_class: bool,
) -> ReviewedState {
    let required = if is_championship_class {
        2usize
    } else {
        1usize
    };
    let total = reviews.len();

    if total == 0 {
        return ReviewedState::Pending;
    }

    let rejected_count = reviews
        .iter()
        .filter(|r| r.decision == ReviewDecision::Rejected)
        .count();

    if rejected_count > 0 {
        return ReviewedState::Rejected;
    }

    // All are approved — check quorum.
    if total >= required {
        ReviewedState::Approved
    } else {
        ReviewedState::Pending
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Submit result
// ══════════════════════════════════════════════════════════════════════════════

pub async fn submit_result(
    conn: &DatabaseConnection,
    event_id: i64,
    entered_by: i64,
    req: SubmitResultRequest,
) -> AppResult<ResultResponse> {
    let unit = parse_unit(&req.unit)?;

    if !req.value_numeric.is_finite() || req.value_numeric < 0.0 {
        return Err(AppError::BadRequest(
            "value_numeric must be a non-negative finite number".into(),
        ));
    }

    // Auto-assign attempt_no if not provided.
    let attempt_no = match req.attempt_no {
        Some(n) if n >= 1 => n,
        Some(n) => {
            return Err(AppError::BadRequest(format!(
                "attempt_no must be >= 1, got {n}"
            )))
        }
        None => {
            #[derive(FromQueryResult)]
            struct MaxAttempt {
                max_no: Option<i32>,
            }

            let row = result_entity::Entity::find()
                .filter(result_entity::Column::EventId.eq(event_id))
                .filter(result_entity::Column::ParticipantId.eq(req.participant_id))
                .select_only()
                .column_as(
                    Func::max(Expr::col(result_entity::Column::AttemptNo)).into(),
                    "max_no",
                )
                .into_model::<MaxAttempt>()
                .one(conn)
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?;

            row.and_then(|r| r.max_no).unwrap_or(0) + 1
        }
    };

    let now = Utc::now();
    let model = ResultActiveModel {
        event_id: Set(event_id),
        participant_id: Set(req.participant_id),
        attempt_no: Set(attempt_no),
        value_numeric: Set(req.value_numeric),
        unit_enum: Set(unit),
        entered_by: Set(entered_by),
        reviewed_state: Set(ReviewedState::Pending),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };

    let inserted = model.insert(conn).await.map_err(|e| {
        let msg = e.to_string();
        if msg.contains("UNIQUE") {
            AppError::Conflict(format!(
                "Attempt {attempt_no} for participant {} in event {event_id} already exists",
                req.participant_id
            ))
        } else {
            AppError::Internal(msg)
        }
    })?;

    tracing::info!(
        result_id = inserted.id,
        event_id,
        participant_id = req.participant_id,
        attempt_no,
        "results.submitted"
    );
    let resp = model_to_result_response(&inserted);
    audit::service::append(
        conn,
        entered_by,
        "result.submitted",
        "result",
        resp.id,
        serde_json::to_value(&resp).unwrap_or_default(),
        serde_json::json!({ "event_id": event_id }),
    )
    .await?;
    Ok(resp)
}

// ══════════════════════════════════════════════════════════════════════════════
// Submit review
// ══════════════════════════════════════════════════════════════════════════════

pub async fn submit_review(
    conn: &DatabaseConnection,
    event_id: i64,
    result_id: i64,
    referee_id: i64,
    req: SubmitReviewRequest,
) -> AppResult<ReviewResponse> {
    let decision = parse_review_decision(&req.decision)?;

    // Result must exist and belong to this event.
    let result_model = load_result_for_event(conn, event_id, result_id).await?;

    // Once arbitrated, the state is final — no further reviews accepted.
    let already_arbitrated = arbitration_entity::Entity::find()
        .filter(arbitration_entity::Column::ResultId.eq(result_id))
        .one(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .is_some();

    if already_arbitrated {
        return Err(AppError::Conflict(format!(
            "Result {result_id} has already been arbitrated and cannot accept further reviews"
        )));
    }

    // Load event to check championship class.
    let event_model = event_entity::Entity::find_by_id(event_id)
        .one(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Event {event_id} not found")))?;

    let now = Utc::now();

    // Insert review + re-derive state atomically.
    let (review_model, new_state) = conn
        .transaction::<_, (review_entity::Model, ReviewedState), AppError>(|txn| {
            let req = req.clone();
            let result_model = result_model.clone();
            let decision = decision.clone();
            Box::pin(async move {
                let review = ReviewActiveModel {
                    result_id:   Set(result_id),
                    referee_id:  Set(referee_id),
                    decision:    Set(decision),
                    comment:     Set(req.comment),
                    reviewed_at: Set(now),
                    created_at:  Set(now),
                    ..Default::default()
                }
                .insert(txn)
                .await
                .map_err(|e| {
                    let msg = e.to_string();
                    if msg.contains("UNIQUE") {
                        AppError::Conflict(format!(
                            "Referee {referee_id} has already submitted a review for result {result_id}"
                        ))
                    } else {
                        AppError::Internal(msg)
                    }
                })?;

                // Reload all reviews for this result to derive the new state.
                let all_reviews = review_entity::Entity::find()
                    .filter(review_entity::Column::ResultId.eq(result_id))
                    .all(txn)
                    .await
                    .map_err(|e| AppError::Internal(e.to_string()))?;

                let new_state =
                    derive_reviewed_state(&all_reviews, event_model.is_championship_class);

                // Update result.reviewed_state if it changed.
                if new_state != result_model.reviewed_state {
                    let mut active: result_entity::ActiveModel = result_model.into();
                    active.reviewed_state = Set(new_state.clone());
                    active.updated_at     = Set(now);
                    active
                        .update(txn)
                        .await
                        .map_err(|e| AppError::Internal(e.to_string()))?;
                }

                Ok((review, new_state))
            })
        })
        .await
        .map_err(tx_err)?;

    tracing::info!(
        result_id,
        event_id,
        referee_id,
        decision = %req.decision,
        new_state = reviewed_state_str(&new_state),
        "results.review_submitted"
    );
    let resp = model_to_review_response(&review_model);
    audit::service::append(
        conn,
        referee_id,
        "result.reviewed",
        "result",
        result_id,
        serde_json::to_value(&resp).unwrap_or_default(),
        serde_json::json!({
            "event_id":  event_id,
            "decision":  req.decision,
            "new_state": reviewed_state_str(&new_state),
        }),
    )
    .await?;
    Ok(resp)
}

/// List all reviews for a result.
pub async fn list_reviews(
    conn: &DatabaseConnection,
    event_id: i64,
    result_id: i64,
) -> AppResult<Vec<ReviewResponse>> {
    load_result_for_event(conn, event_id, result_id).await?;

    let reviews = review_entity::Entity::find()
        .filter(review_entity::Column::ResultId.eq(result_id))
        .order_by_asc(review_entity::Column::CreatedAt)
        .all(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(reviews.iter().map(model_to_review_response).collect())
}

// ══════════════════════════════════════════════════════════════════════════════
// Arbitration
// ══════════════════════════════════════════════════════════════════════════════

pub async fn arbitrate_result(
    conn: &DatabaseConnection,
    event_id: i64,
    result_id: i64,
    arbitrator_id: i64,
    req: ArbitrateRequest,
) -> AppResult<ArbitrationResponse> {
    let decision = parse_review_decision(&req.decision)?;

    let result_model = load_result_for_event(conn, event_id, result_id).await?;

    // Arbitration is only valid while the result is in `pending` state
    // (including the conflict-pending state after disagreeing reviews) or
    // `rejected` (override a premature rejection before the full review panel
    // has been heard).
    if result_model.reviewed_state == ReviewedState::Approved {
        return Err(AppError::Conflict(format!(
            "Result {result_id} is already approved — arbitration is only valid for \
             pending or rejected results"
        )));
    }

    let now = Utc::now();

    let arb_model = conn
        .transaction::<_, arbitration_entity::Model, AppError>(|txn| {
            let decision = decision.clone();
            let req = req.clone();
            let result_model = result_model.clone();
            Box::pin(async move {
                let arb = ArbitrationActiveModel {
                    result_id: Set(result_id),
                    arbitrated_by: Set(arbitrator_id),
                    decision: Set(decision.clone()),
                    comment: Set(req.comment),
                    created_at: Set(now),
                    ..Default::default()
                }
                .insert(txn)
                .await
                .map_err(|e| {
                    let msg = e.to_string();
                    if msg.contains("UNIQUE") {
                        AppError::Conflict(format!(
                            "Result {result_id} has already been arbitrated"
                        ))
                    } else {
                        AppError::Internal(msg)
                    }
                })?;

                // Apply the binding decision to the result row.
                let new_reviewed_state = match decision {
                    ReviewDecision::Approved => ReviewedState::Approved,
                    ReviewDecision::Rejected => ReviewedState::Rejected,
                };
                let mut active: result_entity::ActiveModel = result_model.into();
                active.reviewed_state = Set(new_reviewed_state);
                active.updated_at = Set(now);
                active
                    .update(txn)
                    .await
                    .map_err(|e| AppError::Internal(e.to_string()))?;

                Ok(arb)
            })
        })
        .await
        .map_err(tx_err)?;

    tracing::info!(
        result_id,
        event_id,
        arbitrator_id,
        decision = %req.decision,
        "results.arbitrated"
    );
    let resp = model_to_arbitration_response(&arb_model);
    audit::service::append(
        conn,
        arbitrator_id,
        "result.arbitrated",
        "result",
        result_id,
        serde_json::to_value(&resp).unwrap_or_default(),
        serde_json::json!({
            "event_id": event_id,
            "decision": req.decision,
        }),
    )
    .await?;
    Ok(resp)
}

// ══════════════════════════════════════════════════════════════════════════════
// Corrections
// ══════════════════════════════════════════════════════════════════════════════

pub async fn request_correction(
    conn: &DatabaseConnection,
    event_id: i64,
    result_id: i64,
    requester_id: i64,
    req: RequestCorrectionRequest,
) -> AppResult<CorrectionResponse> {
    let corrected_unit = parse_unit(&req.corrected_unit)?;

    if !req.corrected_value.is_finite() || req.corrected_value < 0.0 {
        return Err(AppError::BadRequest(
            "corrected_value must be a non-negative finite number".into(),
        ));
    }

    // Verify the result belongs to this event.
    load_result_for_event(conn, event_id, result_id).await?;

    // One pending correction per result (DB partial-unique index enforces this
    // as a fallback, but give a clear error here).
    let pending_exists = correction_entity::Entity::find()
        .filter(correction_entity::Column::ResultId.eq(result_id))
        .filter(correction_entity::Column::Status.eq("pending"))
        .one(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .is_some();

    if pending_exists {
        return Err(AppError::Conflict(format!(
            "Result {result_id} already has a pending correction; \
             resolve it before requesting another"
        )));
    }

    let now = Utc::now();
    let correction = CorrectionActiveModel {
        result_id: Set(result_id),
        corrected_value: Set(req.corrected_value),
        corrected_unit: Set(corrected_unit),
        requested_by: Set(requester_id),
        reason: Set(req.reason.clone()),
        status: Set(CorrectionStatus::Pending),
        resolved_by: Set(None),
        resolved_at: Set(None),
        created_at: Set(now),
        ..Default::default()
    }
    .insert(conn)
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?;

    tracing::info!(
        correction_id = correction.id,
        result_id,
        event_id,
        requester_id,
        "results.correction_requested"
    );
    let resp = model_to_correction_response(&correction);
    audit::service::append(
        conn,
        requester_id,
        "result.correction_requested",
        "result",
        result_id,
        serde_json::to_value(&resp).unwrap_or_default(),
        serde_json::json!({
            "event_id":        event_id,
            "correction_id":   resp.id,
        }),
    )
    .await?;
    Ok(resp)
}

/// List all correction versions for a result, oldest first (full history).
pub async fn list_corrections(
    conn: &DatabaseConnection,
    event_id: i64,
    result_id: i64,
) -> AppResult<Vec<CorrectionResponse>> {
    load_result_for_event(conn, event_id, result_id).await?;

    let corrections = correction_entity::Entity::find()
        .filter(correction_entity::Column::ResultId.eq(result_id))
        .order_by_asc(correction_entity::Column::CreatedAt)
        .all(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(corrections
        .iter()
        .map(model_to_correction_response)
        .collect())
}

/// Approve or reject a pending correction.
///
/// The correction row is **never modified** — instead its `status`, `resolved_by`,
/// and `resolved_at` columns are updated exactly once (via the state transition
/// from `pending`).  All prior correction rows remain immutable.
pub async fn resolve_correction(
    conn: &DatabaseConnection,
    event_id: i64,
    result_id: i64,
    correction_id: i64,
    resolver_id: i64,
    req: ResolveCorrectionRequest,
) -> AppResult<CorrectionResponse> {
    let decision = parse_review_decision(&req.decision)?;

    // Verify lineage.
    load_result_for_event(conn, event_id, result_id).await?;

    let correction = correction_entity::Entity::find_by_id(correction_id)
        .one(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Correction {correction_id} not found")))?;

    if correction.result_id != result_id {
        return Err(AppError::NotFound(format!(
            "Correction {correction_id} does not belong to result {result_id}"
        )));
    }

    if correction.status != CorrectionStatus::Pending {
        return Err(AppError::Conflict(format!(
            "Correction {correction_id} is already '{}' and cannot be re-resolved",
            correction_status_str(&correction.status)
        )));
    }

    let now = Utc::now();
    let new_status = match decision {
        ReviewDecision::Approved => CorrectionStatus::Approved,
        ReviewDecision::Rejected => CorrectionStatus::Rejected,
    };

    let mut active: CorrectionActiveModel = correction.into();
    active.status = Set(new_status.clone());
    active.resolved_by = Set(Some(resolver_id));
    active.resolved_at = Set(Some(now));

    let updated = active
        .update(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    tracing::info!(
        correction_id,
        result_id,
        event_id,
        resolver_id,
        decision = %req.decision,
        "results.correction_resolved"
    );
    let resp = model_to_correction_response(&updated);
    audit::service::append(
        conn,
        resolver_id,
        "result.correction_resolved",
        "result",
        result_id,
        serde_json::to_value(&resp).unwrap_or_default(),
        serde_json::json!({
            "event_id":      event_id,
            "correction_id": correction_id,
            "decision":      req.decision,
        }),
    )
    .await?;
    Ok(resp)
}

// ══════════════════════════════════════════════════════════════════════════════
// Rankings
// ══════════════════════════════════════════════════════════════════════════════

pub async fn get_rankings(
    conn: &DatabaseConnection,
    event_id: i64,
    query: RankingsQuery,
) -> AppResult<RankingsResponse> {
    let unit = parse_unit(&query.unit)?;

    let rule = query.advancement_rule.as_str();
    if !matches!(rule, "top_n" | "percentile") {
        return Err(AppError::BadRequest(
            "advancement_rule must be 'top_n' or 'percentile'".into(),
        ));
    }
    if rule == "percentile" && !(0.0..=100.0).contains(&query.advancement_value) {
        return Err(AppError::BadRequest(
            "advancement_value must be 0–100 for percentile rule".into(),
        ));
    }
    if rule == "top_n" && query.advancement_value < 1.0 {
        return Err(AppError::BadRequest(
            "advancement_value must be >= 1 for top_n rule".into(),
        ));
    }

    // Load all non-rejected results for this event.
    let rows = result_entity::Entity::find()
        .filter(result_entity::Column::EventId.eq(event_id))
        .filter(result_entity::Column::ReviewedState.ne("rejected"))
        .order_by_asc(result_entity::Column::CreatedAt)
        .all(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // Load all approved corrections for these results so rankings reflect
    // the effective (corrected) value.  Map: result_id → latest approved correction.
    let result_ids: Vec<i64> = rows.iter().map(|r| r.id).collect();
    let approved_corrections: HashMap<i64, correction_entity::Model> = if result_ids.is_empty() {
        HashMap::new()
    } else {
        let corrections = correction_entity::Entity::find()
            .filter(correction_entity::Column::ResultId.is_in(result_ids))
            .filter(correction_entity::Column::Status.eq("approved"))
            .order_by_asc(correction_entity::Column::CreatedAt)
            .all(conn)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        // Keep only the latest approved correction per result.
        let mut map: HashMap<i64, correction_entity::Model> = HashMap::new();
        for c in corrections {
            map.insert(c.result_id, c);
        }
        map
    };

    // Compute effective value and unit for each result row.
    struct EffectiveAttempt {
        value: f64,
        unit: ResultUnit,
        attempt_no: i32,
        created_at: String,
    }

    let ascending = is_ascending(&unit);
    let mut bests: HashMap<i64, EffectiveAttempt> = HashMap::new();

    for row in &rows {
        // Apply correction if one exists.
        let (eff_value, eff_unit) = if let Some(corr) = approved_corrections.get(&row.id) {
            (corr.corrected_value, corr.corrected_unit.clone())
        } else {
            (row.value_numeric, row.unit_enum.clone())
        };

        // Only include in this ranking if the effective unit matches the query.
        if &eff_unit != &unit {
            continue;
        }

        let is_better = match bests.get(&row.participant_id) {
            None => true,
            Some(prev) => {
                if ascending {
                    eff_value < prev.value
                } else {
                    eff_value > prev.value
                }
            }
        };

        if is_better {
            bests.insert(
                row.participant_id,
                EffectiveAttempt {
                    value: eff_value,
                    unit: eff_unit,
                    attempt_no: row.attempt_no,
                    created_at: row.created_at.to_rfc3339(),
                },
            );
        }
    }

    // Sort: best value first, then earliest recorded_at as tie-breaker.
    let mut entries: Vec<(i64, EffectiveAttempt)> = bests.into_iter().collect();
    entries.sort_by(|(_, a), (_, b)| {
        let value_ord = if ascending {
            a.value
                .partial_cmp(&b.value)
                .unwrap_or(std::cmp::Ordering::Equal)
        } else {
            b.value
                .partial_cmp(&a.value)
                .unwrap_or(std::cmp::Ordering::Equal)
        };
        if value_ord != std::cmp::Ordering::Equal {
            return value_ord;
        }
        a.created_at.cmp(&b.created_at)
    });

    let total = entries.len();

    let advances_cutoff: u32 = match rule {
        "top_n" => query.advancement_value as u32,
        "percentile" => ((total as f64 * query.advancement_value / 100.0).ceil() as u32).max(0),
        _ => unreachable!(),
    };

    let rankings: Vec<RankEntry> = entries
        .into_iter()
        .enumerate()
        .map(|(i, (participant_id, best))| {
            let rank = (i + 1) as u32;
            RankEntry {
                rank,
                participant_id,
                best_value: best.value,
                unit: unit_str(&best.unit).to_owned(),
                best_attempt_no: best.attempt_no,
                best_recorded_at: best.created_at,
                advances: rank <= advances_cutoff,
            }
        })
        .collect();

    Ok(RankingsResponse {
        event_id,
        unit: unit_str(&unit).to_owned(),
        advancement_rule: query.advancement_rule,
        advancement_value: query.advancement_value,
        total_participants: total,
        rankings,
    })
}

// ══════════════════════════════════════════════════════════════════════════════
// Export results as CSV
// ══════════════════════════════════════════════════════════════════════════════

/// Export all results for an event as a UTF-8 CSV document.
///
/// Each row contains the raw submitted data plus the current review state and
/// the effective corrected value (if an approved correction exists).
///
/// Columns: id, event_id, participant_id, attempt_no, value_numeric, unit,
///          effective_value, effective_unit, reviewed_state,
///          entered_by, created_at, updated_at
pub async fn export_results_csv(
    conn: &DatabaseConnection,
    event_id: i64,
) -> AppResult<String> {
    // Verify event exists.
    event_entity::Entity::find_by_id(event_id)
        .one(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Event {event_id} not found")))?;

    let results = result_entity::Entity::find()
        .filter(result_entity::Column::EventId.eq(event_id))
        .order_by_asc(result_entity::Column::ParticipantId)
        .order_by_asc(result_entity::Column::AttemptNo)
        .all(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // Load all approved corrections for this event so we can resolve effective values.
    let all_corrections = correction_entity::Entity::find()
        .filter(correction_entity::Column::ResultId.is_in(results.iter().map(|r| r.id).collect::<Vec<_>>()))
        .filter(correction_entity::Column::Status.eq(CorrectionStatus::Approved))
        .order_by_asc(correction_entity::Column::CreatedAt)
        .all(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // Build a lookup: result_id → latest approved correction.
    let mut latest_correction: HashMap<i64, &correction_entity::Model> = HashMap::new();
    for c in &all_corrections {
        latest_correction.insert(c.result_id, c);
    }

    let mut csv = String::from(
        "id,event_id,participant_id,attempt_no,value_numeric,unit,\
         effective_value,effective_unit,reviewed_state,entered_by,\
         created_at,updated_at\n",
    );

    for r in &results {
        let (eff_val, eff_unit) = match latest_correction.get(&r.id) {
            Some(c) => (c.corrected_value, unit_str(&c.corrected_unit).to_owned()),
            None => (r.value_numeric, unit_str(&r.unit_enum).to_owned()),
        };
        let reviewed_state = match r.reviewed_state {
            ReviewedState::Pending => "pending",
            ReviewedState::Approved => "approved",
            ReviewedState::Rejected => "rejected",
        };
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{}\n",
            r.id,
            r.event_id,
            r.participant_id,
            r.attempt_no,
            r.value_numeric,
            unit_str(&r.unit_enum),
            eff_val,
            eff_unit,
            reviewed_state,
            r.entered_by,
            r.created_at.to_rfc3339(),
            r.updated_at.to_rfc3339(),
        ));
    }

    Ok(csv)
}
