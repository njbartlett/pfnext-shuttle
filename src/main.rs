

use actix_files::{Files, NamedFile};
use actix_web::middleware::Logger;
use actix_web::{error, get, post, web::{self, Json, ServiceConfig}, Result, Responder};
use serde::{Deserialize, Serialize};
use shuttle_actix_web::ShuttleActixWeb;
use shuttle_runtime::CustomError;
use sqlx::{Executor, FromRow, PgPool};

#[derive(Serialize, Deserialize, FromRow, Debug)]
struct Person {
    id: i64,
    name: String,
    email: String,
    phone: Option<String>
}

#[derive(Deserialize)]
struct PersonNew {
    pub name: String,
    pub email: String,
    pub phone: Option<String>
}

#[get("person/{id}")]
async fn get_person(path: web::Path<i32>, state: web::Data<AppState>) -> Result<Json<Person>> {
    let person = sqlx::query_as("SELECT * FROM person WHERE id = $1")
        .bind(*path)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| error::ErrorBadRequest(e.to_string()))?;
    Ok(Json(person))
}

#[get("person")]
async fn list_persons(state: web::Data<AppState>) -> Result<Json<Vec<Person>>> {
    let persons = sqlx::query_as("SELECT * FROM person")
        .fetch_all(&state.pool)
        .await
        .map_err(|e| error::ErrorBadRequest(e.to_string()))?;
    Ok(Json(persons))
}

#[post("person")]
async fn add_person(person: web::Json<PersonNew>, state: web::Data<AppState>) -> Result<Json<Person>> {
    let person = sqlx::query_as("INSERT INTO person(name, email, phone) VALUES ($1, $2, $3) RETURNING id, name, email, phone")
        .bind(&person.name) //.name, &person.email, &person.phone, &person.dob
        .bind(&person.email)
        .bind(&person.phone)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| error::ErrorBadRequest(e.to_string()))?;

    Ok(Json(person))
}

#[get("/")]
async fn html_index() -> impl Responder {
    NamedFile::open_async("assets/index.html").await
}

#[derive(Clone)]
struct AppState {
    pool: PgPool,
}

#[shuttle_runtime::main]
async fn actix_web(
    #[shuttle_shared_db::Postgres] pool: PgPool,
) -> ShuttleActixWeb<impl FnOnce(&mut ServiceConfig) + Send + Clone + 'static> {
    pool.execute(include_str!("../schema.sql"))
        .await
        .map_err(CustomError::new)?;
    let state = web::Data::new(AppState { pool });

    let config = move |cfg: &mut ServiceConfig| {
        //cfg.service(
        //     web:://scope("/")
                //.wrap(Logger::default())
        cfg
            .service(get_person)
            .service(list_persons)
            .service(add_person)
            .service(Files::new("/", "assets").index_file("index.html"))
            .app_data(state);
        //);
    };

    Ok(config.into())
}
