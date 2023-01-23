use spdlog::prelude::*;
use sqlx::sqlite::SqlitePool;

pub struct Pool {
    pool: SqlitePool,
}

impl Pool {
    pub async fn connect(url: impl AsRef<str>) -> anyhow::Result<Self> {
        let url = url.as_ref();

        info!("connecting to database '{url}'");

        let pool = SqlitePool::connect(url).await?;

        info!("database connected");

        sqlx::migrate!().run(&pool).await?;

        Ok(Self { pool })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}
