//! 额度接口失败到规范化账号事实的唯一分类边界

use gateway_core::account::{AccountErrorReason, CredentialState, QuotaEvidence};
use reqwest::StatusCode;
use serde_json::Value;

use crate::transport::CodexClientError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum QuotaEndpointFailure {
    Credential {
        state: CredentialState,
        reason: AccountErrorReason,
        message: Option<&'static str>,
    },
    Exhausted(QuotaEvidence),
}

/// 明确的令牌撤销只标记当前凭据无效，不能推断 RT 永久失效或账号封禁
/// 普通 401/403、429、5xx 与传输失败仍不改变账号事实
pub(crate) fn classify_quota_endpoint_failure(
    error: &CodexClientError,
) -> Option<QuotaEndpointFailure> {
    let CodexClientError::Upstream { status, body, .. } = error else {
        return None;
    };
    if *status == StatusCode::UNAUTHORIZED
        && upstream_error_code(error).as_deref() == Some("token_revoked")
    {
        return Some(QuotaEndpointFailure::Credential {
            state: CredentialState::Invalid,
            reason: AccountErrorReason::CredentialInvalid,
            message: Some("访问令牌已被撤销，请刷新凭据或重新授权"),
        });
    }
    if *status != StatusCode::PAYMENT_REQUIRED {
        return None;
    }
    if is_deactivated_workspace(body) {
        return Some(QuotaEndpointFailure::Credential {
            state: CredentialState::Banned,
            reason: AccountErrorReason::AccountBanned,
            message: None,
        });
    }
    Some(QuotaEndpointFailure::Exhausted(
        QuotaEvidence::PaymentRequired,
    ))
}

fn is_deactivated_workspace(body: &str) -> bool {
    serde_json::from_str::<Value>(body)
        .ok()
        .is_some_and(|value| {
            value.pointer("/detail/code").and_then(Value::as_str) == Some("deactivated_workspace")
        })
}

/// 从上游错误体提取稳定错误码：优先 `/error/code`，其次 `/code`
///
/// 只接受有界的 ASCII 标识，避免把自由文本作为错误码写入诊断
pub(super) fn upstream_error_code(error: &CodexClientError) -> Option<String> {
    let CodexClientError::Upstream { body, .. } = error else {
        return None;
    };
    let value = serde_json::from_str::<Value>(body).ok()?;
    let code = value
        .pointer("/error/code")
        .or_else(|| value.pointer("/code"))
        .and_then(Value::as_str)?
        .trim();
    if code.is_empty()
        || code.len() > 64
        || !code
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        return None;
    }
    Some(code.to_owned())
}
