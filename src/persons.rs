use rocket::State;
use rocket::response::status::BadRequest;
use rocket::serde::json::Json;

use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use crate::AppState;

#[derive(Serialize, FromRow, Debug)]
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

// #[get("/person/{id}")]
// async fn get_person(path: web::Path<i32>, state: web::Data<AppState>) -> actix_web::Result<Json<Person>> {
//     let person = sqlx::query_as("SELECT * FROM person WHERE id = $1")
//         .bind(*path)
//         .fetch_one(&state.pool)
//         .await
//         .map_err(|e| error::ErrorBadRequest(e.to_string()))?;
//     Ok(Json(person))
// }

#[get("/persons")]
pub async fn list_persons(state: &State<AppState>) -> Result<Json<Vec<Person>>, BadRequest<String>> {
    let persons = sqlx::query_as("SELECT * FROM person")
        .fetch_all(&state.pool)
        .await
        .map_err(|e|BadRequest(e.to_string()))?;
    Ok(Json(persons))
}

// #[post("/person")]
// async fn add_person(person: web::Json<PersonNew>, state: web::Data<AppState>) -> actix_web::Result<Json<Person>> {
//     let person = sqlx::query_as("INSERT INTO person(name, email, phone) VALUES ($1, $2, $3) RETURNING id, name, email, phone")
//         .bind(&person.name) //.name, &person.email, &person.phone, &person.dob
//         .bind(&person.email)
//         .bind(&person.phone)
//         .fetch_one(&state.pool)
//         .await
//         .map_err(|e| error::ErrorBadRequest(e.to_string()))?;
//
//     Ok(Json(person))
// }