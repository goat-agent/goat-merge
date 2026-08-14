#![allow(dead_code)]

pub mod fake_github;

use std::sync::atomic::{AtomicI64, Ordering};

use goat_merge::store::repositories::Repository;
use goat_merge::store::{Seal, Store, fresh_key, key_from_hex};

static NEXT_ID: AtomicI64 = AtomicI64::new(1);

pub fn an_id_of_its_own() -> i64 {
    i64::from(std::process::id()) * 1_000_000_000 + NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

pub fn a_seal() -> Seal {
    Seal::holding(key_from_hex(&fresh_key()).expect("a fresh key is 32 bytes"))
}

pub async fn a_store() -> Option<Store> {
    a_store_sealed_with(a_seal()).await
}

pub async fn a_store_sealed_with(seal: Seal) -> Option<Store> {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!(
            "SKIPPED: these tests need a database. Run `docker compose up -d db` and set \
             DATABASE_URL, or see AGENTS.md"
        );
        return None;
    };
    Some(
        Store::open(&url, seal)
            .await
            .expect("the database should accept us"),
    )
}

static THE_JOBS_TABLE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub async fn alone_with_the_jobs(store: &Store) -> tokio::sync::MutexGuard<'static, ()> {
    let held = THE_JOBS_TABLE.lock().await;
    sqlx::query("delete from jobs")
        .execute(store.pool())
        .await
        .expect("the jobs table should be clearable");
    held
}

pub async fn a_repository(store: &Store) -> Repository {
    let id = an_id_of_its_own();
    store
        .remember_installation(id, "acme")
        .await
        .expect("an installation should be remembered");
    store
        .remember_repository(id, id, "acme", &format!("api-{id}"))
        .await
        .expect("a repository should be remembered")
}
