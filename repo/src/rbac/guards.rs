//! Typed permission guards for Rocket route signatures.
//!
//! Each guard validates the caller's session **and** confirms the user holds
//! the corresponding `Permission`.  Use these as route parameters in place of
//! plain `AuthenticatedUser` whenever a route requires a specific capability.
//!
//! ```rust
//! // Only users with EventsWrite permission can reach this handler.
//! #[post("/events", data = "<body>")]
//! async fn create_event(_p: RequireEventsWrite, body: Json<...>) -> ... { ... }
//! ```

use crate::rbac::Permission;

// System
permission_guard!(RequireSystemAdmin, Permission::SystemAdmin);

// Users
permission_guard!(RequireUsersRead, Permission::UsersRead);
permission_guard!(RequireUsersManage, Permission::UsersManage);

// Roles
permission_guard!(RequireRolesManage, Permission::RolesManage);

// Events
permission_guard!(RequireEventsRead, Permission::EventsRead);
permission_guard!(RequireEventsWrite, Permission::EventsWrite);
permission_guard!(RequireEventsDelete, Permission::EventsDelete);

// Participants
permission_guard!(RequireParticipantsRead, Permission::ParticipantsRead);
permission_guard!(RequireParticipantsWrite, Permission::ParticipantsWrite);

// Referees
permission_guard!(RequireRefereesRead, Permission::RefereesRead);
permission_guard!(RequireRefereesWrite, Permission::RefereesWrite);

// Financials
permission_guard!(RequireFinancialsRead, Permission::FinancialsRead);
permission_guard!(RequireFinancialsWrite, Permission::FinancialsWrite);

// Audit
permission_guard!(RequireAuditRead, Permission::AuditRead);
