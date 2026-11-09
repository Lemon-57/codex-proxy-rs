//! 自动重置卡的持久计划与查询投影，不保存上游凭据或完整卡片

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedResetCredit {
    pub id: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingResetCredit {
    pub credit: PlannedResetCredit,
    pub redeem_request_id: String,
}

/// 上游消费前的周窗口事实，按万分比保存以支持确定性的状态比较
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResetCreditQuotaRefresh {
    pub previous_weekly_reset_at: Option<DateTime<Utc>>,
    pub previous_weekly_used: Option<u32>,
    pub next_attempt_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResetCreditAutomationState {
    /// 未单独设置开关的账号动态继承全局默认值
    #[serde(default)]
    pub inherits_default: bool,
    #[serde(default)]
    pub next_quota_check_at: Option<DateTime<Utc>>,
    /// 消费已确认后的独立额度核验任务，不得再次消费卡片
    #[serde(default)]
    pub quota_refresh: Option<ResetCreditQuotaRefresh>,
    pub next_discovery_at: DateTime<Utc>,
    pub credits: Vec<PlannedResetCredit>,
    pub pending: Option<PendingResetCredit>,
    pub retry_at: Option<DateTime<Utc>>,
    pub failures: u32,
    pub last_result: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResetCreditAutomation {
    pub account_id: String,
    pub enabled: bool,
    /// 零表示尚未创建；每次持久状态写入都递增版本
    pub revision: u64,
    pub next_run_at: DateTime<Utc>,
    pub state: ResetCreditAutomationState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResetCreditAutomationStatus {
    pub enabled: bool,
    pub lead_minutes: u32,
    pub next_discovery_at: Option<DateTime<Utc>>,
    pub next_consume_at: Option<DateTime<Utc>>,
    pub last_result: Option<String>,
}
