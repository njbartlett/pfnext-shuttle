// main.rs
#[macro_use]
extern crate rocket;

use std::path::{Path, PathBuf};
use chrono::{DateTime, FixedOffset};

use rand::prelude::*;
use rocket::fs::NamedFile;
use rocket::fs::relative;
use rocket::http::{Method, Status};
use rocket::response::status::Custom;
use rocket::serde::Serialize;
use rocket_cors::{AllowedHeaders, AllowedOrigins};
use shuttle_runtime::CustomError;
use sqlx::{Executor, FromRow, PgPool};

mod claims;
mod sessions;
mod login;
mod bookings;

struct AppState {
    pool: PgPool,
    secrets: shuttle_runtime::SecretStore
}

#[rocket::get("/<path..>")]
pub async fn static_files(mut path: PathBuf) -> Option<NamedFile> {
    //path.set_extension("html");
    let mut path = Path::new(relative!("assets")).join(path);
    if path.is_dir() {
        path.push("index.html");
    }

    NamedFile::open(path).await.ok()
}

#[shuttle_runtime::main]
async fn rocket(
    #[shuttle_shared_db::Postgres] pool: PgPool,
    #[shuttle_runtime::Secrets] secrets: shuttle_runtime::SecretStore
) -> shuttle_rocket::ShuttleRocket {
    // Initiate tables
    pool.execute(include_str!("../schema.sql"))
        .await
        .map_err(CustomError::new)?;

    // Configure CORS
    let allowed_origins = AllowedOrigins::all();
    let cors = rocket_cors::CorsOptions {
        allowed_origins,
        allowed_methods: vec![Method::Get, Method::Post, Method::Options, Method::Head, Method::Delete].into_iter().map(From::from).collect(),
        allowed_headers: AllowedHeaders::All,
        allow_credentials: true,
        ..Default::default()
    }.to_cors().map_err(CustomError::new)?;

    // Configure Rocket
    let state = AppState { pool, secrets };
    let rocket = rocket::build()
        .attach(cors)
        .mount("/", routes![
            login::login, login::validate_login, login::change_password, login::register_user, login::request_pwd_reset, login::reset_pwd, login::list_users,
            sessions::list_sessions, sessions::get_session, sessions::create_session, sessions::delete_session,
            sessions::list_locations, sessions::list_session_types, sessions::update_session,
            bookings::list_bookings, bookings::create_booking, bookings::delete_booking
        ])
        .manage(state);

    Ok(rocket.into())
}

#[derive(FromRow, Serialize)]
struct BigintRecord {
    id: i64
}

#[derive(FromRow, Serialize, Clone, Debug)]
pub struct SessionType {
    id: i32,
    name: String
}

#[derive(Serialize, Clone, Debug)]
pub struct SessionTrainer {
    id: i64,
    name: String,
    email: String
}

#[derive(FromRow, Serialize, Clone, Debug)]
pub struct SessionLocation {
    id: i32,
    name: String,
    address: String
}


fn parse_opt_date(str: Option<String>) -> Result<Option<DateTime<FixedOffset>>, Custom<String>> {
    if str.is_none() {
        return Ok(None);
    }
    let parsed = DateTime::parse_from_rfc3339(str.as_ref().unwrap());
    println!("Parsed input {:?} to {:?}", &str, parsed);
    //.map_err(|e| BadRequest(e.to_string()))?;
    Ok(Some(parsed.map_err(|e| Custom(Status::UnprocessableEntity, e.to_string()))?))
}