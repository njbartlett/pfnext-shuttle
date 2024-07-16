mod persons;
mod sessions;

use actix_files::Files;
// use actix_web::middleware::Logger;
use actix_web::{web::{self, ServiceConfig}};
use shuttle_actix_web::ShuttleActixWeb;
use shuttle_runtime::CustomError;
use sqlx::{Executor, PgPool};

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
            .service(persons::get_person)
            .service(persons::list_persons)
            .service(persons::add_person)
            .service(sessions::list_sessions)
            .service(sessions::list_session_by_date)
            .service(Files::new("/", "assets").index_file("index.html"))
            .app_data(state);
        //);
    };

    Ok(config.into())
}
