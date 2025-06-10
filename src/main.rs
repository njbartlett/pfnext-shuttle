// main.rs
#[macro_use]
extern crate rocket;

use std::collections::HashSet;
use std::env;
use std::path::{Path, PathBuf};

use chrono::{DateTime, FixedOffset};

use dotenv::dotenv;

use rocket::fs::{relative, NamedFile};
use rocket::http::{Method, Status};
use rocket::response::status::Custom;
use rocket::{Build, Request, Rocket, State};
use rocket_cors::{AllowedHeaders, AllowedOrigins};

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

use crate::config::{AppEnv, Config};
use crate::loginsession::AuthenticationError;

mod activities;
mod backup;
mod bookings;
mod config;
mod log;
mod loginsession;
mod mock_chrono;
mod sessions;
mod users;
mod whereclause;

#[rocket::get("/<path..>")]
async fn static_files(
    app_env: &State<AppEnv>,
    path: PathBuf
) -> Option<NamedFile> {
    let root = relative!("/");
    let mut path = Path::new(root).join(&app_env.static_path).join(path);
    if path.is_dir() {
        path.push("index.html");
    }

    NamedFile::open(path).await.ok()
}
#[catch(401)]
pub fn unauthorized(request: &Request) -> Custom<String> {
    let auth_error = request.local_cache::<Option<AuthenticationError>, _>(|| None);
    info!("### Intercepted 401 return, auth error: {:?}", auth_error);
    match auth_error {
        Some(err) => match err {
            AuthenticationError::MissingSession => Custom(
                Status::Unauthorized,
                "Your login session has expired".to_string(),
            ),
            AuthenticationError::MissingDatabase => Custom(
                Status::InternalServerError,
                "Failed to validate session".to_string(),
            ),
            AuthenticationError::DatabaseError(err) => {
                Custom(Status::InternalServerError, err.to_string())
            }
        },
        None => Custom(
            Status::InternalServerError,
            "Failed to authenticate user".to_string(),
        ),
    }
}

#[catch(404)]
pub fn notfound(request: &Request) -> Custom<String> {
    Custom(Status::NotFound, "not found".to_string())
}

async fn create_pool() -> PgPool {
    dotenv().expect("Failed to load .env properties");
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to create pool")
}

#[launch]
async fn launch() -> Rocket<Build> {
    dotenv().ok();

    // Load config
    let config = Config::load().expect("Failed to load config properties");
    info!("Loaded configuration: {:?}", config);
    let app_env = AppEnv::new_from_env().expect("Failed to load application environment");
    info!("Loaded application environment");

    // Start DB connection pool
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&app_env.database_url)
        .await
        .expect("Failed to create pool");

    // Configure CORS
    info!(
        "Initializing CORS with allowed origin regex: {}",
        &app_env.cors_allowed
    );
    let allow_domain = [&app_env.cors_allowed];
    let allowed_origins = AllowedOrigins::some_regex(&allow_domain);
    let cors = rocket_cors::CorsOptions {
        allowed_origins,
        allowed_methods: vec![
            Method::Get,
            Method::Post,
            Method::Options,
            Method::Head,
            Method::Delete,
            Method::Put,
            Method::Patch,
        ]
        .into_iter()
        .map(From::from)
        .collect(),
        allowed_headers: AllowedHeaders::All,
        expose_headers: HashSet::from(["Location".to_string()]),
        allow_credentials: true,
        ..Default::default()
    }
    .to_cors()
    .expect("Failed to create cors options");

    // Configure Rocket
    rocket::build()
        .attach(cors)
        .manage(config)
        .manage(app_env)
        .manage(pool)
        .mount("/", routes![static_files])
        .register("/api", catchers![unauthorized, notfound]) // TODO forbidden
        .mount(
            "/api",
            routes![
                loginsession::login,
                loginsession::logout,
                loginsession::verify_session,
                users::register_user,
                users::request_pwd_reset,
                users::reset_pwd,
                users::get_user,
                users::list_users,
                users::delete_user,
                users::update_user,
                users::patch_user,
                sessions::list_sessions,
                sessions::get_session,
                sessions::create_session,
                sessions::delete_session,
                sessions::list_locations,
                sessions::list_session_types,
                sessions::update_session,
                bookings::list_bookings,
                bookings::create_booking,
                bookings::delete_booking,
                bookings::update_booking,
                bookings::get_attendance_stats,
                activities::list_activity_types,
                activities::list_challenges,
                activities::get_challenge,
                activities::get_activity,
                activities::list_activities,
                activities::create_activity,
                activities::delete_activity,
                log::read_log,
                backup::backup_all
            ],
        )
}

fn parse_opt_date(str: Option<String>) -> Result<Option<DateTime<FixedOffset>>, Custom<String>> {
    if str.is_none() {
        return Ok(None);
    }
    let parsed = DateTime::parse_from_rfc3339(str.as_ref().unwrap());
    Ok(Some(parsed.map_err(|e| {
        Custom(Status::UnprocessableEntity, e.to_string())
    })?))
}
