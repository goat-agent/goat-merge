use serde_json::Value;
use time::{Duration, OffsetDateTime};

use super::{Store, StoreError};

impl Store {
    pub async fn write_down_delivery(
        &self,
        id: &str,
        event: &str,
        payload: &Value,
    ) -> Result<bool, StoreError> {
        let written = sqlx::query(
            "insert into deliveries (id, event, payload) values ($1, $2, $3) \
             on conflict (id) do nothing",
        )
        .bind(id)
        .bind(event)
        .bind(payload)
        .execute(self.pool())
        .await?;
        Ok(written.rows_affected() > 0)
    }

    pub async fn delivery_is_handled(&self, id: &str) -> Result<(), StoreError> {
        sqlx::query("update deliveries set handled_at = now() where id = $1")
            .bind(id)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    pub async fn forget_deliveries_older_than(&self, age: Duration) -> Result<u64, StoreError> {
        let cutoff = OffsetDateTime::now_utc() - age;
        let gone = sqlx::query("delete from deliveries where received_at < $1")
            .bind(cutoff)
            .execute(self.pool())
            .await?;
        Ok(gone.rows_affected())
    }
}
