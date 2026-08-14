use super::{Store, StoreError};

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Repository {
    pub id: i64,
    pub installation_id: i64,
    pub owner: String,
    pub name: String,
    pub active: bool,
    pub out_of_reach_at: Option<time::OffsetDateTime>,
}

impl Repository {
    pub fn full_name(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }

    pub fn we_can_still_see_it(&self) -> bool {
        self.out_of_reach_at.is_none()
    }
}

const COLUMNS: &str = "id, installation_id, owner, name, active, out_of_reach_at";

impl Store {
    pub async fn remember_installation(&self, id: i64, account: &str) -> Result<(), StoreError> {
        sqlx::query(
            "insert into installations (id, account) values ($1, $2) \
             on conflict (id) do update set account = excluded.account, removed_at = null",
        )
        .bind(id)
        .bind(account)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn forget_installation(&self, id: i64) -> Result<(), StoreError> {
        sqlx::query("update installations set removed_at = now() where id = $1")
            .bind(id)
            .execute(self.pool())
            .await?;
        sqlx::query("update repositories set active = false where installation_id = $1")
            .bind(id)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    pub async fn remember_repository(
        &self,
        id: i64,
        installation_id: i64,
        owner: &str,
        name: &str,
    ) -> Result<Repository, StoreError> {
        let row = sqlx::query_as::<_, Repository>(&format!(
            "insert into repositories (id, installation_id, owner, name) \
             values ($1, $2, $3, $4) \
             on conflict (id) do update set \
                 installation_id = excluded.installation_id, \
                 owner = excluded.owner, \
                 name = excluded.name, \
                 out_of_reach_at = null \
             returning {COLUMNS}"
        ))
        .bind(id)
        .bind(installation_id)
        .bind(owner)
        .bind(name)
        .fetch_one(self.pool())
        .await?;
        Ok(row)
    }

    pub async fn repository(
        &self,
        owner: &str,
        name: &str,
    ) -> Result<Option<Repository>, StoreError> {
        let row = sqlx::query_as::<_, Repository>(&format!(
            "select {COLUMNS} from repositories where owner = $1 and name = $2"
        ))
        .bind(owner)
        .bind(name)
        .fetch_optional(self.pool())
        .await?;
        Ok(row)
    }

    pub async fn repository_by_id(&self, id: i64) -> Result<Option<Repository>, StoreError> {
        let row = sqlx::query_as::<_, Repository>(&format!(
            "select {COLUMNS} from repositories where id = $1"
        ))
        .bind(id)
        .fetch_optional(self.pool())
        .await?;
        Ok(row)
    }

    pub async fn repositories(&self) -> Result<Vec<Repository>, StoreError> {
        let rows = sqlx::query_as::<_, Repository>(&format!(
            "select {COLUMNS} from repositories order by owner, name"
        ))
        .fetch_all(self.pool())
        .await?;
        Ok(rows)
    }

    pub async fn repository_is_out_of_reach(&self, id: i64) -> Result<(), StoreError> {
        sqlx::query("update repositories set out_of_reach_at = now() where id = $1")
            .bind(id)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    pub async fn set_repository_active(&self, id: i64, active: bool) -> Result<(), StoreError> {
        sqlx::query("update repositories set active = $2 where id = $1")
            .bind(id)
            .bind(active)
            .execute(self.pool())
            .await?;
        Ok(())
    }
}
