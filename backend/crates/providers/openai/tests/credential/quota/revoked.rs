//! 额度端点明确撤销令牌时的状态、旧快照及并发轮换保护

use super::*;

#[tokio::test]
async fn revoked_quota_worker_preserves_pause_and_snapshot_without_marking_banned() {
    for enabled in [true, false] {
        let store = Arc::new(MemoryAccountStore::default());
        let id = format!("acct_revoked_worker_{enabled}");
        create_account_with_enabled(&store, &id, enabled).await;
        let account = store.account(&id).unwrap();
        let observation = parse_rate_limits_event(&json!({
            "type": "codex.rate_limits",
            "rate_limits": {
                "primary": {"used_percent": 100, "window_minutes": 300, "reset_at": 1900000000},
                "secondary": {"used_percent": 45, "window_minutes": 10080, "reset_at": 1900600000}
            }
        }))
        .unwrap();
        quota_service(&store)
            .synchronize_passive_rate_limits(&account, &[observation])
            .await
            .unwrap();
        persist_quota_state(&store, &account, exhausted_quota(None)).await;
        let before = store.account(&id).unwrap();
        let snapshot = store.quota_json(&id);
        assert!(snapshot.is_some());
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/codex/usage"))
            .respond_with(ResponseTemplate::new(401).set_body_json(json!({
                "error": {"code": "token_revoked"}
            })))
            .mount(&server)
            .await;
        let service = quota_service_with_base_url(&store, reqwest::Client::new(), server.uri());
        if enabled {
            let summary = service.synchronize().await.unwrap();
            assert_eq!(summary.banned, 0);
            assert_eq!(summary.stale, 1);
            assert_eq!(summary.transient, 0);
        } else {
            assert!(service.refresh_account(before.id()).await.is_err());
        }
        let current = store.account(&id).unwrap();
        assert_eq!(current.credential_state(), CredentialState::Invalid);
        assert_eq!(current.enabled(), enabled);
        assert_eq!(current.revision(), before.revision());
        assert_eq!(current.quota(), before.quota());
        assert_eq!(store.quota_json(&id), snapshot);
        assert_eq!(current.name(), before.name());
        assert_eq!(current.email(), before.email());
        assert_eq!(current.outbound_proxy(), before.outbound_proxy());
        assert_eq!(
            current.status_projection(SystemTime::now(), None).status,
            AccountStatus::Error
        );
    }
}

#[tokio::test]
async fn token_revoked_requires_401_and_structured_exact_error_code() {
    for (status, body) in [
        (403, json!({"error": {"code": "token_revoked"}})),
        (429, json!({"error": {"code": "token_revoked"}})),
        (503, json!({"error": {"code": "token_revoked"}})),
        (
            401,
            json!({"error": {"code": "unknown_auth_error", "message": "token_revoked"}}),
        ),
        (401, json!({"error": {"message": "token_revoked"}})),
    ] {
        let store = Arc::new(MemoryAccountStore::default());
        let id = "acct_unconfirmed_revocation";
        create_account(&store, id).await;
        let before = store.account(id).unwrap();
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/codex/usage"))
            .respond_with(ResponseTemplate::new(status).set_body_json(body))
            .mount(&server)
            .await;
        let service = quota_service_with_base_url(&store, reqwest::Client::new(), server.uri());
        assert!(service.refresh_account(before.id()).await.is_err());
        assert_eq!(store.account(id).unwrap(), before);
    }
}

#[tokio::test]
async fn old_revoked_query_cannot_invalidate_rotated_credential() {
    let store = Arc::new(MemoryAccountStore::default());
    let id = "acct_old_revocation";
    create_account(&store, id).await;
    let before = store.account(id).unwrap();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/codex/usage"))
        .respond_with(
            ResponseTemplate::new(401)
                .set_delay(Duration::from_millis(200))
                .set_body_json(json!({"error": {"code": "token_revoked"}})),
        )
        .mount(&server)
        .await;
    let service = quota_service_with_base_url(&store, reqwest::Client::new(), server.uri());
    let (result, revision) = tokio::join!(service.refresh_account(before.id()), async {
        tokio::time::timeout(Duration::from_secs(2), async {
            while server.received_requests().await.unwrap().is_empty() {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .expect("old query started");
        store
            .repository()
            .rotate_refreshed_oauth_secret(
                &before,
                secret("new-valid-token-before-old-rejection"),
                before.access_token_expires_at(),
                None,
            )
            .await
            .expect("rotate before old query completes")
    });
    assert!(matches!(
        result,
        Err(CodexCredentialQuotaError::Upstream { .. })
    ));
    let current = store.account(id).unwrap();
    assert_eq!(current.revision(), revision);
    assert_eq!(current.credential_state(), CredentialState::Ready);
    assert_eq!(current.last_error_reason(), None);
    assert_eq!(current.last_error_message(), None);
}

#[tokio::test]
async fn old_revoked_query_cannot_overwrite_newer_credential_observation() {
    let store = Arc::new(MemoryAccountStore::default());
    let id = "acct_newer_credential_observation";
    create_account(&store, id).await;
    let before = store.account(id).unwrap();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/codex/usage"))
        .respond_with(
            ResponseTemplate::new(401)
                .set_delay(Duration::from_millis(200))
                .set_body_json(json!({"error": {"code": "token_revoked"}})),
        )
        .mount(&server)
        .await;
    let service = quota_service_with_base_url(&store, reqwest::Client::new(), server.uri());
    let (result, ()) = tokio::join!(service.refresh_account(before.id()), async {
        tokio::time::timeout(Duration::from_secs(2), async {
            while server.received_requests().await.unwrap().is_empty() {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .expect("old query started");
        persist_credential_state(&store, &before, CredentialState::Ready).await;
    });
    assert!(matches!(
        result,
        Err(CodexCredentialQuotaError::Upstream { .. })
    ));
    assert_eq!(store.account(id).unwrap(), before);
}
