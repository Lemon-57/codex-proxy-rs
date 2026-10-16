//! 验证普通空闲连接回收不关闭在途请求及绑定下游 WS 的精确续写连接

use super::*;

async fn idle_test_server() -> (String, Arc<Mutex<Vec<bool>>>, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let closed = Arc::new(Mutex::new(Vec::new()));
    let closed_server = closed.clone();
    let task = tokio::spawn(async move {
        loop {
            let (socket, _) = listener.accept().await.unwrap();
            let index = {
                let mut c = closed_server.lock().unwrap();
                let index = c.len();
                c.push(false);
                index
            };
            let closed = closed_server.clone();
            tokio::spawn(async move {
                let mut socket = accept_codex_test_websocket(socket).await;
                while let Some(message) = socket.next().await {
                    match message {
                        Ok(Message::Text(_)) => {
                            if socket
                                .send(Message::Text(
                                    completed_websocket_response(
                                        &format!("resp_idle_{index}"),
                                        1,
                                        1,
                                    )
                                    .into(),
                                ))
                                .await
                                .is_err()
                            {
                                break;
                            }
                        }
                        Ok(Message::Close(_)) | Err(_) => break,
                        _ => {}
                    }
                }
                closed.lock().unwrap()[index] = true;
            });
        }
    });
    (base, closed, task)
}

#[tokio::test]
async fn idle_budget_evicts_ordinary_lru_and_preserves_exact_downstream_lane() {
    let (base, closed, server) = idle_test_server().await;
    let pool = Arc::new(CodexWebSocketPool::with_config(CodexWebSocketPoolConfig {
        max_idle_per_account: 1,
        idle_timeout: None,
        ..websocket_pool_config_for_tests(None, None, None)
    }));
    let backend = CodexBackendClient::new(
        reqwest::Client::builder().no_proxy().build().unwrap(),
        base,
        test_wire_profile(),
    )
    .with_websocket_pool(pool.clone());
    let mut pinned = pooled_websocket_request("pinned-idle");
    pinned.downstream_websocket_connection_id = Some("downstream-lane".to_owned());
    for request in [
        &pinned,
        &pooled_websocket_request("ordinary-old"),
        &pooled_websocket_request("ordinary-new"),
    ] {
        backend
            .create_response(request, request_context("req_idle_budget", Some("account")))
            .await
            .unwrap();
    }
    timeout(Duration::from_secs(2), async {
        while !closed.lock().unwrap()[1] {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(*closed.lock().unwrap(), vec![false, true, false]);
    pinned.set_previous_response_id(Some("resp_idle_0".to_owned()));
    pinned.previous_response_scope = Some(PreviousResponseScope::ConnectionLocal);
    let again = backend
        .create_response(
            &pinned,
            request_context("req_pinned_idle_again", Some("account")),
        )
        .await
        .unwrap();
    assert!(again.websocket_pool_decision.unwrap().is_reuse());
    pool.shutdown().await;
    server.abort();
}

#[tokio::test]
async fn idle_timeout_uses_return_time_and_does_not_expire_a_downstream_lane() {
    let (base, closed, server) = idle_test_server().await;
    let pool = Arc::new(CodexWebSocketPool::with_config(CodexWebSocketPoolConfig {
        idle_timeout: Some(Duration::from_millis(40)),
        max_idle_per_account: 4,
        ..websocket_pool_config_for_tests(None, None, None)
    }));
    let backend = CodexBackendClient::new(
        reqwest::Client::builder().no_proxy().build().unwrap(),
        base,
        test_wire_profile(),
    )
    .with_websocket_pool(pool.clone());
    let mut pinned = pooled_websocket_request("pinned-timeout");
    pinned.downstream_websocket_connection_id = Some("lane-timeout".to_owned());
    backend
        .create_response(
            &pinned,
            request_context("req_pinned_timeout", Some("account")),
        )
        .await
        .unwrap();
    backend
        .create_response(
            &pooled_websocket_request("ordinary-timeout"),
            request_context("req_ordinary_timeout", Some("account")),
        )
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(60)).await;
    pool.maintain_idle_connections().await;
    timeout(Duration::from_secs(2), async {
        while !closed.lock().unwrap()[1] {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(*closed.lock().unwrap(), vec![false, true]);
    pinned.set_previous_response_id(Some("resp_idle_0".to_owned()));
    pinned.previous_response_scope = Some(PreviousResponseScope::ConnectionLocal);
    let again = backend
        .create_response(
            &pinned,
            request_context("req_timeout_lane_again", Some("account")),
        )
        .await
        .unwrap();
    assert!(again.websocket_pool_decision.unwrap().is_reuse());
    pool.shutdown().await;
    server.abort();
}

#[tokio::test]
async fn zero_idle_budget_does_not_close_an_in_flight_response() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let (ready, accepted) = tokio::sync::oneshot::channel();
    let (release, finish) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let mut active = accept_codex_test_websocket(socket).await;
        active.next().await.unwrap().unwrap();
        ready.send(()).unwrap();
        let (socket, _) = listener.accept().await.unwrap();
        let mut idle = accept_codex_test_websocket(socket).await;
        idle.next().await.unwrap().unwrap();
        idle.send(Message::Text(
            completed_websocket_response("resp_short_idle", 1, 1).into(),
        ))
        .await
        .unwrap();
        finish.await.unwrap();
        active
            .send(Message::Text(
                completed_websocket_response("resp_long_active", 1, 1).into(),
            ))
            .await
            .unwrap();
    });
    let pool = Arc::new(CodexWebSocketPool::with_config(CodexWebSocketPoolConfig {
        max_idle_per_account: 0,
        idle_timeout: Some(Duration::from_millis(10)),
        ..websocket_pool_config_for_tests(None, None, None)
    }));
    let backend = CodexBackendClient::new(
        reqwest::Client::builder().no_proxy().build().unwrap(),
        base,
        test_wire_profile(),
    )
    .with_websocket_pool(pool.clone());
    let response = backend
        .create_response_stream(
            &pooled_websocket_request("in-flight"),
            request_context("req_active", Some("account")),
        )
        .await
        .unwrap();
    accepted.await.unwrap();
    backend
        .create_response(
            &pooled_websocket_request("short-idle"),
            request_context("req_short", Some("account")),
        )
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;
    pool.maintain_idle_connections().await;
    release.send(()).unwrap();
    let mut bytes = Vec::new();
    let mut body = response.body;
    while let Some(chunk) = body.next().await {
        bytes.extend_from_slice(&chunk.unwrap());
    }
    assert!(
        String::from_utf8(bytes)
            .unwrap()
            .contains("resp_long_active")
    );
    server.await.unwrap();
    pool.shutdown().await;
}
