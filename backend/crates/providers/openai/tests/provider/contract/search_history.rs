//! 验证 OAuth 搜索历史补声明的真实 HTTP 出站、Lite 压缩位置与无工具语义

use super::*;

#[tokio::test]
async fn search_history_declares_cached_tool_on_the_selected_oauth_http_wire() {
    for (lite, caller_tools) in [(false, false), (true, false), (false, true), (true, true)] {
        let store = Arc::new(MemoryAccountStore::default());
        create_account(&store, "acct_provider_contract").await;
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/codex/responses"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(CAPTURE_COMPLETED_SSE),
            )
            .expect(1)
            .mount(&server)
            .await;
        let history = json!({"type":"web_search_call", "id":"ws_history", "status":"completed", "action":{"type":"search", "query":"synthetic query", "sources":[{"url":"https://example.com"}]}});
        let mut body = json!({"model":"gpt-5.4", "input":[history.clone(), {"type":"compaction_trigger"}], "stream":true, "store":false, "tool_choice":"auto"});
        if caller_tools {
            body["tools"] =
                json!([{"type":"function", "name":"echo", "parameters":{"type":"object"}}]);
        }
        let mut wire_context = Map::from_iter([("use_websocket".to_owned(), json!(false))]);
        if lite {
            wire_context.insert("responses_lite".to_owned(), json!("true"));
        }
        let operation = Operation::Generate(GenerateRequest::from_protocol_payload(
            ProtocolPayload::json_object("openai", body.as_object().unwrap().clone())
                .unwrap()
                .with_context(wire_context),
        ));
        let mut stream = provider_with_base_url(&store, server.uri())
            .execute(
                planned_request("openai", operation),
                context("req_search_history", CancellationToken::new()),
            )
            .await
            .unwrap();
        let mut completed = false;
        while let Some(event) = stream.next().await {
            let event = event.unwrap();
            completed |= event
                .canonical_facts()
                .iter()
                .any(|event| matches!(event, GatewayEvent::Completed(_)))
                || event
                    .wire_event()
                    .is_some_and(|wire| wire.event_type() == Some("response.completed"));
        }
        assert!(completed);
        let requests = server.received_requests().await.unwrap();
        let decoded = zstd::stream::decode_all(requests[0].body.as_slice()).unwrap();
        let sent: Value = serde_json::from_slice(&decoded).unwrap();
        let expected = json!({"type":"web_search", "external_web_access":false});
        if lite {
            assert_eq!(sent["input"][1]["type"], "additional_tools");
            assert_eq!(sent["input"][1]["tools"][0], expected);
            assert_eq!(sent["input"][2]["type"], "compaction_trigger");
            assert_eq!(sent.get("tools"), body.get("tools"));
        } else {
            let added = sent["tools"].as_array().unwrap().last().unwrap();
            assert_eq!(added["type"], expected["type"]);
            assert_eq!(
                added["external_web_access"],
                expected["external_web_access"]
            );
            assert_eq!(sent["input"][1]["type"], "compaction_trigger");
        }
        assert_eq!(sent["input"][0], history);
        assert_eq!(
            sent["tool_choice"],
            if caller_tools {
                json!("auto")
            } else {
                json!("none")
            }
        );
    }
}

#[tokio::test]
async fn search_history_existing_search_declarations_and_api_key_requests_stay_unchanged() {
    for api_key in [false, true] {
        let store = Arc::new(MemoryAccountStore::default());
        let server = MockServer::start().await;
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
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(CAPTURE_COMPLETED_SSE),
            )
            .expect(1)
            .mount(&server)
            .await;
        let history = json!({"type":"web_search_call","id":"ws_preserved","status":"completed","action":{"type":"search","query":"synthetic"}});
        let mut body =
            json!({"model":"gpt-5.4","input":[history],"tool_choice":"auto","store":false});
        if !api_key {
            body["input"].as_array_mut().unwrap().push(json!({"type":"additional_tools","tools":[{"type":"web_search_preview","search_context_size":"low"}]}));
        }
        let operation = Operation::Generate(GenerateRequest::from_protocol_payload(
            ProtocolPayload::json_object("openai", body.as_object().unwrap().clone())
                .unwrap()
                .with_context(Map::from_iter([("use_websocket".into(), json!(false))])),
        ));
        let mut stream = provider_with_base_url(&store, server.uri())
            .execute(
                planned_request("openai", operation),
                context("req_search_preserve", CancellationToken::new()),
            )
            .await
            .unwrap();
        while let Some(event) = stream.next().await {
            event.unwrap();
        }
        let sent = captured_request_body(&server.received_requests().await.unwrap()[0]);
        assert!(sent.get("tools").is_none());
        assert_eq!(sent["tool_choice"], body["tool_choice"]);
        assert_eq!(sent["input"], body["input"]);
    }
}

#[tokio::test]
async fn search_history_declaration_reaches_the_oauth_websocket_wire() {
    for lite in [false, true] {
        let store = Arc::new(MemoryAccountStore::default());
        create_account(&store, "acct_provider_contract").await;
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            let mut socket = accept_codex_test_websocket(socket).await;
            let sent: Value =
                serde_json::from_str(socket.next().await.unwrap().unwrap().to_text().unwrap())
                    .unwrap();
            let completed = CAPTURE_COMPLETED_SSE
                .lines()
                .find_map(|line| line.strip_prefix("data: "))
                .unwrap();
            socket
                .send(Message::Text(completed.to_owned().into()))
                .await
                .unwrap();
            sent
        });
        let body = json!({"model":"gpt-5.4","input":[{"type":"web_search_call","id":"ws_wire","status":"completed","action":{"type":"search","query":"synthetic"}}],"store":false});
        let mut c = Map::from_iter([("use_websocket".to_owned(), json!(true))]);
        if lite {
            c.insert("responses_lite".into(), json!("true"));
        }
        let operation = Operation::Generate(GenerateRequest::from_protocol_payload(
            ProtocolPayload::json_object("openai", body.as_object().unwrap().clone())
                .unwrap()
                .with_context(c),
        ));
        let mut stream = provider_with_base_url(&store, base)
            .execute(
                planned_request("openai", operation),
                context("req_search_ws", CancellationToken::new()),
            )
            .await
            .unwrap();
        while let Some(event) = stream.next().await {
            event.unwrap();
        }
        let sent = server.await.unwrap();
        let added = if lite {
            &sent["input"][1]["tools"][0]
        } else {
            &sent["tools"][0]
        };
        assert_eq!(added["type"], "web_search");
        assert_eq!(added["external_web_access"], false);
        assert_eq!(sent["tool_choice"], "none");
        assert_eq!(sent["input"][0], body["input"][0]);
    }
}

#[tokio::test]
async fn search_history_preserves_explicit_choices_and_invalid_tool_contracts() {
    let history = json!({"type":"web_search_call","id":"ws_edges","status":"completed","action":{"type":"search","query":"synthetic"}});
    for (lite, input, tools, choice, injected) in [
        (false, json!([]), json!([]), json!("auto"), false),
        (
            false,
            json!([history.clone()]),
            json!([{"type":"web_search_preview"}]),
            json!("required"),
            false,
        ),
        (
            false,
            json!([history.clone()]),
            json!([]),
            json!("required"),
            true,
        ),
        (
            false,
            json!([history.clone()]),
            json!([]),
            json!({"type":"function","name":"echo"}),
            true,
        ),
        (
            false,
            json!([history.clone()]),
            json!("invalid"),
            json!("auto"),
            false,
        ),
        (
            true,
            json!([history.clone(),{"type":"additional_tools","tools":"invalid"}]),
            json!([]),
            json!("auto"),
            false,
        ),
        (
            true,
            json!([history,{"type":"additional_tools","tools":[{"type":"function","name":"echo","parameters":{"type":"object"}}]},{"type":"compaction_trigger"}]),
            json!([]),
            json!("auto"),
            true,
        ),
    ] {
        let store = Arc::new(MemoryAccountStore::default());
        create_account(&store, "acct_provider_contract").await;
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/codex/responses"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(CAPTURE_COMPLETED_SSE),
            )
            .expect(1)
            .mount(&server)
            .await;
        let body = json!({"model":"gpt-5.4","input":input,"tools":tools,"tool_choice":choice,"store":false});
        let mut wire_context = Map::from_iter([("use_websocket".into(), json!(false))]);
        if lite {
            wire_context.insert("responses_lite".into(), json!("true"));
        }
        let operation = Operation::Generate(GenerateRequest::from_protocol_payload(
            ProtocolPayload::json_object("openai", body.as_object().unwrap().clone())
                .unwrap()
                .with_context(wire_context),
        ));
        let mut stream = provider_with_base_url(&store, server.uri())
            .execute(
                planned_request("openai", operation),
                context("req_search_edges", CancellationToken::new()),
            )
            .await
            .unwrap();
        while let Some(event) = stream.next().await {
            event.unwrap();
        }
        let sent = captured_request_body(&server.received_requests().await.unwrap()[0]);
        assert_eq!(sent["tool_choice"], body["tool_choice"]);
        if !injected {
            assert_eq!(sent["input"], body["input"]);
            let mut tools = sent["tools"].clone();
            if let Some(tools) = tools.as_array_mut() {
                for tool in tools {
                    // 账号已有的搜索地域补齐由独立规范化规则负责
                    if let Some(tool) = tool.as_object_mut() {
                        tool.remove("user_location");
                    }
                }
            }
            assert_eq!(tools, body["tools"]);
        } else if lite {
            assert_eq!(sent["input"][1]["tools"][0], body["input"][1]["tools"][0]);
            assert_eq!(sent["input"][1]["tools"][1]["type"], "web_search");
            assert_eq!(sent["input"][2], body["input"][2]);
        } else {
            assert_eq!(sent["input"], body["input"]);
            assert_eq!(sent["tools"][0]["type"], "web_search");
            assert_eq!(sent["tools"][0]["external_web_access"], false);
        }
    }
}
