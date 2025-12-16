use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub email: String,
    #[sea_orm(unique)]
    pub username: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::feed::Entity")]
    Feeds,
    #[sea_orm(has_many = "super::feed_like::Entity")]
    FeedLikes,
}

impl Related<super::feed::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Feeds.def()
    }
}

impl Related<super::feed_like::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::FeedLikes.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
