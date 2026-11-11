//! PostgreSQL 迁移屏障、连接池与会话级资源预算

use sqlx::{
    PgPool,
    postgres::{PgConnectOptions, PgPoolOptions},
};

use crate::{
    POSTGRES_IDLE_TRANSACTION_TIMEOUT, POSTGRES_LOCK_TIMEOUT, POSTGRES_STATEMENT_TIMEOUT,
    StoreError, StorePoolConfig, StoreResult, postgres_unavailable,
};

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations");

/// 建立 PostgreSQL pool 并只执行冻结的 migration 集
pub async fn connect_and_migrate(
    database_url: &str,
    pool_config: StorePoolConfig,
) -> StoreResult<PgPool> {
    if database_url.trim().is_empty() {
        return Err(StoreError::InvalidData {
            source: None,
            entity: "PostgreSQL configuration",
            message: "database URL is empty".to_owned(),
        });
    }
    pool_config.validate()?;
    let connect_options = database_url
        .parse::<PgConnectOptions>()
        .map_err(|source| postgres_unavailable("parse PostgreSQL connection options", source))?;
    let migration_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(
            connect_options
                .clone()
                .application_name("codex-proxy-rs:migration"),
        )
        .await
        .map_err(|source| postgres_unavailable("connect PostgreSQL for migrations", source))?;
    // 过渡与回退制品仅校验已应用的冻结前缀，不在旧连接排空前执行新增迁移
    let applied = sqlx::query_as::<_, (i64, Vec<u8>, bool)>(
        "select version, checksum, success from _sqlx_migrations order by version",
    )
    .fetch_all(&migration_pool)
    .await
    .map_err(|source| postgres_unavailable("validate existing migrations", source))?;
    let maximum = applied.last().map(|entry| entry.0).unwrap_or(0);
    let expected = MIGRATOR
        .iter()
        .filter(|entry| entry.version <= maximum)
        .collect::<Vec<_>>();
    let valid = (43..=52).contains(&maximum)
        && applied.len() == expected.len()
        && applied
            .iter()
            .zip(expected)
            .all(|((version, checksum, success), migration)| {
                *success
                    && *version == migration.version
                    && checksum.as_slice() == migration.checksum.as_ref()
            });
    migration_pool.close().await;
    if !valid {
        return Err(StoreError::InvalidData {
            source: None,
            entity: "PostgreSQL migration manifest",
            message: "schema does not match the verified rolling-upgrade prefix".to_owned(),
        });
    }

    connect_pool(connect_options, pool_config, false).await
}

/// 帮助查询不执行迁移，并用连接默认只读事务阻止意外业务写入
pub(crate) async fn connect_read_only(
    database_url: &str,
    pool_config: StorePoolConfig,
) -> StoreResult<PgPool> {
    pool_config.validate()?;
    let options = database_url
        .parse::<PgConnectOptions>()
        .map_err(|source| postgres_unavailable("parse PostgreSQL connection options", source))?;
    connect_pool(options, pool_config, true).await
}

async fn connect_pool(
    connect_options: PgConnectOptions,
    pool_config: StorePoolConfig,
    read_only: bool,
) -> StoreResult<PgPool> {
    let statement_timeout = postgres_duration_setting(POSTGRES_STATEMENT_TIMEOUT);
    let lock_timeout = postgres_duration_setting(POSTGRES_LOCK_TIMEOUT);
    let idle_in_transaction_session_timeout =
        postgres_duration_setting(POSTGRES_IDLE_TRANSACTION_TIMEOUT);
    let pool = PgPoolOptions::new()
        .max_connections(pool_config.max_connections)
        .acquire_timeout(std::time::Duration::from_secs(
            pool_config.acquire_timeout_seconds,
        ))
        .after_connect(move |connection, _metadata| {
            let statement_timeout = statement_timeout.clone();
            let lock_timeout = lock_timeout.clone();
            let idle_in_transaction_session_timeout = idle_in_transaction_session_timeout.clone();
            Box::pin(async move {
                sqlx::query(
                    "select set_config('statement_timeout', $1, false),
                            set_config('lock_timeout', $2, false),
                            set_config('idle_in_transaction_session_timeout', $3, false),
                            set_config('default_transaction_read_only', $4, false),
                            set_config('TimeZone', 'UTC', false)",
                )
                .bind(statement_timeout)
                .bind(lock_timeout)
                .bind(idle_in_transaction_session_timeout)
                .bind(if read_only { "on" } else { "off" })
                .execute(connection)
                .await?;
                Ok(())
            })
        })
        .connect_with(connect_options.application_name("codex-proxy-rs"))
        .await
        .map_err(|source| postgres_unavailable("connect PostgreSQL", source))?;
    Ok(pool)
}

fn postgres_duration_setting(duration: std::time::Duration) -> String {
    format!("{}ms", duration.as_millis())
}
