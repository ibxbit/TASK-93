# Motorsport Backend

Offline-first REST API for motorsport event management. Handles events, vehicles, assets, billing, payments, results, audit logging, and nightly backups — all running on a single SQLite database with no external dependencies.

---

## Start Command

```bash
docker compose up
```

That's it. On first launch the container:
1. Compiles the Rust binary (multi-stage Docker build)
2. Runs all 35 database migrations automatically
3. Seeds the 5 RBAC roles and 5 default users
4. Starts the HTTP server and nightly backup scheduler

---

## Service Addresses

| Service | URL |
|---------|-----|
| REST API | http://localhost:8000 |
| Health check | http://localhost:8000/health |

There is no frontend — all interaction is via the REST API documented below.

---

## Default Users

Seeded automatically on first startup. All credentials are for **development only**.

| Username | Password | Role |
|----------|----------|------|
| `admin` | `Admin123!` | Administrator (full access) |
| `director` | `Director123!` | EventDirector |
| `referee1` | `Referee123!` | Referee (read-only) |
| `finance1` | `Finance123!` | FinanceClerk |
| `auditor1` | `Auditor123!` | Auditor (read-only) |

---

## Authentication

All endpoints except `POST /auth/login` require a Bearer token:

```bash
# 1. Login
curl -s -X POST http://localhost:8000/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"admin","password":"Admin123!"}' | jq .

# 2. Use the returned token
TOKEN="<token from response>"
curl -s http://localhost:8000/events \
  -H "Authorization: Bearer $TOKEN" | jq .
```

---

## API Endpoints

### Auth
| Method | Path | Description |
|--------|------|-------------|
| POST | `/auth/login` | Login — returns bearer token |
| POST | `/auth/logout` | Logout — invalidates token |
| POST | `/auth/rotate-password` | Change own password |

### Events & Rulesets
| Method | Path | Permission |
|--------|------|------------|
| POST | `/events` | events:write |
| GET | `/events` | events:read |
| GET | `/events/<id>` | events:read |
| PUT | `/events/<id>` | events:write |
| POST | `/events/<id>/publish` | events:write |
| POST | `/rulesets` | events:write |
| GET | `/rulesets` | events:read |
| GET | `/rulesets/<id>` | events:read |
| POST | `/rulesets/<id>/rollback` | events:write |

### Vehicles
| Method | Path | Permission |
|--------|------|------------|
| POST | `/vehicles` | events:write |
| GET | `/vehicles` | events:read |
| GET | `/vehicles/<id>` | events:read |
| PUT | `/vehicles/<id>` | events:write |
| POST | `/vehicles/<id>/status` | events:write |
| GET | `/vehicles/<id>/history` | events:read |

### Assets
| Method | Path | Permission |
|--------|------|------------|
| POST | `/assets` | events:write |
| GET | `/assets` | events:read |
| GET | `/assets/<id>` | events:read |
| PUT | `/assets/<id>` | events:write |
| PATCH | `/assets/<id>/status` | events:write |
| GET | `/assets/<id>/history` | events:read |
| GET | `/assets/export` | events:read |
| POST | `/assets/import` | events:write |

### Results & Reviews
| Method | Path | Permission |
|--------|------|------------|
| POST | `/events/<eid>/results` | participants:write |
| GET | `/events/<eid>/rankings` | events:read |
| GET | `/events/<eid>/results/export` | audit:read |
| POST | `/events/<eid>/results/<rid>/reviews` | referees:write |
| GET | `/events/<eid>/results/<rid>/reviews` | events:read |
| POST | `/events/<eid>/results/<rid>/arbitrate` | events:write |
| POST | `/events/<eid>/results/<rid>/corrections` | participants:write |
| GET | `/events/<eid>/results/<rid>/corrections` | events:read |
| POST | `/events/<eid>/results/<rid>/corrections/<cid>/resolve` | events:write |

### Billing & Payments
| Method | Path | Permission |
|--------|------|------------|
| POST | `/invoices` | financials:write |
| GET | `/invoices` | financials:read |
| GET | `/invoices/<id>` | financials:read |
| POST | `/invoices/<id>/lines` | financials:write |
| POST | `/invoices/<id>/discount` | financials:write |
| POST | `/invoices/<id>/issue` | financials:write |
| POST | `/invoices/<iid>/payments` | financials:write |
| GET | `/invoices/<iid>/payments` | financials:read |
| POST | `/invoices/<iid>/payments/<pid>/exceptions` | financials:write |
| GET | `/invoices/<iid>/payments/<pid>/exceptions` | financials:read |
| POST | `/invoices/<iid>/payments/<pid>/refunds` | financials:write |
| POST | `/invoices/<iid>/payments/<pid>/refunds/<rid>/approve` | financials:write (stage 1) / audit:read (stage 2) |
| POST | `/invoices/<iid>/payments/<pid>/refunds/<rid>/reject` | financials:write |

### Audit Logs
| Method | Path | Permission |
|--------|------|------------|
| GET | `/audit/logs` | audit:read |
| GET | `/audit/logs/<id>` | audit:read |

### Analytics & Data Quality
| Method | Path | Permission |
|--------|------|------------|
| POST | `/metrics` | financials:write |
| GET | `/metrics` | financials:read |
| GET | `/metrics/<id>` | financials:read |
| PUT | `/metrics/<id>` | financials:write |
| GET | `/analytics/trends` | audit:read |
| GET | `/analytics/funnel` | audit:read |
| GET | `/analytics/retention` | audit:read |
| GET | `/analytics/export` | audit:read |
| POST | `/data-quality/scans` | audit:read |
| GET | `/data-quality/scans` | audit:read |
| GET | `/data-quality/scans/<id>` | audit:read |

### Administration
| Method | Path | Permission |
|--------|------|------------|
| POST | `/admin/roles/assign` | roles:manage |
| POST | `/admin/roles/revoke` | roles:manage |
| GET | `/admin/users/<id>/roles` | roles:manage |
| GET | `/admin/backups` | system:admin |
| POST | `/admin/backups` | system:admin |
| POST | `/admin/backups/<filename>/restore` | system:admin |

---

## Step-by-Step Verification Guide

### 1. Confirm the server is healthy

```bash
curl -s http://localhost:8000/health | jq .
# Expected: {"status":"ok","version":"0.1.0"}
```

### 2. Authenticate and capture a token

```bash
TOKEN=$(curl -s -X POST http://localhost:8000/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"admin","password":"Admin123!"}' | jq -r .token)
echo "Token: $TOKEN"
```

### 3. Create a ruleset version

```bash
curl -s -X POST http://localhost:8000/rulesets \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"semantic_version":"1.0.0","description":"Initial rules","effective_at":"2026-01-01T00:00:00Z"}' | jq .
```

### 4. Create and publish an event

```bash
# Create
EVENT=$(curl -s -X POST http://localhost:8000/events \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"name":"Round 1 2026","description":"Opening round"}' | jq .)
echo $EVENT | jq .
EVENT_ID=$(echo $EVENT | jq .id)

# Publish (use the ruleset ID from step 3)
RULESET_ID=1
curl -s -X POST "http://localhost:8000/events/$EVENT_ID/publish" \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d "{\"ruleset_version_id\":$RULESET_ID}" | jq .
```

### 5. Create a vehicle (with encrypted VIN)

```bash
curl -s -X POST http://localhost:8000/vehicles \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{
    "vin":"1HGCM82633A004352",
    "registration_id":"ABC-1234",
    "make":"Honda",
    "model":"Accord",
    "year":2023
  }' | jq .
```

### 6. Create an invoice and record a payment

```bash
# Create invoice
INV=$(curl -s -X POST http://localhost:8000/invoices \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"invoice_no":"INV-001","counterparty":"Acme Racing","issue_date":"2026-01-15","tax_rate":0.10}' | jq .)
INV_ID=$(echo $INV | jq .id)

# Add a line item
curl -s -X POST "http://localhost:8000/invoices/$INV_ID/lines" \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"description":"Entry fee","pricing_model":"fixed","quantity":1,"unit_price":500.00}' | jq .

# Issue the invoice
curl -s -X POST "http://localhost:8000/invoices/$INV_ID/issue" \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{}' | jq .

# Record payment
curl -s -X POST "http://localhost:8000/invoices/$INV_ID/payments" \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"amount":550.00,"method":"cash","external_reference":"TXN-2026-001","received_at":"2026-01-15T12:00:00Z"}' | jq .
```

### 7. Query the audit log (as auditor)

```bash
AUDITOR_TOKEN=$(curl -s -X POST http://localhost:8000/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"auditor1","password":"Auditor123!"}' | jq -r .token)

curl -s "http://localhost:8000/audit/logs?limit=10" \
  -H "Authorization: Bearer $AUDITOR_TOKEN" | jq .
```

### 8. Trigger a manual backup (admin only)

```bash
curl -s -X POST http://localhost:8000/admin/backups \
  -H "Authorization: Bearer $TOKEN" | jq .
```

### 9. Verify RBAC enforcement

```bash
# Referee should NOT be able to create events
REFEREE_TOKEN=$(curl -s -X POST http://localhost:8000/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"referee1","password":"Referee123!"}' | jq -r .token)

curl -s -X POST http://localhost:8000/events \
  -H "Authorization: Bearer $REFEREE_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"name":"Unauthorized"}' | jq .
# Expected: {"code":"FORBIDDEN","message":"..."}
```

---

## Running Tests

```bash
./run_tests.sh
```

The script runs all unit and API tests automatically. The server must be reachable at `http://localhost:8000` (started via `docker compose up`).

Alternatively, run each suite individually:

```bash
# Unit tests (no server required)
python -m pytest unit_tests/ -v

# API tests (server must be running)
python -m pytest API_tests/ -v
```

---

## Project Structure

```
repo/
├── src/                     # Rust source — layered architecture
│   ├── main.rs              # Entrypoint: config, DB, Rocket mount
│   ├── config.rs            # Environment-based configuration
│   ├── db.rs                # Connection pool + SQLite PRAGMAs
│   ├── crypto.rs            # AES-256-GCM field encryption
│   ├── errors.rs            # Typed error → HTTP status mapping
│   ├── auth/                # Session auth + Argon2 password hashing
│   ├── rbac/                # Role/permission guards + seeder
│   ├── competition/         # Events and ruleset versions
│   ├── vehicles/            # Vehicle lifecycle management
│   ├── assets/              # Equipment asset register
│   ├── billing/             # Invoices and line items
│   ├── payments/            # Payments, exceptions, refunds
│   ├── results/             # Race results, reviews, arbitration
│   ├── audit/               # Unified append-only audit trail
│   ├── backup/              # Nightly SQLite VACUUM INTO backups
│   ├── analytics/           # Metric definitions and queries
│   ├── data_quality/        # DQ scan results
│   ├── entity/              # SeaORM entity models
│   ├── migration/           # 35 ordered database migrations
│   └── middleware/          # Correlation ID + request logging
├── unit_tests/              # Offline business-logic tests (Python/pytest)
├── API_tests/               # HTTP endpoint tests (Python/pytest + requests)
├── run_tests.sh             # Test runner (unit + API)
├── Dockerfile               # Multi-stage build: rust:1.77-slim → debian:bookworm-slim
├── docker-compose.yml       # Service orchestration + named volumes
├── .env                     # Development environment (ENCRYPTION_KEY pre-set)
└── .env.example             # Template for production credentials
```

---

## Event Templates and Ruleset Versioning

Events in this system are configuration-driven via **rulesets**, which serve as versioned event templates.

### How it works

1. **Create a ruleset version** (`POST /rulesets`) — a ruleset captures the governing rules for a class of events, including `semantic_version`, `description`, and `effective_at` date.
2. **Attach a ruleset to an event** — when creating an event, pass `ruleset_id` to link it to a specific ruleset version.
3. **Immutable after publish** — once an event is published its `ruleset_id` is locked in, providing a stable, auditable configuration snapshot.
4. **Roll back or upgrade** — create a new ruleset version and create a new event referencing it; historical events retain their original ruleset.

### Ruleset endpoints
| Method | Path | Permission |
|--------|------|------------|
| POST | `/rulesets` | events:write |
| GET | `/rulesets` | events:read |
| GET | `/rulesets/<id>` | events:read |
| POST | `/rulesets/<id>/rollback` | events:write |

> **Immutability:** Ruleset rows are never modified after creation. To supersede a version, use `POST /rulesets/<id>/rollback` which creates a new version whose `rollback_of` field links back to the original. This preserves a complete, auditable version chain.

This approach replaces a separate "event template" entity — the ruleset version *is* the reusable template, and its `semantic_version` field provides a human-readable changelog.

---

## Security Controls

### Encryption at Rest (AES-256-GCM)

All sensitive personal and financial identifiers are encrypted at the field level using AES-256-GCM before being written to the database:

| Field | Table | Companion blind-index column |
|-------|-------|------------------------------|
| `vin` | `vehicles` | `vin_hash` |
| `registration_id` | `vehicles` | — |
| `serial_number` | `assets` | — |
| `external_reference` | `payment_entries` | `reference_hash` |

**Key management:** The 256-bit AES key is loaded from the `ENCRYPTION_KEY` environment variable as a base64-encoded string. The application will refuse to start if the key is absent, not valid base64, or not exactly 32 bytes.

**Wire format:** `base64(nonce_12_bytes || ciphertext_with_tag)`. A fresh random 96-bit nonce is generated per write, so the same plaintext produces a different ciphertext on every insert.

**Blind indexes:** Columns that require SQL equality lookups (VIN uniqueness, payment reference idempotency) use a companion `*_hash` column populated by a keyed FNV-1a digest of the plaintext. The hash is opaque without the key but deterministic for the same (key, plaintext) pair.

### Audit Log Immutability

The `audit_log` table is append-only, enforced at two independent layers:

1. **Database-level triggers** (migration `m20240035`): SQLite `BEFORE UPDATE` and `BEFORE DELETE` triggers raise a `FAIL` error with the message `"audit_log is immutable"` — no application-layer bypass is possible.
2. **No mutating API endpoints**: The API exposes only `GET /audit/logs` and `GET /audit/logs/<id>`. No PUT, PATCH, or DELETE endpoints exist for audit entries.

### Sensitive Field Masking in Audit Snapshots

When a state change is written to the audit log, any sensitive fields in the entity snapshot are replaced with the redaction token `[REDACTED]` before the snapshot is persisted:

- `vin`, `registration_id` → `[REDACTED]` (vehicles)
- `serial_number` → `[REDACTED]` (assets)
- `external_reference` → `[REDACTED]` (payments)

The actual plaintext values remain accessible only through the authorised read endpoints, not through audit log queries.

### Backup Rotation and Retention

Nightly backups are created using SQLite's `VACUUM INTO` command (safe for live WAL-mode databases). After each backup:

- Backups are rotated to keep at most `BACKUP_RETAIN_DAYS` files (default: **7**).
- Files are sorted lexicographically by filename (`backup_YYYYMMDD_HHMMSS.sqlite`), which equals chronological order.
- Older backups beyond the retention window are deleted automatically.
- The retention window is configurable via the `BACKUP_RETAIN_DAYS` environment variable.

Restore operations are staged: `POST /admin/backups/<filename>/restore` writes a sentinel file; the actual data file replacement happens on next server startup before any connections open. Filenames containing path separators (`/`, `\`, `..`) are rejected immediately.

### RBAC

All API endpoints are guarded by typed Rocket `FromRequest` guards that verify the caller's session and permission before the handler body executes. Fourteen distinct permissions are mapped across five roles:

| Role | Can do |
|------|--------|
| **Administrator** | Everything |
| **EventDirector** | Events, results, referees, participants |
| **Referee** | Read-only: events, participants, referees |
| **FinanceClerk** | Invoices, payments, metrics |
| **Auditor** | Read-only: audit logs, analytics, data quality, financials |

---

## Architecture Notes

- **Layered**: Routes → Service (business logic) → SeaORM entities (DB models)
- **Encryption**: AES-256-GCM on `vin`, `registration_id`, `serial_number`, `external_reference`; blind-index companion columns (`_hash`) enable equality lookups
- **Immutable audit log**: Database-level triggers block UPDATE/DELETE on `audit_log`
- **Sensitive field masking**: encrypted fields appear as `[REDACTED]` in all audit log snapshots
- **Staged restore**: `POST /admin/backups/<f>/restore` writes a marker file; the actual copy runs on next startup before any connections open
- **Performance**: WAL mode, 64 MB page cache, 256 MB mmap, `synchronous=NORMAL`
- **Offline-first**: zero external HTTP calls; SQLite bundled via sqlx (no libsqlite3 at runtime)
