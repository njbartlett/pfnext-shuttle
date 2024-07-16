use std::path::PathBuf;
use rocket::{Request, Response};
use rocket::http::Status;
use rocket::response::Responder;
use rocket::response::status::Custom;

struct CorsResponse {
    my_header: String
}

impl<'r> Responder<'r, 'static> for CorsResponse {
    fn respond_to(self, req: &'r Request<'_>) -> rocket::response::Result<'static> {
        let origin_host = req.headers()
            .get_one("origin")
            .ok_or(Status::BadRequest)?;
        println!("Received CORS options request from origin: {}", origin_host);
        Response::build()
            .raw_header("Access-Control-Allow-Origin", "*")//origin_host.to_string())
            .raw_header("Access-Control-Allow-Methods", "GET, POST, HEAD, OPTIONS")
            .raw_header("Access-Control-Allow-Headers", "*")
            //.raw_header("Access-Control-Max-Age", "60")
            .ok()
    }
}

#[options("/<path..>")]
pub fn cors_options(path: PathBuf) -> Result<CorsResponse, Custom<String>> {
    println!("Answering options");
    Ok(CorsResponse{
        my_header: "Hello".to_string()
    })
}