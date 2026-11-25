//! 验证原生刷新证据、成员关系变更及版本竞争保护

use gateway_admin::model::accounts::NativeRefreshGroupMove;

use super::*;

#[tokio::test]
async fn move_only_current_native_refresh_group_member() {
    let Some(database) = TestDatabase::create("native_refresh_groups").await else {
        return;
    };
    let repository = PgProviderAccountRepository::new(database.pool.clone());
    let store = admin_account_store(&database.pool);
    let account_id = "acct_native_group";
    let mut fixture = account(account_id, "native-group-user");
    fixture.enabled = false;
    fixture.plan_type = Some("free".into());
    repository.insert_provider_account(fixture).await.unwrap();
    let source = "grp_00000000000000000000000000000001";
    let target = "grp_00000000000000000000000000000002";
    let other = "grp_00000000000000000000000000000003";
    for (id, name) in [(source, "GPT Pro"), (target, "Free Wait"), (other, "Other")] {
        sqlx::query("insert into account_groups (id,name,created_at,updated_at,color) values ($1,$2,now(),now(),'#114477FF')")
            .bind(id).bind(name).execute(&database.pool).await.unwrap();
    }
    for group in [source, target, other] {
        sqlx::query("insert into account_group_accounts values ($1,$2,now())")
            .bind(group)
            .bind(account_id)
            .execute(&database.pool)
            .await
            .unwrap();
    }
    let command = NativeRefreshGroupMove {
        provider: ProviderKind::new("openai").unwrap(),
        source_group: "GPT Pro".into(),
        target_group: "Free Wait".into(),
        plan_type: "free".into(),
    };
    assert!(
        store
            .native_refresh_group_candidates(&command)
            .await
            .unwrap()
            .is_empty()
    );
    let identity = ProviderAccountId::new(account_id).unwrap();
    let provider_account = repository
        .load_current_credential(&identity)
        .await
        .unwrap()
        .account;
    let update = CredentialCasUpdate::new(
        identity.clone(),
        provider_account.revision(),
        ProviderAccountUpdate {
            account_id: identity,
            name: account_id.into(),
            email: None,
            plan_type: None,
        },
        plaintext_credential("fresh-fixture"),
        true,
        provider_account.access_token_expires_at(),
        None,
    )
    .unwrap()
    .preserving_profile()
    .with_native_refresh_plan(Some("free".into()));
    assert!(matches!(
        repository
            .compare_and_swap_credential(update)
            .await
            .unwrap(),
        CredentialCasOutcome::Updated(_)
    ));
    let candidates = store
        .native_refresh_group_candidates(&command)
        .await
        .unwrap();
    assert_eq!(candidates.len(), 1);
    let context = MutationContext {
        actor: MutationActor::System,
        request_id: "native-refresh-test".into(),
    };
    let mut stale = candidates[0].clone();
    stale.credential_revision = gateway_admin::model::Revision::new(1).unwrap();
    assert!(
        store
            .move_native_refresh_group_member(&command, &stale, &context)
            .await
            .unwrap()
            .is_none()
    );
    sqlx::query("update account_groups set enabled = false where id = $1")
        .bind(target)
        .execute(&database.pool)
        .await
        .unwrap();
    assert!(
        store
            .move_native_refresh_group_member(&command, &candidates[0], &context)
            .await
            .unwrap()
            .is_none()
    );
    sqlx::query("update account_groups set enabled = true where id = $1")
        .bind(target)
        .execute(&database.pool)
        .await
        .unwrap();
    assert!(
        store
            .move_native_refresh_group_member(&command, &candidates[0], &context)
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        store
            .move_native_refresh_group_member(&command, &candidates[0], &context)
            .await
            .unwrap()
            .is_none()
    );
    let groups: Vec<String> = sqlx::query_scalar("select account_group_id from account_group_accounts where provider_account_id=$1 order by 1")
        .bind(account_id).fetch_all(&database.pool).await.unwrap();
    assert_eq!(groups, [target, other]);
    let after: (bool, i64, String) = sqlx::query_as("select enabled,credential_revision,provider_credentials_json->>'access_token' from provider_accounts where id=$1")
        .bind(account_id).fetch_one(&database.pool).await.unwrap();
    assert_eq!(after, (false, 2, "fresh-fixture".into()));
    sqlx::query(
        "insert into account_group_accounts values ($1,$2,clock_timestamp() + interval '1 second')",
    )
    .bind(source)
    .bind(account_id)
    .execute(&database.pool)
    .await
    .unwrap();
    assert!(
        store
            .native_refresh_group_candidates(&command)
            .await
            .unwrap()
            .is_empty()
    );
    database.close().await;
}
