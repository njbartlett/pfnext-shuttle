
use actix_web::middleware::Logger;
use actix_web::{
    error, get, post,
    web::{self, Json, ServiceConfig},
    Result,
};
use actix_web::cookie::time::Date;
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

#[get("/{id}")]
async fn get_person(path: web::Path<i32>, state: web::Data<AppState>) -> Result<Json<Person>> {
    let person = sqlx::query_as("SELECT * FROM person WHERE id = $1")
        .bind(*path)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| error::ErrorBadRequest(e.to_string()))?;
    Ok(Json(person))
}

#[get("/")]
async fn list_persons(state: web::Data<AppState>) -> Result<Json<Vec<Person>>> {
    let persons = sqlx::query_as("SELECT * FROM person")
        .fetch_all(&state.pool)
        .await
        .map_err(|e| error::ErrorBadRequest(e.to_string()))?;
    Ok(Json(persons))
}

#[post("")]
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
        cfg.service(
            web::scope("/person")
                .wrap(Logger::default())
                .service(get_person)
                .service(list_persons)
                .service(add_person)
                .app_data(state),
        );
    };

    Ok(config.into())
}
