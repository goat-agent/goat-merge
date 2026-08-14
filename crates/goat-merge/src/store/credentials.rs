use super::{Store, StoreError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppCredentials {
    pub app_id: i64,
    pub slug: String,
    pub client_id: String,
    pub private_key: String,
    pub webhook_secret: String,
    pub client_secret: String,
}

impl Store {
    pub async fn app_credentials(&self) -> Result<Option<AppCredentials>, StoreError> {
        let Some(row) = sqlx::query_as::<_, (i64, String, String, Vec<u8>, Vec<u8>, Vec<u8>)>(
            "select app_id, slug, client_id, private_key, webhook_secret, client_secret \
             from app_credentials where only_row = 1",
        )
        .fetch_optional(self.pool())
        .await?
        else {
            return Ok(None);
        };
        Ok(Some(AppCredentials {
            app_id: row.0,
            slug: row.1,
            client_id: row.2,
            private_key: self.seal().open_text(&row.3)?,
            webhook_secret: self.seal().open_text(&row.4)?,
            client_secret: self.seal().open_text(&row.5)?,
        }))
    }

    pub async fn remember_app(&self, app: &AppCredentials) -> Result<(), StoreError> {
        sqlx::query(
            "insert into app_credentials \
                 (only_row, app_id, slug, client_id, private_key, webhook_secret, client_secret) \
             values (1, $1, $2, $3, $4, $5, $6) \
             on conflict (only_row) do update set \
                 app_id = excluded.app_id, \
                 slug = excluded.slug, \
                 client_id = excluded.client_id, \
                 private_key = excluded.private_key, \
                 webhook_secret = excluded.webhook_secret, \
                 client_secret = excluded.client_secret",
        )
        .bind(app.app_id)
        .bind(&app.slug)
        .bind(&app.client_id)
        .bind(self.seal().seal(app.private_key.as_bytes())?)
        .bind(self.seal().seal(app.webhook_secret.as_bytes())?)
        .bind(self.seal().seal(app.client_secret.as_bytes())?)
        .execute(self.pool())
        .await?;
        Ok(())
    }
}
