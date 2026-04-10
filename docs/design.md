# System Design: Motorsport Event Operations

## 1. Architectural Overview
The system is implemented as a standalone, offline-capable monolith using **Rust** and the **Rocket** web framework. Persistence is handled by **SQLite** via **SeaORM**, enabling zero-network dependency for deployment in remote race venues.

### Architecture Patterns
- **Resource-Scoped API**: Domain-driven design with modules for `competition`, `results`, `billing`, `inventory`, and `audit`.
- **Identity & RBAC middleware**: Stateless session management with SHA-256 hashed tokens and field-level permission checks.
- **Service Layer Pattern**: Handlers manage HTTP concerns; Services manage transactions, business logic, and audit side-effects.

---

## 2. Security Design
- **Encryption at Rest**: Sensitive fields (VIN, Payment References, PII) are encrypted using **AES-256-GCM** with a key loaded from the local environment.
- **Blind Indexing**: To support unique constraints and efficient SQL lookups on encrypted fields, deterministic keyed hashes (**HMAC-SHA256**) are stored in parallel columns.
- **Audit Immunity**: An append-only audit log is used for all state transitions, capturing `who`, `what`, `when`, and `why`. Sensitive values are masked in audit exports.
- **Credential Safety**: Local passwords are never stored in plaintext — only **Argon2** salted hashes.

---

## 3. Core Business Logic Designs

### Result Ranking & Arbitration
- **Deterministic Tie-breakers**: Ranking logic first sorts by `best_value`, then by `best_attempt_time` (earliest valid attempt).
- **Multi-Referee Quorum**: Championship classes trigger a logic gate requiring $N \ge 2$ positive reviews. If reviews conflict, the system remains in a `pending_arbitration` state until an **Event Director** issues an override.
- **Immutability**: Once a result is approved, it is immutable. Corrections follow a versioned "Request → Review → Effective" lifecycle that preserves the prior erroneous record.

### Asset & Vehicle Lifecycle
- **Workflow State Machine**: Vehicles transition through `Draft → Published → Delisted → Sold`. Each transition requires a reason and is recorded in a specific vehicle audit log.
- **Depreciation Engine**: Implements straight-line depreciation calculated at the Service layer based on `procurement_cost`, `useful_life_in_months`, and `procurement_date`.

### Offline Payment Idempotency
- **Conflict Prevention**: To prevent duplicate manual ledger entries (critical in offline scenarios), every payment must supply a unique `externalReference` (e.g., check number or bank ACH ref).
- **SQLite Constraint**: A `UNIQUE` index on `payment_entries(external_reference_hash)` ensures that even if a UI is double-clicked or a device is re-synced, duplicates are rejected at the database level.

---

## 4. Non-Functional Readiness
- **Latency Monitoring**: Every request is stamped with a `correlation_id` and logged with its duration to verify the **p95 < 250ms** target.
- **Persistence Safety**: A background scheduler performs nightly SQLite backups with a 7-day rotation policy to prevent data loss in standalone environments.
- **Data Quality Scans**: A dedicated module performs periodic Z-Score analysis on results to flag outliers and runs stable hashing scans to detect duplicate entries across different tables.
