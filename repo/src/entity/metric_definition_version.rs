use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "metric_definition_versions")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub metric_id: i64,
    pub version: i32,
    pub definition: String,
    pub changed_by: Option<i64>,
    pub change_reason: Option<String>,
    pub created_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::metric_definition::Entity",
        from = "Column::MetricId",
        to = "super::metric_definition::Column::Id"
    )]
    MetricDefinition,
}

impl Related<super::metric_definition::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::MetricDefinition.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
