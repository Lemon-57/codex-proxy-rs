//! 验证模型目录、上游拒绝反馈与未知模型的账号选号边界

use super::*;

fn operation(model: &str) -> Operation {
    let payload = ProtocolPayload::json_object(
        "openai",
        Map::from_iter([
            ("model".to_owned(), json!(model)),
            ("input".to_owned(), json!("hello")),
        ]),
    )
    .unwrap()
    .with_context(Map::from_iter([("use_websocket".to_owned(), json!(false))]));
    Operation::Generate(GenerateRequest::from_protocol_payload(payload))
}

#[tokio::test]
async fn grok_models_never_acquire_chatgpt_oauth_accounts() {
    let oauth = MockServer::start().await;
    let store = Arc::new(MemoryAccountStore::default());
    create_account(&store, "acct_provider_contract").await;
    let leases = Arc::new(TestLeaseCoordinator::default());
    let provider = provider_with_affinity_and_base_url_and_leases(
        &store,
        Arc::default(),
        oauth.uri(),
        leases.clone(),
    );
    for model in ["grok-4.7", "xai/grok-4.7", "GROK-NEW", "grok"] {
        let error = provider
            .clone()
            .execute(
                planned_request_for_model("openai", operation(model), model),
                context("req_grok_oauth_rejected", CancellationToken::new()),
            )
            .await
            .err()
            .expect("Grok cannot select a ChatGPT account");
        assert_eq!(error.kind(), ProviderErrorKind::NoEligibleAccount);
    }
    assert!(leases.requests.lock().unwrap().is_empty());
    assert!(oauth.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn mixed_pool_routes_grok_to_api_key_despite_oauth_weight() {
    let api = MockServer::start().await;
    let oauth = MockServer::start().await;
    let store = Arc::new(MemoryAccountStore::default());
    create_account(&store, "acct_scope_new").await;
    store.set_scheduling("acct_scope_new", None, AccountWeight::new(100).unwrap());
    store
        .seed_api_key(
            "acct_provider_contract",
            api.uri(),
            provider_openai::credential::ResponsesTransport::Http,
        )
        .await;
    store.set_scheduling(
        "acct_provider_contract",
        None,
        AccountWeight::new(1).unwrap(),
    );
    Mock::given(method("POST"))
        .and(path("/responses"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(CAPTURE_COMPLETED_SSE, "text/event-stream"),
        )
        .expect(1)
        .mount(&api)
        .await;
    let mut stream = provider_with_base_url(&store, oauth.uri())
        .execute(
            planned_request_for_model("openai", operation("grok-4.7"), "grok-4.7"),
            context("req_grok_api_key", CancellationToken::new()),
        )
        .await
        .unwrap();
    assert_eq!(
        stream.metadata().provider_account_id().as_str(),
        "acct_provider_contract"
    );
    while let Some(event) = stream.next().await {
        event.unwrap();
    }
    let requests = api.received_requests().await.unwrap();
    assert_eq!(captured_request_body(&requests[0])["model"], "grok-4.7");
    assert!(oauth.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn unlisted_models_remain_available_without_catalog_discovery() {
    for api_key in [false, true] {
        let server = MockServer::start().await;
        let store = Arc::new(MemoryAccountStore::default());
        if api_key {
            store
                .seed_api_key(
                    "acct_provider_contract",
                    server.uri(),
                    provider_openai::credential::ResponsesTransport::Http,
                )
                .await;
        } else {
            create_account(&store, "acct_provider_contract").await;
        }
        Mock::given(method("POST"))
            .and(path(if api_key {
                "/responses"
            } else {
                "/codex/responses"
            }))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(CAPTURE_COMPLETED_SSE, "text/event-stream"),
            )
            .expect(1)
            .mount(&server)
            .await;
        let mut stream = provider_with_base_url(&store, server.uri())
            .execute(
                planned_request_for_model(
                    "openai",
                    operation("custom-unlisted"),
                    "custom-unlisted",
                ),
                context("req_unlisted", CancellationToken::new()),
            )
            .await
            .unwrap();
        while let Some(event) = stream.next().await {
            event.unwrap();
        }
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            captured_request_body(&requests[0])["model"],
            "custom-unlisted"
        );
    }
}

#[tokio::test]
async fn known_oauth_models_filter_plan_candidates_before_weight_and_keep_unknown_models() {
    let server = MockServer::start().await;
    let store = Arc::new(MemoryAccountStore::default());
    create_account(&store, "acct_provider_contract").await;
    let mut free = profile("chatgpt-acct_scope_new");
    free.plan_type = Some("free".to_owned());
    store
        .seed_oauth_credential(ImportCodexOAuthCredential {
            account_id: "acct_scope_new".to_owned(),
            name: "free".to_owned(),
            secret: secret("test-free"),
            verified_account: free,
            next_refresh_at: Some(Utc::now() + chrono::Duration::minutes(30)),
            enabled: true,
        })
        .await;
    store.set_scheduling("acct_scope_new", None, AccountWeight::new(100).unwrap());
    for (id, models) in [
        (
            "acct_scope_new",
            json!([{"slug":"gpt-6-luna","display_name":"Luna"}]),
        ),
        (
            "acct_provider_contract",
            json!([{"slug":"gpt-6-luna","display_name":"Luna"},{"slug":"gpt-6-astra","display_name":"Astra"}]),
        ),
    ] {
        Mock::given(method("GET"))
            .and(path("/codex/models"))
            .and(header("chatgpt-account-id", format!("chatgpt-{id}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"models":models})))
            .expect(1)
            .mount(&server)
            .await;
    }
    Mock::given(method("POST"))
        .and(path("/codex/responses"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(CAPTURE_COMPLETED_SSE, "text/event-stream"),
        )
        .expect(3)
        .mount(&server)
        .await;
    let affinity = Arc::new(MemorySessionAffinity::default());
    let provider = provider_with_affinity_and_base_url(&store, affinity.clone(), server.uri());
    provider.query_model_capabilities().await.unwrap();
    for (model, expected) in [
        ("gpt-6-luna", "acct_scope_new"),
        ("gpt-6-astra", "acct_provider_contract"),
        ("custom-unlisted", "acct_scope_new"),
    ] {
        let request_operation = if model == "custom-unlisted" {
            operation(model)
        } else {
            Operation::Generate(GenerateRequest::from_protocol_payload(
                ProtocolPayload::json_object(
                    "openai",
                    json!({
                        "model": model, "input": "hello",
                        "client_metadata": {"x-codex-turn-metadata": json!({
                            "session_id":"model-session", "thread_id":"model-session"
                        }).to_string()}
                    })
                    .as_object()
                    .unwrap()
                    .clone(),
                )
                .unwrap()
                .with_context(Map::from_iter([("use_websocket".to_owned(), json!(false))])),
            ))
        };
        let mut stream = provider
            .clone()
            .execute(
                planned_request_for_model("openai", request_operation, model),
                context("req_plan_model", CancellationToken::new()),
            )
            .await
            .unwrap();
        assert_eq!(
            stream.metadata().provider_account_id().as_str(),
            expected,
            "{model}"
        );
        while let Some(event) = stream.next().await {
            event.unwrap();
        }
    }
    assert_eq!(affinity.binding_count(), 1);
    let pinned = AttemptContext::new(
        RequestAttemptContext::new(
            ModelRequestId::new("req_catalog_pin").unwrap(),
            ClientApiKeyId::new("key_openai_contract").unwrap(),
        ),
        NonZeroU32::MIN,
        SystemTime::now() + Duration::from_secs(30),
        account_policy(),
        AccountAttemptContext::new(
            BTreeSet::new(),
            Some(ProviderAccountId::new("acct_scope_new").unwrap()),
            None,
        )
        .with_account_scope(contract_account_scope()),
        None,
        CancellationToken::new(),
    );
    let error = provider
        .execute(
            planned_request_for_model("openai", operation("gpt-6-astra"), "gpt-6-astra"),
            pinned,
        )
        .await
        .err()
        .expect("a fixed account cannot escape model eligibility");
    assert_eq!(error.kind(), ProviderErrorKind::NoEligibleAccount);
}

#[tokio::test]
async fn upstream_model_rejection_cools_only_that_model_across_requests_and_transports() {
    use gateway_core::provider_ports::{ProviderCooldownScope, ProviderScopedCooldown};

    for websocket in [false, true] {
        let server = MockServer::start().await;
        let store = Arc::new(MemoryAccountStore::default());
        create_account(&store, "acct_provider_contract").await;
        let cooldowns = Arc::new(MemoryCooldownPort::new());
        let (provider, pool) = provider_with_cooldowns(&store, server.uri(), cooldowns.clone());
        Mock::given(method(if websocket { "GET" } else { "POST" }))
            .and(path("/codex/responses"))
            .respond_with(ResponseTemplate::new(400).set_body_json(json!({"error":{
                "code":"model_not_found", "type":"invalid_request_error",
                "message":"The 'gpt-6-astra' model is not supported when using Codex with a ChatGPT account."
            }}))).mount(&server).await;
        let request_operation = if websocket {
            let payload = ProtocolPayload::json_object(
                "openai",
                Map::from_iter([
                    ("model".to_owned(), json!("gpt-6-astra")),
                    ("input".to_owned(), json!("hello")),
                ]),
            )
            .unwrap();
            Operation::Generate(GenerateRequest::from_protocol_payload(payload))
        } else {
            operation("gpt-6-astra")
        };
        let mut stream = provider
            .clone()
            .execute(
                planned_request_for_model("openai", request_operation, "gpt-6-astra"),
                context("req_model_rejected", CancellationToken::new()),
            )
            .await
            .unwrap();
        let error = loop {
            match stream.next().await {
                Some(Err(error)) => break error,
                Some(Ok(_)) => {}
                None => panic!("expected model rejection"),
            }
        };
        assert_eq!(error.kind(), ProviderErrorKind::Unsupported);
        assert!(error.replay_is_safe());
        drop(stream);
        let blocked = provider
            .clone()
            .execute(
                planned_request_for_model("openai", operation("gpt-6-astra"), "gpt-6-astra"),
                context("req_model_cooled", CancellationToken::new()),
            )
            .await
            .err()
            .expect("model cooldown must survive the first request");
        assert_eq!(blocked.kind(), ProviderErrorKind::NoEligibleAccount);
        assert!(cooldowns.cooldowns.lock().unwrap().is_empty());
        server.reset().await;
        Mock::given(method("POST"))
            .and(path("/codex/responses"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(CAPTURE_COMPLETED_SSE, "text/event-stream"),
            )
            .expect(2)
            .mount(&server)
            .await;
        let mut other = provider
            .clone()
            .execute(
                planned_request_for_model("openai", operation("gpt-6-luna"), "gpt-6-luna"),
                context("req_other_model", CancellationToken::new()),
            )
            .await
            .unwrap();
        while let Some(event) = other.next().await {
            event.unwrap();
        }
        let account = store.account("acct_provider_contract").unwrap();
        let scope =
            ProviderCooldownScope::upstream_model(UpstreamModelId::new("gpt-6-astra").unwrap());
        let key = (account.id().clone(), scope.clone());
        {
            let mut scoped = cooldowns.scoped.lock().unwrap();
            let until = scoped[&key].until();
            assert!(
                (Duration::from_secs(29 * 60)..=Duration::from_secs(30 * 60))
                    .contains(&until.duration_since(SystemTime::now()).unwrap())
            );
            scoped.insert(
                key,
                ProviderScopedCooldown::new(
                    account.id().clone(),
                    account.revision(),
                    scope,
                    SystemTime::now() - Duration::from_secs(1),
                ),
            );
        }
        let mut recovered = provider
            .clone()
            .execute(
                planned_request_for_model("openai", operation("gpt-6-astra"), "gpt-6-astra"),
                context("req_model_recovered", CancellationToken::new()),
            )
            .await
            .unwrap();
        while let Some(event) = recovered.next().await {
            event.unwrap();
        }
        pool.shutdown().await;
    }
}
