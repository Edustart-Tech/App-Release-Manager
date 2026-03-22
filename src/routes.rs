use crate::schema::{
    AppState, Release, SupportedApp, SupportedTarget, UpdateResponse, UploadReleaseForm,
};
use axum::extract::Multipart;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use semver::Version;

/// Check for updates
#[utoipa::path(
    get,
    path = "/{app_name}/{target}/{arch}/{current_version}",
    params(
        ("app_name" = SupportedApp, Path, description = "Application name"),
        ("target" = SupportedTarget, Path, description = "Target OS"),
        ("arch" = String, Path, description = "Architecture (e.g., aarch64, x86_64)"),
        ("current_version" = String, Path, description = "Current version of the application")
    ),
    responses(
        (status = 200, description = "Update available", body = UpdateResponse),
        (status = 204, description = "No update available"),
        (status = 400, description = "Bad request (invalid version format)")
    )
)]
pub async fn check_update(
    Path((app_name, target, arch, current_version)): Path<(String, String, String, String)>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    println!("🚀 Incoming Update Check:");
    println!("   AppName: {}", app_name);
    println!("   Target:  {}", target);
    println!("   Arch:    {}", arch);
    println!("   Version: {}", current_version);

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
    println!(
        "✅ Update available for {} {} {}: {} -> {}",
        app_name, target, arch, current_version, v
    );
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
        "ℹ️  No update available for {} {} {} (current: {})",
        app_name, target, arch, current_version
    );
    // No update available
    (StatusCode::NO_CONTENT, Json(None))
}

/// Upload a new release
#[utoipa::path(
    post,
    path = "/upload",
    request_body(content = UploadReleaseForm, content_type = "multipart/form-data"),
    responses(
        (status = 201, description = "Release created successfully", body = String),
        (status = 400, description = "Bad request"),
        (status = 409, description = "Conflict - Asset already exists"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn upload_release(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let mut app_name = String::new();
    let mut version = String::new();
    let mut target = String::new();
    let mut arch = String::new();
    let mut notes = String::new();
    let mut signature = String::new();
    let mut file_data: Vec<u8> = Vec::new();
    let mut file_name = String::new();

    println!("Starting upload_release handler...");

    // 1. Extract fields and file from multipart
    while let Some(res) = multipart.next_field().await.transpose() {
        let field = match res {
            Ok(f) => f,
            Err(e) => {
                println!("Error processing multipart field: {:?}", e);
                continue;
            }
        };

        let name = field.name().unwrap_or_default().to_string();
        match name.as_str() {
            "app_name" => app_name = field.text().await.unwrap_or_default(),
            "version" => version = field.text().await.unwrap_or_default(),
            "target" => target = field.text().await.unwrap_or_default(),
            "arch" => arch = field.text().await.unwrap_or_default(),
            "notes" => notes = field.text().await.unwrap_or_default(),
            "signature" => signature = field.text().await.unwrap_or_default(),
            "file" => {
                file_name = field.file_name().unwrap_or("installer").to_string();
                let content_type = field.content_type().unwrap_or("unknown");
                println!(
                    "Processing file field: name={}, type={}",
                    file_name, content_type
                );

                match field.bytes().await {
                    Ok(bytes) => {
                        println!("Received file: {}, size: {} bytes", file_name, bytes.len());
                        file_data = bytes.to_vec();
                    }
                    Err(e) => {
                        println!("Error reading file bytes: {:?}", e);
                        return (
                            StatusCode::BAD_REQUEST,
                            format!("Failed to read file: {}", e),
                        )
                            .into_response();
                    }
                }
            }
            _ => (),
        }
    }

    if file_data.is_empty() {
        println!("Warning: No file data received or file is empty!");
        return (StatusCode::BAD_REQUEST, "No file uploaded or file is empty").into_response();
    }

    println!(
        "Extracted fields: app_name={}, version={}, target={}, arch={}",
        app_name, version, target, arch
    );

    // 2. GitHub Integration (Octocrab)
    println!("Initializing GitHub client...");
    let token = std::env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN must be set");
    let octo = octocrab::Octocrab::builder()
        .personal_token(token)
        .build()
        .unwrap();
    let owner = std::env::var("GITHUB_OWNER").unwrap_or_else(|_| "Edustart-Tech".into());
    let repo = std::env::var("GITHUB_REPO").unwrap_or_else(|_| "App-Release-Manager".into());
    let tag = format!("{}-v{}", app_name, version);

    println!("Checking if release tag {} exists...", tag);
    let release = match octo.repos(&owner, &repo).releases().get_by_tag(&tag).await {
        Ok(r) => {
            println!("Tag {} exists. Checking for asset conflict...", tag);
            // Check if asset exists
            if r.assets.iter().any(|a| a.name == file_name) {
                println!(
                    "Conflict: Asset {} already exists in release {}",
                    file_name, tag
                );
                return (StatusCode::CONFLICT, "Asset already exists in this release")
                    .into_response();
            }
            println!("Release {} ready for upload.", tag);
            r
        }
        Err(_) => {
            println!("Release not found, creating new release for tag {}...", tag);
            match octo
                .repos(&owner, &repo)
                .releases()
                .create(&tag)
                .name(&tag)
                .body(&notes)
                .send()
                .await
            {
                Ok(r) => {
                    println!("GitHub release created successfully: id={}", r.id);
                    r
                }
                Err(e) => {
                    println!("Failed to create GitHub release: {:?}", e);
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("GH Release Fail: {:?}", e),
                    )
                        .into_response();
                }
            }
        }
    };

    println!("Uploading asset to GitHub release...");

    // Upload the Asset
    let asset = match octo
        .repos(&owner, &repo)
        .releases()
        .upload_asset(*release.id, &file_name, file_data.clone().into())
        .send()
        .await
    {
        Ok(a) => {
            println!(
                "Asset uploaded successfully: url={}",
                a.browser_download_url
            );
            a
        }
        Err(e) => {
            println!("Failed to upload asset: {:?}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("GH Upload Fail: {:?}", e),
            )
                .into_response();
        }
    };

    let github_download_url = asset.browser_download_url.to_string();
    let mut final_download_url = github_download_url.clone();

    // 3. Mega.io S4 Integration
    let s4_endpoint = std::env::var("S4_ENDPOINT").unwrap_or_default();
    let s4_bucket_name = std::env::var("S4_BUCKET").unwrap_or_default();
    let s4_access_key = std::env::var("S4_ACCESS_KEY").unwrap_or_default();
    let s4_secret_key = std::env::var("S4_SECRET_KEY").unwrap_or_default();
    let s4_region = std::env::var("S4_REGION").unwrap_or_else(|_| "auto".to_string());

    if !s4_endpoint.is_empty() && !s4_bucket_name.is_empty() {
        println!("Uploading to S4 object storage...");
        let region = s3::region::Region::Custom {
            region: s4_region,
            endpoint: s4_endpoint.clone(),
        };

        match s3::creds::Credentials::new(
            Some(&s4_access_key),
            Some(&s4_secret_key),
            None,
            None,
            None,
        ) {
            Ok(credentials) => {
                match s3::bucket::Bucket::new(&s4_bucket_name, region, credentials) {
                    Ok(mut bucket) => {
                        bucket = bucket.with_path_style();
                        let s4_key =
                            format!("{}/{}/{}/{}/{}", app_name, version, target, arch, file_name);

                        match bucket.put_object(&s4_key, &file_data).await {
                            Ok(res) => {
                                println!(
                                    "S4 Upload successful! Status code: {}",
                                    res.status_code()
                                );

                                // Construct the public URL
                                // Using the endpoint which includes the account ID as the user mentioned
                                let endpoint_trimmed = s4_endpoint.trim_end_matches('/');
                                let public_url =
                                    format!("{}/{}/{}", endpoint_trimmed, s4_bucket_name, s4_key);
                                println!("S4 Download URL: {}", public_url);

                                // Allow an explicit override for the public URL base if provided
                                let override_base =
                                    std::env::var("S4_PUBLIC_URL_BASE").unwrap_or_default();
                                if !override_base.is_empty() {
                                    final_download_url = format!(
                                        "{}/{}/{}",
                                        override_base.trim_end_matches('/'),
                                        s4_bucket_name,
                                        s4_key
                                    );
                                } else {
                                    final_download_url = public_url;
                                }
                            }
                            Err(e) => {
                                println!("S4 Upload failed: {:?}", e);
                            }
                        }

                        // Upload changelogs.txt if notes are provided
                        if !notes.trim().is_empty() {
                            let changelog_key = format!(
                                "{}/{}/{}/{}/changelogs.txt",
                                app_name, version, target, arch
                            );
                            println!("Uploading changelog to S4: {}", changelog_key);
                            match bucket.put_object(&changelog_key, notes.as_bytes()).await {
                                Ok(res) => println!(
                                    "S4 Changelog upload successful! Status code: {}",
                                    res.status_code()
                                ),
                                Err(e) => println!("S4 Changelog upload failed: {:?}", e),
                            }
                        }
                    }
                    Err(e) => println!("Failed to initialize S4 Bucket: {:?}", e),
                }
            }
            Err(e) => println!("Failed to initialize S4 Credentials: {:?}", e),
        }
    }

    // 4. Save to Database
    println!("Saving release to local database...");
    let pub_date = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT OR REPLACE INTO releases (app_name, target, arch, version, url, signature, pub_date, notes) VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&app_name).bind(&target).bind(&arch).bind(&version)
    .bind(&final_download_url).bind(&signature).bind(&pub_date).bind(&notes)
    .execute(&state.pool).await.unwrap();

    println!("Release process completed successfully.");
    (
        StatusCode::CREATED,
        axum::response::Json(final_download_url),
    )
        .into_response()
}

/// Get the latest version
#[utoipa::path(
    get,
    path = "/latest/{app_name}/{target}/{arch}",
    params(
        ("app_name" = SupportedApp, Path, description = "Application name"),
        ("target" = SupportedTarget, Path, description = "Target OS"),
        ("arch" = String, Path, description = "Architecture")
    ),
    responses(
        (status = 200, description = "Latest version found", body = UpdateResponse),
        (status = 204, description = "No version found")
    )
)]
// Handler to get the latest version (without update check logic)
pub async fn get_latest_version(
    Path((app_name, target, arch)): Path<(String, String, String)>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    println!("🔍 Latest Version Check:");
    println!("   AppName: {}", app_name);
    println!("   Target:  {}", target);
    println!("   Arch:    {}", arch);

    // Fetch all releases for this app/target/arch
    let releases = sqlx::query_as::<_, Release>(
        "SELECT id, app_name, target, arch, version, url, signature, pub_date, notes FROM releases WHERE app_name = ? AND target = ? AND arch = ?"
    )
    .bind(&app_name)
    .bind(&target)
    .bind(&arch)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_else(|_| vec![]);

    // Find the latest version
    let latest_release = releases
        .into_iter()
        .filter_map(|r| {
            let v = Version::parse(&r.version).ok()?;
            Some((v, r))
        })
        .max_by(|(v1, _), (v2, _)| v1.cmp(v2));

    if let Some((_, release)) = latest_release {
        let response = UpdateResponse {
            version: release.version,
            url: release.url,
            signature: release.signature,
            pub_date: release.pub_date,
            notes: release.notes,
        };
        return (StatusCode::OK, Json(Some(response)));
    }

    (StatusCode::NO_CONTENT, Json(None))
}

/// Download the latest release
#[utoipa::path(
    get,
    path = "/download/latest/{app_name}/{target}/{arch}",
    params(
        ("app_name" = SupportedApp, Path, description = "Application name"),
        ("target" = SupportedTarget, Path, description = "Target OS"),
        ("arch" = String, Path, description = "Architecture")
    ),
    responses(
        (status = 307, description = "Redirect to download URL"),
        (status = 404, description = "No release found")
    )
)]
// Handler to download the latest release (redirect)
pub async fn download_latest_release(
    Path((app_name, target, arch)): Path<(String, String, String)>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    println!(
        "Received latest download request: app_name={}, target={}, arch={}",
        app_name, target, arch
    );

    // Fetch all releases for this app/target/arch
    let releases = sqlx::query_as::<_, Release>(
        "SELECT id, app_name, target, arch, version, url, signature, pub_date, notes FROM releases WHERE app_name = ? AND target = ? AND arch = ?"
    )
    .bind(&app_name)
    .bind(&target)
    .bind(&arch)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_else(|_| vec![]);

    // Find the latest version
    let latest_release = releases
        .into_iter()
        .filter_map(|r| {
            let v = Version::parse(&r.version).ok()?;
            Some((v, r))
        })
        .max_by(|(v1, _), (v2, _)| v1.cmp(v2));

    if let Some((_, release)) = latest_release {
        println!("Redirecting to: {}", release.url);
        return axum::response::Redirect::temporary(&release.url).into_response();
    }

    (StatusCode::NOT_FOUND, "No release found").into_response()
}

/// Root endpoint
#[utoipa::path(
    get,
    path = "/",
    responses(
        (status = 200, description = "Service is running", body = String)
    )
)]
// Handler for the root route
pub async fn root() -> &'static str {
    "Updater Service Running"
}

/// Get all releases
#[utoipa::path(
    get,
    path = "/releases",
    responses(
        (status = 200, description = "List of all releases", body = Vec<Release>)
    )
)]
pub async fn get_releases(State(state): State<AppState>) -> impl IntoResponse {
    let releases = sqlx::query_as::<_, Release>(
        "SELECT id, app_name, target, arch, version, url, signature, pub_date, notes FROM releases ORDER BY pub_date DESC"
    )
    .fetch_all(&state.pool)
    .await
    .unwrap_or_else(|_| vec![]);

    let mut buf = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(b"    ");
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
    match serde::Serialize::serialize(&releases, &mut ser) {
        Ok(_) => {
            let json_string = String::from_utf8(buf).unwrap_or_default();
            (
                StatusCode::OK,
                [("content-type", "application/json")],
                json_string,
            )
                .into_response()
        }
        Err(e) => {
            println!("Error serializing releases: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to serialize releases",
            )
                .into_response()
        }
    }
}
