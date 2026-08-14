use time::OffsetDateTime;

use super::{Store, StoreError};

pub const US: &str = "goat-merge";

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Note {
    pub id: i64,
    pub at: OffsetDateTime,
    pub actor: String,
    pub action: String,
    pub repository_id: Option<i64>,
    pub pull_request: Option<i32>,
    pub detail: String,
}

const COLUMNS: &str = "id, at, actor, action, repository_id, pull_request, detail";

impl Store {
    pub async fn write_down(
        &self,
        actor: &str,
        action: &str,
        repository_id: Option<i64>,
        pull_request: Option<i32>,
        detail: &str,
    ) -> Result<(), StoreError> {
        sqlx::query(
            "insert into audit (actor, action, repository_id, pull_request, detail) \
             values ($1, $2, $3, $4, $5)",
        )
        .bind(actor)
        .bind(action)
        .bind(repository_id)
        .bind(pull_request)
        .bind(detail)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn what_happened_to(
        &self,
        repository_id: i64,
        pull_request: Option<i32>,
        most: i64,
    ) -> Result<Vec<Note>, StoreError> {
        let notes = sqlx::query_as::<_, Note>(&format!(
            "select {COLUMNS} from audit \
             where repository_id = $1 and ($2::int is null or pull_request = $2) \
             order by at desc limit $3"
        ))
        .bind(repository_id)
        .bind(pull_request)
        .bind(most)
        .fetch_all(self.pool())
        .await?;
        Ok(notes)
    }
}
