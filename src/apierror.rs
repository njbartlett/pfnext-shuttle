use rocket::http::Status;
use rocket::response::{self, Responder};
use rocket::serde::json::Json;
use rocket::Request;
use serde::Serialize;

/// Machine-readable error code for the credits opt-in handshake on booking creation.
pub const CREDITS_OPT_IN_REQUIRED: &str = "credits_opt_in_required";

#[derive(Debug, PartialEq, Clone, Serialize)]
pub struct ApiErrorBody {
    pub code: String,
    pub message: String,
}

impl std::fmt::Display for ApiErrorBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// API error responder producing a JSON body: {"code": "...", "message": "..."}.
/// The status is the first tuple field, mirroring rocket's status::Custom.
#[derive(Debug, PartialEq, Clone)]
pub struct ApiError(pub Status, pub ApiErrorBody);

impl ApiError {
    pub fn new(status: Status, message: impl Into<String>) -> Self {
        Self::with_code(status, default_code(status), message)
    }

    pub fn with_code(status: Status, code: &str, message: impl Into<String>) -> Self {
        ApiError(status, ApiErrorBody { code: code.to_string(), message: message.into() })
    }
}

fn default_code(status: Status) -> &'static str {
    match status.code {
        400 => "bad_request",
        401 => "unauthorized",
        402 => "payment_required",
        403 => "forbidden",
        404 => "not_found",
        409 => "conflict",
        422 => "unprocessable_entity",
        500 => "internal_error",
        _ => "error",
    }
}

impl<'r> Responder<'r, 'static> for ApiError {
    fn respond_to(self, request: &'r Request<'_>) -> response::Result<'static> {
        (self.0, Json(self.1)).respond_to(request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rocket::local::asynchronous::Client;

    #[get("/fail")]
    fn fail() -> Result<(), ApiError> {
        Err(ApiError::new(Status::Forbidden, "admin role required"))
    }

    #[get("/fail_coded")]
    fn fail_coded() -> Result<(), ApiError> {
        Err(ApiError::with_code(Status::PaymentRequired, CREDITS_OPT_IN_REQUIRED, "Opt in to use credits for booking."))
    }

    #[rocket::async_test]
    async fn test_json_error_body() {
        let client = Client::untracked(rocket::build().mount("/", routes![fail, fail_coded])).await.unwrap();

        let resp = client.get("/fail").dispatch().await;
        assert_eq!(Status::Forbidden, resp.status());
        assert_eq!(Some(rocket::http::ContentType::JSON), resp.content_type());
        assert_eq!("{\"code\":\"forbidden\",\"message\":\"admin role required\"}", resp.into_string().await.unwrap());

        let resp = client.get("/fail_coded").dispatch().await;
        assert_eq!(Status::PaymentRequired, resp.status());
        assert_eq!("{\"code\":\"credits_opt_in_required\",\"message\":\"Opt in to use credits for booking.\"}", resp.into_string().await.unwrap());
    }
}
