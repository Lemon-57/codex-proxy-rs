//! 使用精确原生刷新观测与当前成员关系完成增量归组

use gateway_admin::model::{
    accounts::{NativeRefreshGroupCandidate, NativeRefreshGroupMove},
    audit::MutationAuditOperation,
};

use super::*;

macro_rules! eligible_rows {
    () => {
        "
    from account_groups source_group
    join account_group_accounts member on member.account_group_id = source_group.id
    join provider_accounts account on account.id = member.provider_account_id
    join provider_account_native_refresh_plans observation on observation.account_id = account.id
        and observation.credential_revision = account.credential_revision
    join account_groups target_group on target_group.name = $3 and target_group.enabled
    where account.provider_kind = $1 and source_group.name = $2 and source_group.enabled
      and source_group.id <> target_group.id and observation.plan_type = $4
      and account.credential_state = 'ready' and account.created_at <= observation.observed_at
      and member.created_at <= observation.observed_at"
    };
}

pub(super) async fn candidates(
    pool: &PgPool,
    command: &NativeRefreshGroupMove,
) -> AdminStoreResult<Vec<NativeRefreshGroupCandidate>> {
    let rows = sqlx::query_as::<_, (String, i64)>(concat!(
        "select account.id, account.credential_revision ",
        eligible_rows!(),
        " order by observation.observed_at, account.id limit 32"
    ))
    .bind(command.provider.as_str())
    .bind(&command.source_group)
    .bind(&command.target_group)
    .bind(&command.plan_type)
    .fetch_all(pool)
    .await
    .map_err(|error| unavailable("read native refresh group candidates", error))?;
    rows.into_iter()
        .map(|(account_id, revision)| {
            let credential_revision = to_u64(revision)
                .and_then(Revision::new)
                .map_err(|error| admin_store_error("provider_account", error))?;
            Ok(NativeRefreshGroupCandidate {
                account_id,
                credential_revision: admin_revision(credential_revision)?,
            })
        })
        .collect()
}

pub(super) async fn move_member(
    pool: &PgPool,
    command: &NativeRefreshGroupMove,
    candidate: &NativeRefreshGroupCandidate,
    context: &MutationContext,
) -> AdminStoreResult<Option<AdminRevision>> {
    let revision = to_i64(candidate.credential_revision.get())
        .map_err(|error| admin_store_error("provider_account", error))?;
    let mut transaction = pool
        .begin()
        .await
        .map_err(|error| unavailable("begin native refresh group update", error))?;
    sqlx::query("set local lock_timeout = '100ms'")
        .execute(&mut *transaction)
        .await
        .map_err(|error| unavailable("bound native refresh group lock wait", error))?;
    let config_revision = bump_config_revision_in_transaction(&mut transaction)
        .await
        .map_err(|error| admin_store_error("provider_account", error))?;
    let edges = sqlx::query_as::<_, (String, String)>(concat!(
        "select source_group.id, target_group.id ",
        eligible_rows!(),
        " and account.id = $5 and account.credential_revision = $6 for update of account"
    ))
    .bind(command.provider.as_str())
    .bind(&command.source_group)
    .bind(&command.target_group)
    .bind(&command.plan_type)
    .bind(&candidate.account_id)
    .bind(revision)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|error| unavailable("lock native refresh group candidate", error))?;
    let Some((source, target)) = edges else {
        transaction
            .rollback()
            .await
            .map_err(|error| unavailable("discard stale native refresh group update", error))?;
        return Ok(None);
    };
    sqlx::query("delete from account_group_accounts where account_group_id = $1 and provider_account_id = $2")
        .bind(source).bind(&candidate.account_id).execute(&mut *transaction).await
        .map_err(|error| unavailable("remove source group member after refresh", error))?;
    sqlx::query(
        "insert into account_group_accounts (account_group_id, provider_account_id, created_at)
        values ($1, $2, clock_timestamp()) on conflict do nothing",
    )
    .bind(target)
    .bind(&candidate.account_id)
    .execute(&mut *transaction)
    .await
    .map_err(|error| unavailable("add target group member after refresh", error))?;
    let audit = mutation_audit(
        context,
        MutationAuditOperation::ProviderAccountBatchUpdate,
        &candidate.account_id,
        vec!["groups".to_owned()],
    );
    append_admin_audit_event_in_transaction(&mut transaction, audit, config_revision)
        .await
        .map_err(|error| admin_store_error("provider_account", error))?;
    transaction
        .commit()
        .await
        .map_err(|error| unavailable("commit native refresh group update", error))?;
    admin_revision(config_revision).map(Some)
}

fn unavailable(operation: &'static str, error: sqlx::Error) -> AdminStoreError {
    admin_store_error("provider_account", postgres_unavailable(operation, error))
}
