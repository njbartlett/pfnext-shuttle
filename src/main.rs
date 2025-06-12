// main.rs
#[macro_use]
extern crate rocket;

use std::collections::HashSet;
use std::env;
use std::fs::read_to_string;
use std::path::{Path, PathBuf};

use chrono::{DateTime, FixedOffset};

use dotenv::dotenv;

use log::{info, warn};

use rocket::fs::{relative, NamedFile};
use rocket::http::{Method, Status};
use rocket::response::status::Custom;
use rocket::{Build, Request, Rocket, State};
use rocket_cors::{AllowedHeaders, AllowedOrigins, Cors, CorsOptions};

use sqlx::postgres::PgPoolOptions;
use sqlx::Executor;

use user_agent_parser::{self, UserAgentParser};

use crate::config::{AppEnv, Config};
use crate::loginsession::AuthenticationError;

mod activities;
mod backup;
mod bookings;
mod config;
mod transaction_log;
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
pub fn notfound(_request: &Request) -> Custom<String> {
    Custom(Status::NotFound, "not found".to_string())
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

    // Configure Rocket
    rocket::build()
        .manage(config)
        .manage(app_env)
        .manage(pool)
        .manage(user_agent_parser)
        .mount("/", routes![static_files])
        .register("/api", catchers![unauthorized, notfound])
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
                transaction_log::read_log,
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

fn _configure_cors(app_env: &AppEnv) -> Cors {
    let allowed_origins = AllowedOrigins::some_regex::<&String>(&[&app_env.cors_allowed]);
    println!("Initializing CORS with allowed domain(s): {:?}", &allowed_origins);
    CorsOptions {
        allowed_origins,
        allowed_methods: vec![Method::Get, Method::Post, Method::Options, Method::Head, Method::Delete, Method::Put, Method::Patch]
            .into_iter()
            .map(From::from)
            .collect(),
        allowed_headers: AllowedHeaders::All,
        expose_headers: HashSet::from(["Location".to_string()]),
        allow_credentials: true,
        ..Default::default()
    }
    .to_cors()
    .map_err(|e| format!("Failed to create CORS options: {}", e))
    .unwrap()
}
