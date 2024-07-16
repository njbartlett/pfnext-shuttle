// main.rs
#[macro_use]
extern crate rocket;

mod claims;
mod sessions;
mod persons;

use claims::Claims;

use rocket::fs::NamedFile;
use rocket::fs::{relative};
use rocket::http::{Header, Method, Status};
use rocket::response::status::Custom;
use rocket::serde::json::Json;
use rocket_cors::{AllowedHeaders, AllowedMethods, AllowedOrigins};

use serde::{Deserialize, Serialize};
use sqlx::{Executor, PgPool};
use std::path::{Path, PathBuf};
use rocket::{Request, Response};
use rocket::response::Responder;
use shuttle_runtime::CustomError;
use sqlx::postgres::PgQueryResult;

struct AppState {
    pool: PgPool,
}

#[derive(Serialize)]
struct PublicResponse {
    message: String,
}

#[get("/public")]
fn public() -> Json<PublicResponse> {
    Json(PublicResponse {
        message: "This endpoint is open to anyone".to_string(),
    })
}

#[derive(Serialize)]
struct PrivateResponse {
    message: String,
    user: String,
}

// More details on Rocket request guards can be found here
// https://rocket.rs/v0.5-rc/guide/requests/#request-guards
#[get("/private")]
fn private(user: Claims) -> Json<PrivateResponse> {
    Json(PrivateResponse {
        message: "The `Claims` request guard ensures only valid JWTs can access this endpoint"
            .to_string(),
        user: user.name,
    })
}

#[derive(Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Serialize)]
struct LoginResponse {
    token: String,
}

/// Tries to authenticate a user. Successful authentications get a JWT
#[post("/authenticate", data = "<login>")]
fn authenticate(login: Json<LoginRequest>) -> Result<Json<LoginResponse>, Custom<String>> {
    // This should be real user validation code, but is left simple for this example
    if login.username != "username@a.com" || login.password != "password" {
        return Err(Custom(
            Status::Unauthorized,
            "account was not found".to_string(),
        ));
    }

    let claim = Claims::from_name(&login.username);
    let response = LoginResponse {
        token: claim.into_token()?,
    };

    Ok(Json(response))
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
            sessions::list_sessions,
            sessions::list_sessions_by_date
        ])
        .manage(state);

    Ok(rocket.into())
}
