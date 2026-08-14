pub mod audit;
pub mod credentials;
pub mod deliveries;
pub mod jobs;
pub mod queue;
pub mod repositories;
pub mod sealed;
pub mod sessions;

use sqlx::postgres::{PgPool, PgPoolOptions};

pub use sealed::{Seal, SealError, fresh_key, key_from_hex};

#[derive(Clone)]
pub struct Store {
    pool: PgPool,
    seal: Seal,
}

impl Store {
    pub async fn open(url: &str, seal: Seal) -> Result<Self, StoreError> {
        let pool = PgPoolOptions::new()
            .max_connections(16)
            .connect(url)
            .await
            .map_err(|problem| StoreError::Unreachable {
                problem: problem.to_string(),
            })?;
        sqlx::migrate!("../../migrations").run(&pool).await?;
        Ok(Self { pool, seal })
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub fn seal(&self) -> &Seal {
        &self.seal
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("the database at DATABASE_URL could not be reached: {problem}")]
    Unreachable { problem: String },
    #[error("the database schema could not be brought up to date: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("the database refused a query: {0}")]
    Refused(#[from] sqlx::Error),
    #[error(transparent)]
    Sealed(#[from] SealError),
}
