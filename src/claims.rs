// claims.rs
use chrono::{Duration, Utc};
use jsonwebtoken::{
    decode, encode, errors::ErrorKind, DecodingKey, EncodingKey, Header, Validation,
};
use rocket::{
    http::Status,
    request::{FromRequest, Outcome},
    response::status::Custom,
};
use serde::{Deserialize, Serialize};
use crate::AppState;

const BEARER: &str = "Bearer ";
const AUTHORIZATION: &str = "Authorization";

/// Key used for symmetric token encoding
const SECRET_SIZE: usize = 1024;

// Used when decoding a token to `Claims`
#[derive(Debug, PartialEq)]
pub(crate) enum AuthenticationError {
    Missing,
    Decoding(String),
    Expired,
}

// Basic claim object. Only the `exp` claim (field) is required. Consult the `jsonwebtoken` documentation for other claims that can be validated.
// The `name` is a custom claim for this API
#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Claims {
    pub(crate) uid: i64,
    pub(crate) email: String,
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
                Outcome::Error((Status::Forbidden, AuthenticationError::Missing))
            },
            Some(value) => {
                // Get the secret encoding/decoding key from the Rocket state
                let secrets: &shuttle_runtime::SecretStore;
                match request.rocket().state::<AppState>() {
                    Some(app_state) => {
                        secrets = &app_state.secrets;
                    },
                    None => {
                        return Outcome::Error((Status::Forbidden, AuthenticationError::Decoding("Missing app state".to_string())));
                    }
                }
                match Claims::from_authorization(value, secrets) {
                    Err(e) => {
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
    pub(crate) fn create(uid: i64, email: &str, roles: &Vec<String>, duration: Duration) -> Self {
        let expiration = Utc::now()
            .checked_add_signed(duration)
            .expect("failed to create an expiration time")
            .timestamp();
        Self {
            uid,
            email: email.to_string(),
            roles: roles.to_owned(),
            exp: expiration as usize,
        }
    }

    /// Create a `Claims` from a 'Bearer <token>' value
    fn from_authorization(value: &str, secrets: &shuttle_runtime::SecretStore) -> Result<Self, AuthenticationError> {
        let token = value.strip_prefix(BEARER).map(str::trim);

        if token.is_none() {
            return Err(AuthenticationError::Missing);
        }

        // Safe to unwrap as we just confirmed it is not none
        let token = token.unwrap();

        // Use `jsonwebtoken` to get the claims from a JWT
        // Consult the `jsonwebtoken` documentation for using other algorithms and validations (the default validation just checks the expiration claim)
        let secret = secrets.get("TOKEN_KEY").ok_or(AuthenticationError::Decoding("missing decoding key".to_string()))?;
        let token = decode::<Claims>(
            token,
            &DecodingKey::from_secret(secret.as_ref()),
            &Validation::default(),
        )
        .map_err(|e| match e.kind() {
            ErrorKind::ExpiredSignature => AuthenticationError::Expired,
            _                           => AuthenticationError::Decoding(e.to_string()),
        })?;

        Ok(token.claims)
    }

    /// Converts this claims into a token string
    pub(crate) fn into_token(self, secrets: &shuttle_runtime::SecretStore) -> Result<String, Custom<String>> {
        let secret = secrets.get("TOKEN_KEY").ok_or(Custom(Status::Forbidden, "missing decoding key".to_string()))?;
        let token = encode(
            &Header::default(),
            &self,
            &EncodingKey::from_secret(secret.as_ref()),
        )
        .map_err(|e| Custom(Status::BadRequest, e.to_string()))?;

        Ok(token)
    }

    pub(crate) fn has_role(&self, required_role: &str) -> bool {
        return self.roles.iter().any(|r| r == required_role);
    }

    pub(crate) fn assert_roles_contains(&self, required_role: &str) -> Result<(), Custom<String>> {
        if !self.has_role(required_role) {
            return Err(Custom(Status::Forbidden, format!("missing required role: {}", required_role)));
        }
        Ok(())
    }

}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use chrono::Duration;
    use rocket::http::Status;
    use rocket::response::status::Custom;
    use crate::claims::AuthenticationError;

    use super::Claims;

    #[test]
    fn missing_bearer() {
        let secrets = shuttle_runtime::SecretStore::new(BTreeMap::from([
            ("TOKEN_KEY".to_owned(), "let me in".to_owned().into())
        ]));
        let claim_err = Claims::from_authorization("no-Bearer-prefix", &secrets).unwrap_err();

        assert_eq!(claim_err, AuthenticationError::Missing);
    }

    #[test]
    fn to_token_and_back() {
        let claim = Claims::create(1, "joe@example.com", &vec!("member".to_string()), Duration::minutes(1));
        let secrets = shuttle_runtime::SecretStore::new(BTreeMap::from([
            ("TOKEN_KEY".to_owned(), "let me in".to_owned().into())
        ]));
        let token = claim.into_token(&secrets).unwrap();
        let token = format!("Bearer {token}");

        let claim = Claims::from_authorization(&token, &secrets).unwrap();

        assert_eq!(claim.email, "joe@example.com");
    }

    #[test]
    fn assert_roles_any() {
        let claim = Claims::create(1, "joe@example.com", &vec!("member".to_string()), Duration::minutes(1));
        assert_eq!(claim.assert_roles_contains("member"), Ok(()));
        assert_eq!(claim.assert_roles_contains("admin"), Err(Custom(Status::Forbidden, "missing required role: admin".to_string())));
    }

}