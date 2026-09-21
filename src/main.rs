// main.rs
#[macro_use]
extern crate rocket;

use std::fs::read_to_string;
use std::path::Path;

use dotenv::dotenv;

use log::info;

use rocket::http::{Method, Status};
use rocket::{Build, Request, Rocket, Route};
use rocket_cors::{AllowedHeaders, AllowedOrigins, CorsOptions};

use sqlx::postgres::PgPoolOptions;
use sqlx::Executor;

use user_agent_parser::{self, UserAgentParser};

use crate::apierror::ApiError;
use crate::config::{AppEnv, Config};
use crate::loginsession::AuthenticationError;

mod activities;
mod apierror;
mod backup;
mod blog;
mod bookings;
mod common;
mod config;
mod transaction_log;
mod loginsession;
mod mock_chrono;
mod notifications;
mod polls;
mod sessions;
mod templates;
mod testcommon;
mod users;
mod whereclause;

#[catch(401)]
fn api_unauthorized(request: &Request) -> ApiError {
    let auth_error = request.local_cache::<Option<AuthenticationError>, _>(|| None);
    match auth_error {
        Some(err) => match err {
            AuthenticationError::MissingSession => ApiError::new(
                Status::Unauthorized,
                "Your login session has expired".to_string(),
            ),
            AuthenticationError::MissingDatabase => ApiError::new(
                Status::InternalServerError,
                "Failed to validate session".to_string(),
            ),
            AuthenticationError::DatabaseError(err) => {
                ApiError::new(Status::InternalServerError, err.to_string())
            }
        },
        None => ApiError::new(
            Status::InternalServerError,
            "Failed to authenticate user".to_string(),
        ),
    }
}

#[catch(404)]
fn api_notfound(_: &Request) -> ApiError {
    ApiError::new(Status::NotFound, "not found".to_string())
}

#[launch]
async fn launch() -> Rocket<Build> {
    dotenv().ok();
    env_logger::init();

    // Load config
    let config = Config::load().expect("Failed to load config properties");
    info!("Loaded configuration: {:?}", config);
    let app_env = AppEnv::new_from_env().expect("Failed to load application environment");
    info!("Loaded application environment");

    // Load users agents config
    let user_agent_parser = UserAgentParser::from_path("user_agents.yaml")
        .map_err(|e| format!("Failed to load User-Agents config: {}", e))
        .unwrap();

    // Start DB connection pool
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&app_env.database_url)
        .await
        .expect("Failed to create pool");

    // Import Schema
    let schema_path = Path::new("schema.sql");
    let schema = read_to_string(schema_path)
        .map_err(|e| format!("Failed to open DB schema file {:?}: {}", schema_path, e))
        .unwrap();
    info!("Loaded schema from {:?}", schema_path);
    pool.execute(schema.as_str()).await
        .map_err(|e| format!("Failed to import DB schema: {}", e))
        .unwrap();
    info!("Imported schema into database.");

    // Configure CORS for non-same-origin clients (e.g. the mobile app's
    // capacitor://localhost and http://localhost origins), from the
    // comma-separated CORS_ALLOWED environment variable
    let cors = build_cors(&app_env.cors_allowed);

    let api_routes = vec![
        activities::routes(),
        bookings::routes(),
        loginsession::routes(),
        sessions::routes(),
        users::routes(),
        transaction_log::routes(),
        backup::routes(),
        polls::routes(),
        blog::api_routes(),
    ].into_iter().flatten().collect::<Vec<Route>>();

    // Configure Rocket
    rocket::build()
        .manage(config)
        .manage(app_env)
        .manage(pool)
        .manage(user_agent_parser)
        .attach(cors)
        // The single-page app (web/dist); page paths with no file fall back
        // to the SPA shell, and blog post pages get their Open Graph tags
        .mount("/", crate::templates::routes())
        .mount("/", blog::page_routes())
        // Images embedded in posts keep their historical URLs
        .mount("/blog", blog::blob_routes())
        // The "/api" catchers also cover the longer "/api/v1" prefix
        .register("/api", catchers![api_unauthorized, api_notfound])
        // Unversioned mount consumed by the website; "/api/v1" is the frozen
        // contract for mobile clients, which cannot be updated in lockstep
        .mount("/api", api_routes.clone())
        .mount("/api/v1", api_routes)
}

fn build_cors(cors_allowed: &str) -> rocket_cors::Cors {
    // CORS_ALLOWED entries (comma-separated, so no commas within an entry):
    //  - starting with "^": a regex matched against the whole Origin, e.g.
    //    ^https?://localhost(:\d+)?$ for the local dev server on any port.
    //    Browsers send an Origin header on all POST/PUT/PATCH/DELETE requests,
    //    including same-origin ones, so the site's own origin must be allowed
    //    or logins and bookings from the website itself get rejected.
    //  - http(s) origins: matched exactly.
    //  - other schemes (e.g. capacitor://localhost on iOS): "opaque" to the URL
    //    parser and rejected by rocket_cors in the exact list, so matched via an
    //    escaped regex instead.
    let entries: Vec<&str> = cors_allowed
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    let (regex, exact): (Vec<&str>, Vec<&str>) = entries.iter()
        .partition(|o| o.starts_with('^') || !(o.starts_with("http://") || o.starts_with("https://")));
    let regex_patterns: Vec<String> = regex.iter()
        .map(|o| if o.starts_with('^') {
            o.to_string()
        } else {
            format!("^{}$", o.replace('.', "\\."))
        })
        .collect();
    CorsOptions {
        allowed_origins: AllowedOrigins::some(&exact, &regex_patterns),
        allowed_methods: [Method::Get, Method::Post, Method::Put, Method::Patch, Method::Delete, Method::Options]
            .into_iter().map(From::from).collect(),
        allowed_headers: AllowedHeaders::some(&["Authorization", "Content-Type", "Accept"]),
        allow_credentials: true,
        ..Default::default()
    }
    .to_cors()
    .expect("Failed to build CORS fairing")
}

#[cfg(test)]
mod tests {
    use super::build_cors;

    #[test]
    fn test_build_cors_accepts_mixed_entries() {
        // Exact http(s) origins, an opaque scheme, and a raw regex must all build
        build_cors("https://anotherlevelfitness.uk, capacitor://localhost, ^https?://localhost(:\\d+)?$");
        build_cors("");
    }
}
