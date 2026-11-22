//! Event log entries for requests whose login session could not be
//! authenticated.
//!
//! The `LoginSession` request guard records why authentication failed with
//! [`AuthFailure::record`]. Once the response is ready, [`UnauthorizedLogger`]
//! writes an UNAUTHORIZED event with the details of the request and the client:
//! the method and path, the response status, the client's IP address, the
//! proxy headers, and the User-Agent, Referer, Origin and Host headers.
//!
//! The guard also runs on routes where login is optional (e.g. the blog and
//! session listings, login and logout). Those requests are served anonymously
//! rather than refused, and the entry's outcome says so. Only refused requests
//! have their IP address geolocated; anonymous views log just the address.
//!
//! The location lookup can take a moment, so the entry is written in a
//! background task and the response is not delayed.

use rocket::fairing::{Fairing, Info, Kind};
use rocket::http::Status;
use rocket::{Request, Response};
use rocket_client_addr::ClientRealAddr;
use sqlx::PgPool;

use crate::geoip::{summarize, GeoIp};
use crate::transaction_log::append_log;

/// Header values are cut to this many characters
const MAX_HEADER_CHARS: usize = 300;

/// Why the request's login session could not be authenticated
#[derive(Default)]
pub struct AuthFailure(Option<String>);

impl AuthFailure {
    pub fn record(request: &Request<'_>, reason: String) {
        // The request-local cache keeps the first reason recorded
        request.local_cache(|| AuthFailure(Some(reason)));
    }

    fn reason<'r>(request: &'r Request<'_>) -> Option<&'r str> {
        request.local_cache(AuthFailure::default).0.as_deref()
    }
}

pub struct UnauthorizedLogger;

#[rocket::async_trait]
impl Fairing for UnauthorizedLogger {
    fn info(&self) -> Info {
        Info { name: "Unauthorized request logger", kind: Kind::Response }
    }

    async fn on_response<'r>(&self, request: &'r Request<'_>, response: &mut Response<'r>) {
        let Some(reason) = AuthFailure::reason(request) else {
            return;
        };
        let Some(pool) = request.rocket().state::<PgPool>().cloned() else {
            return;
        };
        let refused = response.status() == Status::Unauthorized;
        // Geolocate only refused requests; anonymous views are ordinary visitors
        let geoip = if refused { request.rocket().state::<GeoIp>().cloned() } else { None };
        let client_ip = request.guard::<ClientRealAddr>().await.succeeded()
            .map(|addr| addr.get_ipv6().to_canonical());

        // Everything but the location, which is looked up in the background
        let outcome = if refused {
            format!("refused ({})", response.status())
        } else {
            format!("served anonymously ({})", response.status())
        };
        let mut request_details = vec![
            reason.to_string(),
            format!("{} {} → {outcome}", request.method(), clean(&request.uri().to_string())),
        ];
        let mut client_details = vec![
            format!("socket {}", request.remote().map(|a| a.to_string()).unwrap_or_else(|| "unknown".to_string())),
        ];
        for header in ["X-Forwarded-For", "X-Real-IP", "Forwarded", "User-Agent", "Referer", "Origin", "Host"] {
            let values: Vec<&str> = request.headers().get(header).collect();
            if !values.is_empty() {
                client_details.push(format!("{header}: {}", clean(&values.join(", "))));
            }
        }
        if request.headers().get_one("User-Agent").is_none() {
            client_details.push("no User-Agent".to_string());
        }

        tokio::spawn(async move {
            let ip_detail = match (client_ip, geoip) {
                (Some(ip), Some(geoip)) => match geoip.lookup(ip).await {
                    Some(location) => format!("IP {ip} ({})", clean(&summarize(&location))),
                    None => format!("IP {ip} (location unknown)"),
                },
                (Some(ip), None) => format!("IP {ip}"),
                (None, _) => "IP unknown".to_string(),
            };
            request_details.push(ip_detail);
            request_details.append(&mut client_details);
            let detail = request_details.join(" | ");
            if let Err(e) = append_log(&pool, &None, "UNAUTHORIZED", &detail).await {
                error!("Failed to log unauthorized request: {} {}", e.0, e.1);
            }
        });
    }
}

/// A header or URI value made safe for a one-line log entry: control
/// characters replaced and the length capped
fn clean(value: &str) -> String {
    let mut cleaned: String = value.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(MAX_HEADER_CHARS)
        .collect();
    if value.chars().count() > MAX_HEADER_CHARS {
        cleaned.push('…');
    }
    cleaned
}

#[cfg(test)]
mod tests {
    use rocket::http::{Header, Status};
    use rocket::local::asynchronous::Client;
    use rocket::{get, routes};
    use sqlx::{query_as, Executor, PgPool};

    use crate::loginsession::LoginSession;

    use super::{clean, UnauthorizedLogger};

    #[get("/required")]
    fn required(_login: LoginSession) -> &'static str { "ok" }

    #[get("/optional")]
    fn optional(_login: Option<LoginSession>) -> &'static str { "ok" }

    #[get("/public")]
    fn public() -> &'static str { "ok" }

    async fn client(pool: &PgPool) -> Client {
        let rocket = rocket::build()
            .manage(pool.clone())
            .manage(crate::geoip::GeoIp::new())
            .attach(UnauthorizedLogger)
            .mount("/", routes![required, optional, public]);
        Client::untracked(rocket).await.unwrap()
    }

    /// The entries logged so far, waiting briefly for the background tasks
    async fn logged(pool: &PgPool, expected: usize) -> Vec<(String, String)> {
        for _ in 0..50 {
            let rows: Vec<(String, String)> = query_as("SELECT type, detail FROM eventlog ORDER BY id")
                .fetch_all(pool).await.unwrap();
            if rows.len() >= expected {
                return rows;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        panic!("expected {expected} log entries");
    }

    #[sqlx::test]
    async fn test_refused_request_is_logged_with_details(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();
        let client = client(&pool).await;

        let response = client.get("/required?x=1")
            .header(Header::new("User-Agent", "curl/8.0\tInjected: yes"))
            .header(Header::new("X-Forwarded-For", "203.0.113.7, 10.0.0.1"))
            .header(Header::new("Referer", "https://example.org/page"))
            .dispatch().await;
        assert_eq!(Status::Unauthorized, response.status());

        let entries = logged(&pool, 1).await;
        let (event_type, detail) = &entries[0];
        assert_eq!("UNAUTHORIZED", event_type);
        assert!(detail.starts_with("Session cookie not sent by client | GET /required?x=1 → refused (401 Unauthorized) | IP "), "{detail}");
        // The client address comes from X-Forwarded-For; 203.0.113.0/24 is
        // reserved for documentation, so it is not looked up
        assert!(detail.contains("IP 203.0.113.7 (location unknown)"), "{detail}");
        assert!(detail.contains("X-Forwarded-For: 203.0.113.7, 10.0.0.1"), "{detail}");
        assert!(detail.contains("User-Agent: curl/8.0 Injected: yes"), "{detail}");
        assert!(detail.contains("Referer: https://example.org/page"), "{detail}");
        assert!(!detail.contains('\t'), "{detail}");
    }

    #[sqlx::test]
    async fn test_anonymous_request_is_logged_as_served(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();
        let client = client(&pool).await;

        // A public address, which would be geolocated if the request were refused
        let response = client.get("/optional")
            .header(Header::new("Authorization", "Basic abc"))
            .header(Header::new("X-Forwarded-For", "8.8.8.8"))
            .dispatch().await;
        assert_eq!(Status::Ok, response.status());

        let entries = logged(&pool, 1).await;
        let detail = &entries[0].1;
        assert!(detail.starts_with("Authorization header is not a bearer token, and no session cookie sent | GET /optional → served anonymously (200 OK) | IP 8.8.8.8 | "), "{detail}");
        assert!(detail.contains("no User-Agent"), "{detail}");
    }

    #[sqlx::test]
    async fn test_unknown_session_is_logged(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();
        let client = client(&pool).await;

        let response = client.get("/required").header(Header::new("Authorization", "Bearer nosuchsession")).dispatch().await;
        assert_eq!(Status::Unauthorized, response.status());

        let detail = &logged(&pool, 1).await[0].1;
        assert!(detail.starts_with("Session record not found in table (expired or logged out), sent as bearer token | GET /required"), "{detail}");
        assert!(!detail.contains("nosuchsession"), "the token itself is not logged: {detail}");
    }

    #[sqlx::test]
    async fn test_routes_without_login_are_not_logged(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();
        let client = client(&pool).await;

        client.get("/public").dispatch().await;
        client.get("/required").dispatch().await;

        // Only the second request is logged
        let entries = logged(&pool, 1).await;
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        let entries_after = logged(&pool, 1).await;
        assert_eq!(1, entries_after.len());
        assert!(entries[0].1.contains("GET /required"));
    }

    #[test]
    fn test_clean() {
        assert_eq!("a b", clean("a\nb"));
        let long = "x".repeat(400);
        assert_eq!(format!("{}…", "x".repeat(300)), clean(&long));
    }
}
