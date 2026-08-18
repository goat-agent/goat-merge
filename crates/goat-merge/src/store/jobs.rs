use time::{Duration, OffsetDateTime};

use super::{Store, StoreError};

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Job {
    pub id: i64,
    pub kind: String,
    pub subject: String,
    pub attempts: i32,
}

const COLUMNS: &str = "id, kind, subject, attempts";

impl Store {
    pub async fn ask_for(&self, kind: &str, subject: &str) -> Result<(), StoreError> {
        self.put_in_a_request(kind, subject, None).await
    }

    pub async fn ask_for_later(
        &self,
        kind: &str,
        subject: &str,
        after: Duration,
    ) -> Result<(), StoreError> {
        self.put_in_a_request(kind, subject, Some(OffsetDateTime::now_utc() + after))
            .await
    }

    async fn put_in_a_request(
        &self,
        kind: &str,
        subject: &str,
        when: Option<OffsetDateTime>,
    ) -> Result<(), StoreError> {
        sqlx::query(
            "insert into jobs (kind, subject, run_after) values ($1, $2, coalesce($3, now())) \
             on conflict (kind, subject) do update set \
                 run_after = case when jobs.locked_at is null \
                                  then least(jobs.run_after, coalesce($3, now())) \
                                  else jobs.run_after end, \
                 asked_again_for = case when jobs.locked_at is null \
                                        then jobs.asked_again_for \
                                        else least(jobs.asked_again_for, coalesce($3, now())) \
                                   end",
        )
        .bind(kind)
        .bind(subject)
        .bind(when)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn claim_a_job(&self, worker: &str) -> Result<Option<Job>, StoreError> {
        let job = sqlx::query_as::<_, Job>(&format!(
            "update jobs set locked_at = now(), locked_by = $1, attempts = attempts + 1 \
             where id = ( \
                 select id from jobs \
                 where locked_at is null and run_after <= now() \
                 order by run_after \
                 for update skip locked \
                 limit 1 \
             ) returning {COLUMNS}"
        ))
        .bind(worker)
        .fetch_optional(self.pool())
        .await?;
        Ok(job)
    }

    pub async fn job_is_done(&self, id: i64) -> Result<(), StoreError> {
        let cleared = sqlx::query("delete from jobs where id = $1 and asked_again_for is null")
            .bind(id)
            .execute(self.pool())
            .await?;
        if cleared.rows_affected() > 0 {
            return Ok(());
        }
        sqlx::query(
            "update jobs set run_after = asked_again_for, asked_again_for = null, \
                 locked_at = null, locked_by = null, attempts = 0, last_error = null \
             where id = $1 and asked_again_for is not null",
        )
        .bind(id)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn job_went_wrong(
        &self,
        id: i64,
        problem: &str,
        try_again_in: Duration,
    ) -> Result<(), StoreError> {
        let when = OffsetDateTime::now_utc() + try_again_in;
        sqlx::query(
            "update jobs set locked_at = null, locked_by = null, asked_again_for = null, \
                 last_error = $2, run_after = $3 \
             where id = $1",
        )
        .bind(id)
        .bind(problem)
        .bind(when)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn release_jobs_held_since_before(&self, stale: Duration) -> Result<u64, StoreError> {
        let cutoff = OffsetDateTime::now_utc() - stale;
        let freed = sqlx::query(
            "update jobs set locked_at = null, locked_by = null, asked_again_for = null, \
                 run_after = least(run_after, asked_again_for) \
             where locked_at is not null and locked_at < $1",
        )
        .bind(cutoff)
        .execute(self.pool())
        .await?;
        Ok(freed.rows_affected())
    }
}
