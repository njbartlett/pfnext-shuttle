// main.rs
#[macro_use]
extern crate rocket;

use std::path::{Path, PathBuf};

use rocket::fs::NamedFile;
use rocket::fs::relative;
use rocket::http::Method;
use rocket_cors::{AllowedHeaders, AllowedOrigins};
use shuttle_runtime::CustomError;
use sqlx::{Executor, PgPool};

mod claims;
mod sessions;
mod login;

struct AppState {
    pool: PgPool,
}

#[rocket::get("/<path..>")]
pub async fn static_files(mut path: PathBuf) -> Option<NamedFile> {
    path.set_extension("html");
    let mut path = Path::new(relative!("assets")).join(path);
    if path.is_dir() {
        path.push("index.html");
    }

    NamedFile::open(path).await.ok()
}

#[shuttle_runtime::main]
async fn rocket(#[shuttle_shared_db::Postgres] pool: PgPool) -> shuttle_rocket::ShuttleRocket {
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
    let state = AppState { pool };
    let rocket = rocket::build()
        .attach(cors)
        .mount("/", routes![
            login::login, login::change_password, login::create_user,
            sessions::list_sessions, sessions::list_sessions_by_date,
            sessions::list_bookings, sessions::book_session, sessions::cancel_booking
        ])
        .manage(state);

    Ok(rocket.into())
}
