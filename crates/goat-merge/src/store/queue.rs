use sqlx::{Postgres, Transaction};
use time::OffsetDateTime;

use super::{Store, StoreError};

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Queue {
    pub id: i64,
    pub repository_id: i64,
    pub branch: String,
    pub paused: bool,
    pub paused_by: Option<String>,
    pub base_sha: Option<String>,
    pub base_moved_at: OffsetDateTime,
    pub verify_at_once: i32,
    pub verify_at_once_because: Option<String>,
    pub speculate_to: i32,
    pub speculate_to_because: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Entry {
    pub id: i64,
    pub queue_id: i64,
    pub pull_request: i32,
    pub title: String,
    pub requested_by: String,
    pub requested_at: OffsetDateTime,
    pub queued_at: Option<OffsetDateTime>,
    pub priority: i32,
    pub expedited_by: Option<String>,
    pub expedite_note: Option<String>,
    pub status: String,
    pub status_detail: String,
    pub settled_at: Option<OffsetDateTime>,
    pub merged_sha: Option<String>,
    pub merged_head: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Attempt {
    pub id: i64,
    pub queue_id: i64,
    pub base: String,
    pub candidate_branch: String,
    pub candidate_pull_request: Option<i32>,
    pub candidate_sha: Option<String>,
    pub conclusion: String,
    pub failed_checks: Vec<String>,
    pub started_at: OffsetDateTime,
    pub finished_at: Option<OffsetDateTime>,
    pub discarded_because: Option<String>,
    pub narrowed_from: Option<i64>,
    pub built_on: Option<i64>,
    pub depth: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct WhatItRode {
    pub entry_id: i64,
    #[sqlx(flatten)]
    pub attempt: Attempt,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Assumption {
    pub entry_id: i64,
    pub pull_request: i32,
    pub head: String,
    pub settled_at: Option<OffsetDateTime>,
    pub merged_sha: Option<String>,
    pub merged_head: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct StandingOn<'a> {
    pub attempt: &'a Attempt,
    pub assumed: &'a [(i64, String)],
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Member {
    pub entry_id: i64,
    pub pull_request: i32,
    pub head: String,
}

const QUEUE_COLUMNS: &str = "id, repository_id, branch, paused, paused_by, base_sha, \
                             base_moved_at, verify_at_once, verify_at_once_because, \
                             speculate_to, speculate_to_because";
const ENTRY_COLUMNS: &str = "id, queue_id, pull_request, title, requested_by, requested_at, \
                             queued_at, priority, expedited_by, expedite_note, status, \
                             status_detail, settled_at, merged_sha, merged_head";
const ATTEMPT_COLUMNS: &str = "id, queue_id, base, candidate_branch, candidate_pull_request, \
                               candidate_sha, conclusion, failed_checks, started_at, finished_at, \
                               discarded_because, narrowed_from, built_on, depth";
const RUNNING_ORDER: &str = "order by priority desc, queued_at asc nulls last, requested_at asc";

impl Store {
    pub async fn queue_for(&self, repository_id: i64, branch: &str) -> Result<Queue, StoreError> {
        let queue = sqlx::query_as::<_, Queue>(&format!(
            "insert into queues (repository_id, branch) values ($1, $2) \
             on conflict (repository_id, branch) do update set branch = excluded.branch \
             returning {QUEUE_COLUMNS}"
        ))
        .bind(repository_id)
        .bind(branch)
        .fetch_one(self.pool())
        .await?;
        Ok(queue)
    }

    pub async fn queue_on(
        &self,
        repository_id: i64,
        branch: &str,
    ) -> Result<Option<Queue>, StoreError> {
        let queue = sqlx::query_as::<_, Queue>(&format!(
            "select {QUEUE_COLUMNS} from queues where repository_id = $1 and branch = $2"
        ))
        .bind(repository_id)
        .bind(branch)
        .fetch_optional(self.pool())
        .await?;
        Ok(queue)
    }

    pub async fn queue_by_id(&self, id: i64) -> Result<Option<Queue>, StoreError> {
        let queue = sqlx::query_as::<_, Queue>(&format!(
            "select {QUEUE_COLUMNS} from queues where id = $1"
        ))
        .bind(id)
        .fetch_optional(self.pool())
        .await?;
        Ok(queue)
    }

    pub async fn every_queue(&self) -> Result<Vec<Queue>, StoreError> {
        let queues = sqlx::query_as::<_, Queue>(&format!(
            "select q.{QUEUE_COLUMNS} from queues q \
             join repositories r on r.id = q.repository_id \
             join installations i on i.id = r.installation_id \
             where r.active and r.out_of_reach_at is null and i.removed_at is null \
             order by q.id"
        ))
        .fetch_all(self.pool())
        .await?;
        Ok(queues)
    }

    pub async fn queues_of(&self, repository_id: i64) -> Result<Vec<Queue>, StoreError> {
        let queues = sqlx::query_as::<_, Queue>(&format!(
            "select {QUEUE_COLUMNS} from queues where repository_id = $1 order by branch"
        ))
        .bind(repository_id)
        .fetch_all(self.pool())
        .await?;
        Ok(queues)
    }

    pub async fn note_where_the_base_is(
        &self,
        queue_id: i64,
        sha: &str,
    ) -> Result<Queue, StoreError> {
        let queue = sqlx::query_as::<_, Queue>(&format!(
            "update queues set \
                 base_sha = $2, \
                 base_moved_at = case when base_sha is distinct from $2 \
                                      then now() else base_moved_at end \
             where id = $1 returning {QUEUE_COLUMNS}"
        ))
        .bind(queue_id)
        .bind(sha)
        .fetch_one(self.pool())
        .await?;
        Ok(queue)
    }

    pub async fn set_paused(
        &self,
        queue_id: i64,
        paused: bool,
        by: &str,
    ) -> Result<Queue, StoreError> {
        let queue = sqlx::query_as::<_, Queue>(&format!(
            "update queues set paused = $2, \
                 paused_by = case when $2 then $3 else null end, \
                 paused_at = case when $2 then now() else null end \
             where id = $1 returning {QUEUE_COLUMNS}"
        ))
        .bind(queue_id)
        .bind(paused)
        .bind(by)
        .fetch_one(self.pool())
        .await?;
        Ok(queue)
    }

    pub async fn enter(
        &self,
        queue_id: i64,
        pull_request: i32,
        requested_by: &str,
        title: &str,
    ) -> Result<Entry, StoreError> {
        let entry = sqlx::query_as::<_, Entry>(&format!(
            "insert into entries (queue_id, pull_request, title, requested_by, status) \
             values ($1, $2, $4, $3, 'Waiting') \
             on conflict (queue_id, pull_request) do update set \
                 title = excluded.title, \
                 requested_by = excluded.requested_by, \
                 requested_at = now(), \
                 queued_at = null, \
                 settled_at = null, \
                 merged_sha = null, \
                 merged_head = null, \
                 priority = 0, \
                 expedited_by = null, \
                 expedite_note = null \
             returning {ENTRY_COLUMNS}"
        ))
        .bind(queue_id)
        .bind(pull_request)
        .bind(requested_by)
        .bind(title)
        .fetch_one(self.pool())
        .await?;
        Ok(entry)
    }

    pub async fn entry(
        &self,
        queue_id: i64,
        pull_request: i32,
    ) -> Result<Option<Entry>, StoreError> {
        let entry = sqlx::query_as::<_, Entry>(&format!(
            "select {ENTRY_COLUMNS} from entries where queue_id = $1 and pull_request = $2"
        ))
        .bind(queue_id)
        .bind(pull_request)
        .fetch_optional(self.pool())
        .await?;
        Ok(entry)
    }

    pub async fn running(&self, queue_id: i64) -> Result<Vec<Entry>, StoreError> {
        let entries = sqlx::query_as::<_, Entry>(&format!(
            "select {ENTRY_COLUMNS} from entries \
             where queue_id = $1 and settled_at is null {RUNNING_ORDER}"
        ))
        .bind(queue_id)
        .fetch_all(self.pool())
        .await?;
        Ok(entries)
    }

    pub async fn history(&self, queue_id: i64, most: i64) -> Result<Vec<Entry>, StoreError> {
        let entries = sqlx::query_as::<_, Entry>(&format!(
            "select {ENTRY_COLUMNS} from entries \
             where queue_id = $1 and settled_at is not null \
             order by settled_at desc limit $2"
        ))
        .bind(queue_id)
        .bind(most)
        .fetch_all(self.pool())
        .await?;
        Ok(entries)
    }

    pub async fn take_a_number(&self, entry_id: i64) -> Result<(), StoreError> {
        sqlx::query("update entries set queued_at = now() where id = $1 and queued_at is null")
            .bind(entry_id)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    pub async fn give_up_the_number(&self, entry_id: i64) -> Result<(), StoreError> {
        sqlx::query("update entries set queued_at = null where id = $1")
            .bind(entry_id)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    pub async fn remember_title(&self, entry_id: i64, title: &str) -> Result<(), StoreError> {
        sqlx::query("update entries set title = $2 where id = $1 and title is distinct from $2")
            .bind(entry_id)
            .bind(title)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    pub async fn record_status(
        &self,
        entry_id: i64,
        headline: &str,
        detail: &str,
    ) -> Result<bool, StoreError> {
        let changed = sqlx::query(
            "update entries set status = $2, status_detail = $3 \
             where id = $1 and (status is distinct from $2 or status_detail is distinct from $3)",
        )
        .bind(entry_id)
        .bind(headline)
        .bind(detail)
        .execute(self.pool())
        .await?;
        Ok(changed.rows_affected() > 0)
    }

    pub async fn settle(
        &self,
        entry_id: i64,
        headline: &str,
        detail: &str,
        merged_sha: Option<&str>,
        merged_head: Option<&str>,
    ) -> Result<(), StoreError> {
        sqlx::query(
            "update entries set status = $2, status_detail = $3, merged_sha = $4, \
                 merged_head = $5, settled_at = now(), queued_at = null \
             where id = $1",
        )
        .bind(entry_id)
        .bind(headline)
        .bind(detail)
        .bind(merged_sha)
        .bind(merged_head)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn expedite(&self, entry_id: i64, by: &str, note: &str) -> Result<(), StoreError> {
        sqlx::query(
            "update entries set priority = 100, expedited_by = $2, expedite_note = $3 \
             where id = $1",
        )
        .bind(entry_id)
        .bind(by)
        .bind(note)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn open_attempt(
        &self,
        queue_id: i64,
        base: &str,
        candidate_branch: &str,
        aboard: &[(i64, String)],
        narrowed_from: Option<i64>,
        standing_on: Option<StandingOn<'_>>,
    ) -> Result<Attempt, StoreError> {
        let mut writing = self.pool().begin().await?;
        let attempt = sqlx::query_as::<_, Attempt>(&format!(
            "insert into verifications \
                 (queue_id, base, candidate_branch, narrowed_from, built_on, depth) \
             values ($1, $2, $3, $4, $5, $6) returning {ATTEMPT_COLUMNS}"
        ))
        .bind(queue_id)
        .bind(base)
        .bind(candidate_branch)
        .bind(narrowed_from)
        .bind(standing_on.map(|below| below.attempt.id))
        .bind(standing_on.map_or(0, |below| below.attempt.depth + 1))
        .fetch_one(&mut *writing)
        .await?;
        for (place, (entry_id, head)) in aboard.iter().enumerate() {
            sqlx::query(
                "insert into verification_members (verification_id, entry_id, head, place) \
                 values ($1, $2, $3, $4)",
            )
            .bind(attempt.id)
            .bind(entry_id)
            .bind(head)
            .bind(i32::try_from(place).unwrap_or(i32::MAX))
            .execute(&mut *writing)
            .await?;
        }
        let assumed = standing_on.map_or(&[][..], |below| below.assumed);
        for (place, (entry_id, head)) in assumed.iter().enumerate() {
            sqlx::query(
                "insert into verification_assumptions (verification_id, entry_id, head, place) \
                 values ($1, $2, $3, $4)",
            )
            .bind(attempt.id)
            .bind(entry_id)
            .bind(head)
            .bind(i32::try_from(place).unwrap_or(i32::MAX))
            .execute(&mut *writing)
            .await?;
        }
        writing.commit().await?;
        Ok(attempt)
    }

    pub async fn live_attempts_on(&self, queue_id: i64) -> Result<Vec<Attempt>, StoreError> {
        let attempts = sqlx::query_as::<_, Attempt>(&format!(
            "select {ATTEMPT_COLUMNS} from verifications \
             where queue_id = $1 and discarded_at is null \
             order by depth asc, started_at asc"
        ))
        .bind(queue_id)
        .fetch_all(self.pool())
        .await?;
        Ok(attempts)
    }

    pub async fn what_last_failed_assuming(
        &self,
        queue_id: i64,
        assumed: &[i64],
    ) -> Result<Option<Attempt>, StoreError> {
        let attempt = sqlx::query_as::<_, Attempt>(&format!(
            "select {ATTEMPT_COLUMNS} from verifications v \
             where v.queue_id = $1 \
               and coalesce(( \
                   select array_agg(a.entry_id order by a.entry_id) \
                   from verification_assumptions a where a.verification_id = v.id \
               ), '{{}}') = $2 \
             order by v.started_at desc limit 1"
        ))
        .bind(queue_id)
        .bind({
            let mut sorted = assumed.to_vec();
            sorted.sort_unstable();
            sorted
        })
        .fetch_optional(self.pool())
        .await?;
        Ok(attempt)
    }

    pub async fn what_it_assumed(&self, attempt_id: i64) -> Result<Vec<Assumption>, StoreError> {
        let assumed = sqlx::query_as::<_, Assumption>(
            "select a.entry_id, e.pull_request, a.head, \
                    e.settled_at, e.merged_sha, e.merged_head \
             from verification_assumptions a \
             join entries e on e.id = a.entry_id \
             where a.verification_id = $1 order by a.place",
        )
        .bind(attempt_id)
        .fetch_all(self.pool())
        .await?;
        Ok(assumed)
    }

    pub async fn everything_built_on(&self, attempt_id: i64) -> Result<Vec<Attempt>, StoreError> {
        let built = sqlx::query_as::<_, Attempt>(&format!(
            "with recursive standing_on (id) as ( \
                 select id from verifications where built_on = $1 \
                 union all \
                 select v.id from verifications v \
                 join standing_on s on v.built_on = s.id \
             ) \
             select {ATTEMPT_COLUMNS} from verifications \
             where id in (select id from standing_on) order by depth desc"
        ))
        .bind(attempt_id)
        .fetch_all(self.pool())
        .await?;
        Ok(built)
    }

    pub async fn verification_carrying(
        &self,
        entry_id: i64,
    ) -> Result<Option<Attempt>, StoreError> {
        let attempt = sqlx::query_as::<_, Attempt>(&format!(
            "select v.{ATTEMPT_COLUMNS} from verifications v \
             join verification_members m on m.verification_id = v.id \
             where m.entry_id = $1 and v.discarded_at is null \
             order by v.started_at desc limit 1"
        ))
        .bind(entry_id)
        .fetch_optional(self.pool())
        .await?;
        Ok(attempt)
    }

    pub async fn everyone_aboard(&self, attempt_id: i64) -> Result<Vec<Member>, StoreError> {
        let aboard = sqlx::query_as::<_, Member>(
            "select m.entry_id, e.pull_request, m.head from verification_members m \
             join entries e on e.id = m.entry_id \
             where m.verification_id = $1 order by m.place",
        )
        .bind(attempt_id)
        .fetch_all(self.pool())
        .await?;
        Ok(aboard)
    }

    pub async fn what_each_of_them_last_rode(
        &self,
        entry_ids: &[i64],
    ) -> Result<Vec<WhatItRode>, StoreError> {
        if entry_ids.is_empty() {
            return Ok(Vec::new());
        }
        let rode = sqlx::query_as::<_, WhatItRode>(&format!(
            "select distinct on (m.entry_id) m.entry_id, v.{ATTEMPT_COLUMNS} \
             from verification_members m \
             join verifications v on v.id = m.verification_id \
             where m.entry_id = any($1) \
             order by m.entry_id, (v.discarded_at is null) desc, v.started_at desc"
        ))
        .bind(entry_ids)
        .fetch_all(self.pool())
        .await?;
        Ok(rode)
    }

    pub async fn attempts_carrying(&self, entry_id: i64) -> Result<Vec<Attempt>, StoreError> {
        let attempts = sqlx::query_as::<_, Attempt>(&format!(
            "select v.{ATTEMPT_COLUMNS} from verifications v \
             join verification_members m on m.verification_id = v.id \
             where m.entry_id = $1 order by v.started_at asc"
        ))
        .bind(entry_id)
        .fetch_all(self.pool())
        .await?;
        Ok(attempts)
    }

    pub async fn speculate_this_deep_next_time(
        &self,
        queue_id: i64,
        how_deep: usize,
        because: &str,
    ) -> Result<(), StoreError> {
        sqlx::query("update queues set speculate_to = $2, speculate_to_because = $3 where id = $1")
            .bind(queue_id)
            .bind(i32::try_from(how_deep).unwrap_or(i32::MAX))
            .bind(because)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    pub async fn verify_this_many_next_time(
        &self,
        queue_id: i64,
        how_many: usize,
        because: &str,
    ) -> Result<(), StoreError> {
        sqlx::query(
            "update queues set verify_at_once = $2, verify_at_once_because = $3 where id = $1",
        )
        .bind(queue_id)
        .bind(i32::try_from(how_many).unwrap_or(i32::MAX))
        .bind(because)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn attach_candidate(
        &self,
        attempt_id: i64,
        pull_request: Option<i32>,
        sha: &str,
    ) -> Result<(), StoreError> {
        sqlx::query(
            "update verifications set candidate_pull_request = $2, candidate_sha = $3 \
             where id = $1",
        )
        .bind(attempt_id)
        .bind(pull_request)
        .bind(sha)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn conclude_attempt(
        &self,
        attempt_id: i64,
        conclusion: &str,
        failed_checks: &[String],
    ) -> Result<(), StoreError> {
        sqlx::query(
            "update verifications set conclusion = $2, failed_checks = $3, finished_at = now() \
             where id = $1",
        )
        .bind(attempt_id)
        .bind(conclusion)
        .bind(failed_checks)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn discard_attempt(&self, attempt_id: i64, because: &str) -> Result<(), StoreError> {
        sqlx::query(
            "update verifications set discarded_at = now(), discarded_because = $2 \
             where id = $1 and discarded_at is null",
        )
        .bind(attempt_id)
        .bind(because)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn hold_the_branch(
        &self,
        queue_id: i64,
    ) -> Result<Transaction<'static, Postgres>, StoreError> {
        let mut held = self.pool().begin().await?;
        sqlx::query("select pg_advisory_xact_lock($1)")
            .bind(queue_id)
            .execute(&mut *held)
            .await?;
        Ok(held)
    }
}
