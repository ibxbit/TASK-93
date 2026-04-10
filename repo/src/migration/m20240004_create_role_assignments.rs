use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(RoleAssignments::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(RoleAssignments::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(RoleAssignments::UserId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RoleAssignments::RoleId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RoleAssignments::AssignedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    // NULL when seeded by the system; set to admin's user_id otherwise.
                    .col(
                        ColumnDef::new(RoleAssignments::AssignedBy)
                            .big_integer()
                            .null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_role_assignments_user_id")
                            .from(RoleAssignments::Table, RoleAssignments::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_role_assignments_role_id")
                            .from(RoleAssignments::Table, RoleAssignments::RoleId)
                            .to(Roles::Table, Roles::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Enforce one assignment per (user, role) pair.
        manager
            .create_index(
                Index::create()
                    .name("idx_role_assignments_user_role")
                    .table(RoleAssignments::Table)
                    .col(RoleAssignments::UserId)
                    .col(RoleAssignments::RoleId)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // Efficient lookup of all roles for a user (called on every auth request).
        manager
            .create_index(
                Index::create()
                    .name("idx_role_assignments_user_id")
                    .table(RoleAssignments::Table)
                    .col(RoleAssignments::UserId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(RoleAssignments::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum RoleAssignments {
    Table,
    Id,
    UserId,
    RoleId,
    AssignedAt,
    AssignedBy,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Roles {
    Table,
    Id,
}
