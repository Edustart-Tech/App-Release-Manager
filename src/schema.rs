use serde::{Deserialize, Serialize};
use sqlx::{Pool, Sqlite, prelude::FromRow};

#[derive(Clone)]
pub struct AppState {
    pub pool: Pool<Sqlite>,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum SupportedApp {
    Classprime,
    Classfi,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum SupportedTarget {
    Darwin,
    Windows,
}

#[derive(Debug, Serialize, FromRow, utoipa::ToSchema)]
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

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct UpdateResponse {
    pub version: String,
    pub url: String,
    pub signature: String,
    pub pub_date: String,
    pub notes: String,
}

#[derive(Debug, utoipa::ToSchema)]
pub struct UploadReleaseForm {
    #[schema(example = "classprime")]
    pub app_name: String,
    #[schema(example = "1.0.1")]
    pub version: String,
    #[schema(example = "darwin")]
    pub target: String,
    #[schema(example = "aarch64")]
    pub arch: String,
    #[schema(example = "Release notes")]
    pub notes: String,
    #[schema(example = "signature")]
    pub signature: String,
    #[schema(value_type = String, format = Binary)]
    pub file: Vec<u8>,
}
