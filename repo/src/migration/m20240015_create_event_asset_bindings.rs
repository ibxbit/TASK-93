use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(EventAssetBindings::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(EventAssetBindings::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(EventAssetBindings::EventId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(EventAssetBindings::AssetId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(EventAssetBindings::BoundAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    // Cascade: remove bindings when the event is hard-deleted.
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_event_asset_bindings_event_id")
                            .from(EventAssetBindings::Table, EventAssetBindings::EventId)
                            .to(Events::Table, Events::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    // Restrict: cannot delete an asset that is bound to an event.
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_event_asset_bindings_asset_id")
                            .from(EventAssetBindings::Table, EventAssetBindings::AssetId)
                            .to(Assets::Table, Assets::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await?;

        // Each asset may only be bound to a given event once.
        manager
            .create_index(
                Index::create()
                    .name("idx_event_asset_bindings_event_asset")
                    .table(EventAssetBindings::Table)
                    .col(EventAssetBindings::EventId)
                    .col(EventAssetBindings::AssetId)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(EventAssetBindings::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum EventAssetBindings {
    Table,
    Id,
    EventId,
    AssetId,
    BoundAt,
}

#[derive(DeriveIden)]
enum Events {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Assets {
    Table,
    Id,
}
