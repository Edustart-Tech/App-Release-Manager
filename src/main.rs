use axum::{
    Router,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::get,
};
use semver::Version;
use serde::Serialize;
use sqlx::{FromRow, Pool, Row, Sqlite, sqlite::SqlitePoolOptions};
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;

#[derive(Clone)]
struct AppState {
    pool: Pool<Sqlite>,
}

#[derive(Debug, Serialize, FromRow)]
struct Release {
    id: i64,
    app_name: String,
    target: String,
    arch: String,
    version: String,
    url: String,
    signature: String,
    pub_date: String,
    notes: String,
}

#[derive(Debug, Serialize)]
struct UpdateResponse {
    version: String,
    url: String,
    signature: String,
    pub_date: String,
    notes: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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

    let state = AppState { pool };

    let app = Router::new()
        .route(
            "/{app_name}/{target}/{arch}/{current_version}",
            get(check_update),
        )
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

// Handler for the update check
async fn check_update(
    Path((app_name, target, arch, current_version)): Path<(String, String, String, String)>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    println!(
        "Received update check: app_name={}, target={}, arch={}, version={}",
        app_name, target, arch, current_version
    );

    let current_ver = match Version::parse(&current_version) {
        Ok(v) => v,
        Err(e) => {
            println!(
                "Failed to parse current version '{}': {}",
                current_version, e
            );
            return (StatusCode::BAD_REQUEST, Json(None));
        }
    };

    // Fetch all releases for this app/target/arch
    // We fetch all because SQLite doesn't do semver comparison easily.
    let releases = sqlx::query_as::<_, Release>(
        "SELECT id, app_name, target, arch, version, url, signature, pub_date, notes FROM releases WHERE app_name = ? AND target = ? AND arch = ?"
    )
    .bind(&app_name)
    .bind(&target)
    .bind(&arch)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_else(|_| vec![]);

    // Find the latest version > current_version
    let latest_update = releases
        .into_iter()
        .filter_map(|r| {
            let v = Version::parse(&r.version).ok()?;
            if v > current_ver {
                Some((v, r)) // Only consider newer versions
            } else {
                None
            }
        })
        .max_by(|(v1, _), (v2, _)| v1.cmp(v2)); // Find the highest version

    if let Some((v, release)) = latest_update {
        println!("Update available: {} -> {}", current_version, v);
        // Return 200 with update info
        let response = UpdateResponse {
            version: release.version,
            url: release.url,
            signature: release.signature,
            pub_date: release.pub_date,
            notes: release.notes,
        };
        return (StatusCode::OK, Json(Some(response)));
    }

    println!(
        "No update available for {} {} {} {}",
        app_name, target, arch, current_version
    );
    // No update available
    (StatusCode::NO_CONTENT, Json(None))
}
