pub mod entity;
pub mod guards;
pub mod handlers;
pub mod seeder;

use serde::{Deserialize, Serialize};

// ── Role ──────────────────────────────────────────────────────────────────────

/// The five system roles.  Each has a distinct, least-privilege permission set.
/// The variant names are used as the canonical wire format (snake_case via serde).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Administrator,
    EventDirector,
    Referee,
    FinanceClerk,
    Auditor,
}

impl Role {
    /// Canonical DB / wire name.
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Administrator => "administrator",
            Role::EventDirector => "event_director",
            Role::Referee => "referee",
            Role::FinanceClerk => "finance_clerk",
            Role::Auditor => "auditor",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Role::Administrator => "Full system access",
            Role::EventDirector => "Manage events, participants, and referees",
            Role::Referee => "Read-only access to assigned events and participants",
            Role::FinanceClerk => "Manage financial records and settlements",
            Role::Auditor => "Read-only access to audit logs and all records",
        }
    }

    /// Parse from the canonical DB name.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "administrator" => Some(Role::Administrator),
            "event_director" => Some(Role::EventDirector),
            "referee" => Some(Role::Referee),
            "finance_clerk" => Some(Role::FinanceClerk),
            "auditor" => Some(Role::Auditor),
            _ => None,
        }
    }

    pub fn all() -> &'static [Role] {
        &[
            Role::Administrator,
            Role::EventDirector,
            Role::Referee,
            Role::FinanceClerk,
            Role::Auditor,
        ]
    }
}

// ── Permission ────────────────────────────────────────────────────────────────

/// Fine-grained permissions.  Each corresponds to one capability on one
/// resource type.  Roles map to a *static* subset of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    // System meta — grants all other permissions implicitly
    SystemAdmin,
    // User management
    UsersRead,
    UsersManage,
    // Role management
    RolesManage,
    // Event lifecycle
    EventsRead,
    EventsWrite,
    EventsDelete,
    // Participant management
    ParticipantsRead,
    ParticipantsWrite,
    // Referee management
    RefereesRead,
    RefereesWrite,
    // Financial records & settlement
    FinancialsRead,
    FinancialsWrite,
    // Audit trail
    AuditRead,
}

/// **Authoritative** role → permission mapping.
///
/// Every permission check in the system flows through this function, making
/// the least-privilege policy reviewable in one place.
pub fn role_permissions(role: Role) -> &'static [Permission] {
    use Permission::*;
    match role {
        // Administrator: unrestricted — manages everything including other admins.
        Role::Administrator => &[
            SystemAdmin,
            UsersRead,
            UsersManage,
            RolesManage,
            EventsRead,
            EventsWrite,
            EventsDelete,
            ParticipantsRead,
            ParticipantsWrite,
            RefereesRead,
            RefereesWrite,
            FinancialsRead,
            FinancialsWrite,
            AuditRead,
        ],
        // EventDirector: full control of event operations; no financials or audit.
        Role::EventDirector => &[
            EventsRead,
            EventsWrite,
            EventsDelete,
            ParticipantsRead,
            ParticipantsWrite,
            RefereesRead,
            RefereesWrite,
        ],
        // Referee: read-only view of events and participants they are assigned to.
        Role::Referee => &[EventsRead, ParticipantsRead, RefereesRead],
        // FinanceClerk: read events for context; write financial records only.
        Role::FinanceClerk => &[EventsRead, FinancialsRead, FinancialsWrite],
        // Auditor: wide read-only — events, participants, financials, audit trail.
        // No write access anywhere.
        Role::Auditor => &[
            UsersRead,
            EventsRead,
            ParticipantsRead,
            FinancialsRead,
            AuditRead,
        ],
    }
}

// ── Permission guard macro ────────────────────────────────────────────────────

/// Generates a typed Rocket request guard that:
/// 1. Validates the caller's session (delegates to `AuthenticatedUser::from_request`)
/// 2. Checks that the user holds the required `Permission`
/// 3. Returns HTTP 403 Forbidden if the check fails
///
/// Usage in route signatures:
/// ```rust
/// #[get("/events")]
/// async fn list_events(_p: RequireEventsRead) -> ... { ... }
/// ```
#[macro_export]
macro_rules! permission_guard {
    ($name:ident, $perm:expr) => {
        pub struct $name(pub $crate::auth::AuthenticatedUser);

        #[rocket::async_trait]
        impl<'r> rocket::request::FromRequest<'r> for $name {
            type Error = ();

            async fn from_request(
                req: &'r rocket::Request<'_>,
            ) -> rocket::request::Outcome<Self, ()> {
                use rocket::request::Outcome;
                use $crate::auth::AuthenticatedUser;

                match AuthenticatedUser::from_request(req).await {
                    Outcome::Success(user) => {
                        if user.has_permission($perm) {
                            Outcome::Success($name(user))
                        } else {
                            Outcome::Error((rocket::http::Status::Forbidden, ()))
                        }
                    }
                    Outcome::Error(e) => Outcome::Error(e),
                    Outcome::Forward(f) => Outcome::Forward(f),
                }
            }
        }
    };
}
