# API Specification: Motorsport Event Operations

## 1. Authentication & Security
Bearer token should be passed in the `Authorization: Bearer <token>` header for all protected endpoints.

### POST /auth/login
Authenticate with local credentials.
- **Request**: `LoginRequest { username, password }`
- **Response**: `200 OK` with `LoginResponse { token, expires_at }`
- **Error**: `401 Unauthorized` on invalid credentials.

### POST /auth/logout
Invalidate current session.
- **Headers**: Authorization required.
- **Response**: `200 OK`

### POST /auth/rotate-password
Change password and invalidate all active sessions.
- **Request**: `RotatePasswordRequest { current_password, new_password }`
- **Response**: `200 OK`

---

## 2. Competition Configuration

### POST /events
Create a new draft event.
- **Permissions**: `events:write`
- **Request**: `CreateEventRequest { name, schedule_group, venue_id, asset_ids, .. }`
- **Response**: `201 Created`

### GET /events/<id>
Retrieve event details.
- **Permissions**: `events:read`
- **Response**: `200 OK`

### PUT /events/<id>
Update a draft event.
- **Permissions**: `events:write`
- **Response**: `200 OK` or `409 Conflict` if not in draft.

### POST /events/<id>/publish
Freeze configuration and set ruleset version.
- **Request**: `{ ruleset_version_id }`
- **Response**: `200 OK`

### POST /rulesets
Publish a new immutable ruleset version.
- **Request**: `CreateRulesetRequest { semantic_version, effective_at, .. }`
- **Response**: `201 Created`

---

## 3. Results & Arbitration

### POST /events/<id>/results
Submit a result for a participant.
- **Request**: `SubmitResultRequest { participant_id, value_numeric, unit }`
- **Response**: `200 OK`

### POST /events/<id>/results/<rid>/reviews
Submit a referee review.
- **Permissions**: `Referee` or `Administrator`
- **Request**: `ReviewRequest { decision: approved|rejected, comment }`
- **Response**: `200 OK`

### POST /events/<id>/results/<rid>/arbitrate
Arbitration override by Event Director.
- **Permissions**: `EventDirector` or `Administrator`
- **Request**: `ArbitrateRequest { decision, comment }`
- **Response**: `200 OK`

### GET /events/<id>/rankings
Get deterministic rankings with tie-breakers.
- **Query Params**: `unit`, `advancement_rule`, `advancement_value`
- **Response**: `200 OK`

---

## 4. Asset Ledger & Vehicles

### POST /assets/import
Bulk import assets from JSON/CSV.
- **Response**: `200 OK` with import summary and full version history links.

### PATCH /vehicles/<vin>/status
Transition vehicle through lifecycle: Draft → Published → Delisted → Sold.
- **Request**: `TransitionStatusRequest { status, reason }`
- **Response**: `200 OK`

---

## 5. Billing & Payments

### POST /invoices
Generate invoice for event participation/assets.
- **Request**: `{ counterparty, lines: [] }`
- **Response**: `201 Created`

### POST /invoices/<id>/discount
Apply percentage (0-30%) or fixed discount (capped at $500).
- **Request**: `{ type: percentage|fixed, value }`
- **Response**: `200 OK`

### POST /payments
Record manual payment ledger entry.
- **Request**: `RecordPaymentRequest { method: cash|check|ach, amount, externalReference }`
- **Response**: `201 Created` or `409 Conflict` on duplicate `externalReference`.

---

## 6. Analytics & Audit

### POST /analytics/metrics
Register a new unified metric in the catalog.
- **Request**: `{ name, definition, version }`
- **Response**: `201 Created`

### GET /audit-logs
Query append-only audit trail.
- **Query Params**: `entity_type`, `entity_id`, `actor_id`, `time_window`
- **Response**: `200 OK` with masked sensitive fields (PII/Payment Refs).
