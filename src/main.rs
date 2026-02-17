use axum::{
    Router,
    extract::DefaultBodyLimit,
    routing::{get, post},
};
use sqlx::{Pool, Row, Sqlite, sqlite::SqlitePoolOptions};
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;

use crate::schema::AppState;
mod routes;
mod schema;

async fn ensure_db() -> Result<Pool<Sqlite>, sqlx::Error> {
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:updater.db".to_string());

    // Create the database file if it doesn't exist
    if !std::path::Path::new("updater.db").exists() {
        std::fs::File::create("updater.db")?;
    }

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    // Run migrations (create table if not exists)
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS releases (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            app_name TEXT NOT NULL,
            target TEXT NOT NULL,
            arch TEXT NOT NULL,
            version TEXT NOT NULL,
            url TEXT NOT NULL,
            signature TEXT NOT NULL,
            pub_date TEXT NOT NULL,
            notes TEXT NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await?;

    // Seed some data for testing if empty
    let count: i64 = sqlx::query("SELECT count(*) FROM releases")
        .fetch_one(&pool)
        .await?
        .get(0);

    if count == 0 {
        println!("Seeding database with dummy data");
        sqlx::query(
            r#"
            INSERT INTO releases (app_name, target, arch, version, url, signature, pub_date, notes)
            VALUES
            ('classprime', 'darwin', 'aarch64', '1.0.1', 'https://github.com/user/repo/releases/download/v1.0.1/app-aarch64.app.tar.gz', 'sig123', '2024-01-01T12:00:00Z', 'Initial release'),
            ('classprime', 'darwin', 'x86_64', '1.0.1', 'https://github.com/user/repo/releases/download/v1.0.1/app-x64.app.tar.gz', 'sig123', '2024-01-01T12:00:00Z', 'Initial release'),
            ('classfi', 'windows', 'x86_64', '1.0.1', 'https://github.com/user/repo/releases/download/v1.0.1/app-setup.exe', 'sig123', '2024-01-01T12:00:00Z', 'Initial release')
            "#,
        )
        .execute(&pool)
        .await?;
    }
    return Ok(pool);
}

use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[derive(OpenApi)]
#[openapi(
    paths(
        routes::check_update,
        routes::upload_release,
        routes::get_latest_version,
        routes::download_latest_release,
        routes::get_releases,
        routes::root
    ),
    components(
        schemas(schema::Release, schema::UpdateResponse, schema::UploadReleaseForm, schema::SupportedApp, schema::SupportedTarget)
    ),
    tags(
        (name = "updater", description = "Updater API")
    )
)]
struct ApiDoc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = ensure_db().await?;
    let state = AppState { pool };

    let app = Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .route("/", get(routes::root))
        .route("/releases", get(routes::get_releases))
        .route(
            "/{app_name}/{target}/{arch}/{current_version}",
            get(routes::check_update),
        )
        .route(
            "/latest/{app_name}/{target}/{arch}",
            get(routes::get_latest_version),
        )
        .route(
            "/download/latest/{app_name}/{target}/{arch}",
            get(routes::download_latest_release),
        )
        .route("/upload", post(routes::upload_release))
        .layer(DefaultBodyLimit::disable())
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    println!("listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
