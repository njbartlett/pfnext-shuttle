// main.rs
#[macro_use]
extern crate rocket;

mod claims;
mod sessions;
mod persons;
mod login;

use claims::Claims;

use rocket::fs::NamedFile;
use rocket::fs::{relative};
use rocket::http::{Header, Method, Status};
use rocket::response::status::{BadRequest, Custom};
use rocket::serde::json::Json;
use rocket_cors::{AllowedHeaders, AllowedMethods, AllowedOrigins};

use serde::{Deserialize, Serialize};
use sqlx::{Executor, FromRow, PgPool, query_as, QueryBuilder};
use std::path::{Path, PathBuf};
use itertools::Itertools;
use password_auth::{generate_hash, verify_password};
use rocket::{Request, Response, State};
use rocket::http::StatusClass::ServerError;
use rocket::response::Responder;
use shuttle_runtime::CustomError;
use sqlx::postgres::PgQueryResult;

struct AppState {
    pool: PgPool,
}

#[derive(Serialize)]
struct PrivateResponse {
    message: String,
    user: String,
}

#[get("/private")]
fn private(user: Claims) -> Json<PrivateResponse> {
    Json(PrivateResponse {
        message: "The `Claims` request guard ensures only valid JWTs can access this endpoint".to_string(),
        user: user.email,
    })
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
            static_files,
            login::login,
            login::create_user,
            private,
            sessions::list_sessions, sessions::list_sessions_by_date,
            sessions::book_session, sessions::cancel_booking
        ])
        .manage(state);

    Ok(rocket.into())
}
