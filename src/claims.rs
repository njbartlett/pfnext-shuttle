// claims.rs
use std::fmt::{Display, Formatter};
use std::ops::Add;
use chrono::{Duration, Utc};
use jsonwebtoken::{errors::ErrorKind, DecodingKey, EncodingKey, Header, Validation, Algorithm};
use rocket::{http::Status, request::{FromRequest, Outcome}, response::status::Custom};
use serde::{Deserialize, Serialize};
use crate::AppState;

const BEARER: &str = "Bearer ";
const AUTHORIZATION: &str = "Authorization";

// Used when decoding a token to `Claims`
#[derive(Debug, PartialEq, Clone)]
pub(crate) enum AuthenticationError {
    Missing,
    Decoding(String),
    Expired,
}

impl Display for AuthenticationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing => f.write_str("missing authorization header"),
            Self::Decoding(msg) => write!(f, "failed to decode authorization header: {}", msg),
            Self::Expired => f.write_str("authorization token expired")
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Claims {
    pub(crate) uid: i64,
    pub(crate) email: String,
    pub(crate) phone: Option<String>,
    pub(crate) roles: Vec<String>,
    exp: usize,
}

// Rocket specific request guard implementation
#[rocket::async_trait]
impl<'r> FromRequest<'r> for Claims {
    type Error = AuthenticationError;

    async fn from_request(request: &'r rocket::Request<'_>) -> Outcome<Self, Self::Error> {
        let auth_header = request.headers().get_one(AUTHORIZATION);
        match auth_header {
            None => {
                request.local_cache::<Option<AuthenticationError>, _>(|| Some(AuthenticationError::Missing));
                Outcome::Error((Status::Forbidden, AuthenticationError::Missing))
            },
            Some(value) => {
                // Get the secret encoding/decoding key from the Rocket state
                let secret: Option<String> = request.rocket().state()
                    .and_then(|s: &AppState| s.secrets.get("ACCESS_TOKEN_KEY"));
                if secret.is_none() {
                    return Outcome::Error((Status::InternalServerError, AuthenticationError::Decoding("Missing app state".to_string())));
                }

                match Claims::from_authorization(value, &secret.unwrap()) {
                    Err(e) => {
                        request.local_cache::<Option<AuthenticationError>, _>(|| Some(e.clone()));
                        Outcome::Error((Status::Forbidden, e))
                    },
                    Ok(claims) => {
                        Outcome::Success(claims)
                    },
                }
            },
        }
    }
}

impl Claims {
    pub(crate) fn create(uid: i64, email: &str, phone: &Option<String>, roles: &Vec<String>, duration: Duration) -> Self {
        let now = Utc::now();
        let expiration = Utc::now().add(duration);
        info!("now={}, expiration={}", now, expiration);
        info!("Creating token with expiration {}", expiration);
        Self {
            uid,
            email: email.to_string(),
            phone: phone.clone(),
            roles: roles.to_owned(),
            exp: expiration.timestamp() as usize,
        }
    }

    /// Converts this claims into a token string
    pub(crate) fn into_token(self, secret: &str) -> Result<String, Custom<String>> {
        jsonwebtoken::encode(
            &Header::default(),
            &self,
            &EncodingKey::from_secret(secret.as_ref()),
        ).map_err(|e| Custom(Status::InternalServerError, e.to_string()))
    }

    pub(crate) fn has_role(&self, required_role: &str) -> bool {
        return self.roles.iter().any(|r| r == required_role);
    }

    pub(crate) fn assert_roles_contains(&self, required_role: &str) -> Result<(), Custom<String>> {
        if !self.has_role(required_role) {
            return Err(Custom(Status::Forbidden, format!("user is not allowed to perform this action (missing required role: {})", required_role)));
        }
        Ok(())
    }

    /// Create a `Claims` from a 'Bearer <token>' value
    fn from_authorization(value: &str, secret: &str) -> Result<Self, AuthenticationError> {
        let token = value
            .strip_prefix(BEARER)
            .map(str::trim)
            .ok_or(AuthenticationError::Missing)?;

        let mut validation = Validation::new(Algorithm::HS256);
        validation.leeway = 0;
        let token = jsonwebtoken::decode::<Claims>(token, &DecodingKey::from_secret(secret.as_ref()), &validation)
            .map_err(|e| match e.kind() {
                ErrorKind::ExpiredSignature => AuthenticationError::Expired,
                _                           => AuthenticationError::Decoding(e.to_string()),
            })?;
        Ok(token.claims)
    }
}

#[cfg(test)]
mod tests {
    
    use chrono::Duration;
    use rocket::http::Status;
    use rocket::response::status::Custom;
    use crate::claims::AuthenticationError;

    use super::Claims;

    #[test]
    fn missing_bearer() {
        let claim_err = Claims::from_authorization("no-Bearer-prefix", "let me in").unwrap_err();

        assert_eq!(claim_err, AuthenticationError::Missing);
    }

    #[test]
    fn to_token_and_back() {
        let claim = Claims::create(1, "joe@example.com", &Some(String::from("010101")), &vec!("member".to_string()), Duration::minutes(1));
        let token = claim.into_token("let me in").unwrap();
        let token = format!("Bearer {token}");

        let claim = Claims::from_authorization(&token, "let me in").unwrap();

        assert_eq!(claim.email, "joe@example.com");
    }

    #[test]
    fn assert_roles_any() {
        let claim = Claims::create(1, "joe@example.com", &Some(String::from("010101")), &vec!("member".to_string()), Duration::minutes(1));
        assert_eq!(claim.assert_roles_contains("member"), Ok(()));
        assert_eq!(claim.assert_roles_contains("admin"), Err(Custom(Status::Forbidden, "missing required role: admin".to_string())));
    }

}