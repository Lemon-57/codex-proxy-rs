//! 下游客户端对 Codex Core/Desktop 请求协议的兼容处理
//! 账号身份保护、会话规范化和 HTTP 传输规则由对应职责模块维护

mod body;
mod grok;
mod headers;
mod search_history;

use serde_json::{Map, Value};

pub(super) use body::normalize_codex_request_body;

pub(crate) fn normalize_selected_codex_downstream_body(
    body: &mut Map<String, Value>,
    context: &Map<String, Value>,
    responses_lite: bool,
) {
    body::normalize_non_codex_request_body(body);
    grok::normalize_request_body(body, context);
    search_history::declare_search_history_tool(body, responses_lite);
}

pub(super) fn is_non_codex_request_header(name: &str) -> bool {
    headers::is_non_codex_request_header(name) || grok::is_client_header(name)
}
