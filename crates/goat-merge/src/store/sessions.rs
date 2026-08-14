use chacha20poly1305::aead::{OsRng, rand_core::RngCore};
use sha2::{Digest, Sha256};

use super::{Store, StoreError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub login: String,
}

pub fn fresh_token() -> String {
    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn digest(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

impl Store {
    pub async fn start_session(
        &self,
        login: &str,
        access_token: &str,
    ) -> Result<String, StoreError> {
        let token = fresh_token();
        sqlx::query("insert into sessions (token_digest, login, access_token) values ($1, $2, $3)")
            .bind(digest(&token))
            .bind(login)
            .bind(self.seal().seal(access_token.as_bytes())?)
            .execute(self.pool())
            .await?;
        Ok(token)
    }

    pub async fn session(&self, token: &str) -> Result<Option<Session>, StoreError> {
        let found = sqlx::query_as::<_, (String,)>(
            "update sessions set seen_at = now() where token_digest = $1 returning login",
        )
        .bind(digest(token))
        .fetch_optional(self.pool())
        .await?;
        Ok(found.map(|row| Session { login: row.0 }))
    }

    pub async fn end_session(&self, token: &str) -> Result<(), StoreError> {
        sqlx::query("delete from sessions where token_digest = $1")
            .bind(digest(token))
            .execute(self.pool())
            .await?;
        Ok(())
    }
}
