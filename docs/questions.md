## Business Gap Analysis & Logic Questions

### Competition Configuration Drift
**Question**: What happens if an Event Template is updated *after* events have already been created from it?
**Hypothesis**: Updates to templates should not affect existing events to maintain audit integrity.
**Solution**: Implemented **Versioned Rulesets**. When an event is "Published", it binds to a specific `ruleset_version_id`. Any subsequent updates to the template require a new Ruleset Version, which only affects future events or those explicitly Rollbacked/Upgraded.

### Competitive Integrity in Mixed Classes
**Question**: How does the system handle ranking when a single event contains participants using different measurement units (e.g., Timing vs. Distance)?
**Hypothesis**: Ranking must be scoped to a specific unit type to remain mathematically valid.
**Solution**: The `GET /rankings` endpoint requires a mandatory `unit` parameter. Results are filtered by unit before being passed to the ranking algorithm, ensuring "Time" participants are not ranked against "Distance" participants in the same leaderboard.

### Offline Payment Reconciliation
**Question**: In an offline environment, how do we handle payment "disputes" or "voids" if there is no real-time connection to a bank?
**Hypothesis**: Treat payments as manual ledger entries that require a multi-step audit trail for reversals.
**Solution**: Payments are never deleted. Instead, "Exception Transaction" records (Void, Reversal, Dispute) are entered as separate ledger lines linked via `idempotent externalReference`. Any refund over $1,000.00 mandates a "Double Approval" workflow requiring both a **Finance Clerk** and an **Auditor**.

### Vehicle Lifecycle Validation
**Question**: How do we prevent mileage fraud (odometer tampering) in the vehicle register?
**Hypothesis**: Enforce a non-decreasing constraint on mileage updates.
**Solution**: The `UpdateVehicleRequest` validation layer compares the current `mileage` to the incoming value. If the new value is lower, the system rejects the update with `422 Unprocessable Entity` and logs the attempted violation in the vehicle's audit log.

### Result Correction Accountability
**Question**: Who can correct a result once it has been published?
**Hypothesis**: Only officials with specific permissions should be able to initiate and approve corrections.
**Solution**: Implemented a "Request → Review → Effective" workflow. A **Referee** or **Event Director** can request a correction. However, for championship classes, the correction must follow the same multi-referee quorum rules as the original submission before it becomes effective, with all prior versions preserved in the immutable log.

### Session Inactivity Management
**Question**: How do we handle stale data/resource locks from abandoned client sessions in an offline environment?
**Hypothesis**: Auto-expire sessions after 30 minutes per prompt constraints.
**Solution**: Implemented a sliding expiration window in `src/auth/service.rs:183`. Stale sessions are best-effort purged on the next validation attempt, or can be cleared during password rotation.
