use serde::Serialize;
use sqlx::{Pool, Sqlite, prelude::FromRow};

#[derive(Clone)]
pub struct AppState {
    pub pool: Pool<Sqlite>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct Release {
    pub id: i64,
    pub app_name: String,
    pub target: String,
    pub arch: String,
    pub version: String,
    pub url: String,
    pub signature: String,
    pub pub_date: String,
    pub notes: String,
}

#[derive(Debug, Serialize)]
pub struct UpdateResponse {
    pub version: String,
    pub url: String,
    pub signature: String,
    pub pub_date: String,
    pub notes: String,
}
