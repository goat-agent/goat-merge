use std::collections::HashMap;
use std::sync::Mutex;

use jsonwebtoken::{Algorithm, EncodingKey, Header};
use serde::Serialize;
use time::{Duration, OffsetDateTime};

use super::GithubError;

pub struct AppAuth {
    app_id: i64,
    key: EncodingKey,
    tokens: Mutex<HashMap<i64, Loan>>,
}

#[derive(Clone)]
struct Loan {
    token: String,
    good_until: OffsetDateTime,
}

#[derive(Serialize)]
struct Claims {
    iat: i64,
    exp: i64,
    iss: String,
}

impl AppAuth {
    pub fn holding(app_id: i64, private_key: &str) -> Result<Self, GithubError> {
        let key = EncodingKey::from_rsa_pem(private_key.as_bytes()).map_err(|problem| {
            GithubError::PrivateKeyUnusable {
                problem: problem.to_string(),
            }
        })?;
        Ok(Self {
            app_id,
            key,
            tokens: Mutex::new(HashMap::new()),
        })
    }

    pub fn as_the_app(&self) -> Result<String, GithubError> {
        let now = OffsetDateTime::now_utc();
        let claims = Claims {
            iat: (now - Duration::seconds(60)).unix_timestamp(),
            exp: (now + Duration::minutes(9)).unix_timestamp(),
            iss: self.app_id.to_string(),
        };
        jsonwebtoken::encode(&Header::new(Algorithm::RS256), &claims, &self.key).map_err(
            |problem| GithubError::PrivateKeyUnusable {
                problem: problem.to_string(),
            },
        )
    }

    pub fn borrowed(&self, installation: i64) -> Option<String> {
        let held = self.tokens.lock().ok()?;
        let loan = held.get(&installation)?;
        (loan.good_until > OffsetDateTime::now_utc() + Duration::minutes(1))
            .then(|| loan.token.clone())
    }

    pub fn keep(&self, installation: i64, token: String, good_until: OffsetDateTime) {
        if let Ok(mut held) = self.tokens.lock() {
            held.insert(installation, Loan { token, good_until });
        }
    }

    pub fn forget(&self, installation: i64) {
        if let Ok(mut held) = self.tokens.lock() {
            held.remove(&installation);
        }
    }
}
