use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(RulesetVersions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(RulesetVersions::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    // SemVer string e.g. "2.1.0" — uniquely identifies a published ruleset.
                    .col(
                        ColumnDef::new(RulesetVersions::SemanticVersion)
                            .string_len(32)
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(RulesetVersions::Description).text().null())
                    .col(
                        ColumnDef::new(RulesetVersions::EffectiveAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RulesetVersions::CreatedBy)
                            .big_integer()
                            .not_null(),
                    )
                    // Points to the version being reverted; NULL for normal releases.
                    .col(
                        ColumnDef::new(RulesetVersions::RollbackOf)
                            .big_integer()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(RulesetVersions::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_ruleset_versions_created_by")
                            .from(RulesetVersions::Table, RulesetVersions::CreatedBy)
                            .to(Users::Table, Users::Id),
                    )
                    // Self-referencing FK: rollback chain audit trail.
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_ruleset_versions_rollback_of")
                            .from(RulesetVersions::Table, RulesetVersions::RollbackOf)
                            .to(RulesetVersions::Table, RulesetVersions::Id),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(RulesetVersions::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum RulesetVersions {
    Table,
    Id,
    SemanticVersion,
    Description,
    EffectiveAt,
    CreatedBy,
    RollbackOf,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
