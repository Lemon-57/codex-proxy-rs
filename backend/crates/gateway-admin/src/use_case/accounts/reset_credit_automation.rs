//! 重置卡查询、消费与自动使用排程，统一账号锁、持久幂等键和重试编排

use super::*;

use crate::model::{MutationActor, reset_credit_automation::*};
use chrono::{DateTime, Utc};
use gateway_core::time::DeploymentTimeZone;
use rand_core::{OsRng, RngCore as _};
use sha2::{Digest as _, Sha256};

impl DefaultAccountsService {
    pub(super) async fn reset_credits_upstream(
        &self,
        context: &MutationContext,
        account_id: ProviderAccountId,
    ) -> Result<ProviderResetCredits, AdminError> {
        let (_, provider) = self.provider_for_account(&account_id).await?;
        match provider.reset_credits(&account_id).await {
            Ok(credits) => Ok(credits),
            Err(error)
                if error.kind()
                    == crate::ports::provider::ProviderAdminErrorKind::CredentialRefreshRequired =>
            {
                self.refresh(context, account_id.clone()).await?;
                let (_, provider) = self.provider_for_account(&account_id).await?;
                provider
                    .reset_credits(&account_id)
                    .await
                    .map_err(map_reset_credits_error_after_refresh)
            }
            Err(error) => Err(map_provider_error(error, "provider reset credits")),
        }
    }

    pub(super) async fn consume_reset_credit_upstream(
        &self,
        context: &MutationContext,
        command: ConsumeProviderResetCredit,
    ) -> Result<ProviderResetCreditResult, AdminError> {
        let account_id = command.account_id.clone();
        let (_, provider) = self.provider_for_account(&account_id).await?;
        match provider.consume_reset_credit(command.clone()).await {
            Ok(result) => Ok(result),
            Err(error)
                if error.kind()
                    == crate::ports::provider::ProviderAdminErrorKind::CredentialRefreshRequired =>
            {
                self.refresh(context, account_id.clone()).await?;
                let (_, provider) = self.provider_for_account(&account_id).await?;
                provider
                    .consume_reset_credit(command)
                    .await
                    .map_err(map_reset_credits_error_after_refresh)
            }
            Err(error) => Err(map_provider_error(error, "provider reset-credit consume")),
        }
    }
}

impl DefaultAccountsService {
    async fn automation_settings(
        &self,
    ) -> Result<crate::model::settings::RuntimeSettingsValues, AdminError> {
        Ok(self
            .settings
            .load_runtime_settings()
            .await
            .map_err(|e| map_store_error(e, "runtime settings"))?
            .values)
    }

    pub(super) async fn automation_status(
        &self,
        account_id: &ProviderAccountId,
    ) -> Result<ResetCreditAutomationStatus, AdminError> {
        self.require_reset_credit_automation(account_id).await?;
        let settings = self.automation_settings().await?;
        let lead_minutes = settings.reset_credit_lead_minutes;
        let record = self
            .accounts
            .reset_credit_automation(account_id.as_str())
            .await
            .map_err(|e| map_store_error(e, "reset-credit automation"))?;
        let record = record.map(|mut r| {
            if r.state.inherits_default {
                r.enabled = settings.reset_credit_default_enabled;
            }
            r
        });
        Ok(ResetCreditAutomationStatus {
            enabled: record
                .as_ref()
                .map_or(settings.reset_credit_default_enabled, |r| r.enabled),
            lead_minutes,
            next_discovery_at: record
                .as_ref()
                .filter(|r| r.enabled)
                .map(|r| r.state.next_discovery_at),
            next_consume_at: record.as_ref().filter(|r| r.enabled).and_then(|r| {
                next_consumption(r, lead_minutes)
                    .or_else(|| {
                        r.state
                            .pending
                            .as_ref()
                            .map(|pending| consume_at(&r.account_id, &pending.credit, lead_minutes))
                    })
                    .map(|due| r.state.retry_at.map_or(due, |retry| due.max(retry)))
            }),
            last_result: record.and_then(|r| {
                if r.state.quota_refresh.is_some() {
                    return Some("quota_refresh_pending".to_owned());
                }
                if r.state
                    .pending
                    .as_ref()
                    .is_some_and(|pending| pending.credit.expires_at <= Utc::now())
                {
                    Some("expired_unknown".to_owned())
                } else {
                    r.state.last_result
                }
            }),
        })
    }

    async fn require_reset_credit_automation(
        &self,
        account_id: &ProviderAccountId,
    ) -> Result<bool, AdminError> {
        let (stored, provider) = self.provider_for_account(account_id).await?;
        let capabilities =
            provider.account_capabilities(account_id, &stored.account.authentication_kind);
        if !capabilities.reset_credits || !capabilities.consume_reset_credit {
            return Err(AdminError::invalid("当前账号不支持自动重置卡"));
        }
        Ok(stored.account.enabled)
    }

    pub(super) async fn configure_automation(
        &self,
        context: &MutationContext,
        account_id: &ProviderAccountId,
        enabled: bool,
    ) -> Result<ResetCreditAutomationStatus, AdminError> {
        let lock = self.reset_credit_lock(account_id).await;
        let _guard = lock.lock().await;
        self.require_reset_credit_automation(account_id).await?;
        let now = Utc::now();
        let mut record = self
            .accounts
            .reset_credit_automation(account_id.as_str())
            .await
            .map_err(|e| map_store_error(e, "reset-credit automation"))?
            .unwrap_or_else(|| new_automation(account_id, now));
        if record.enabled != enabled || record.state.inherits_default {
            record.enabled = enabled;
            record.state.inherits_default = false;
            // 首次开启错开一分钟；重新开启保留已知到期时间及未决幂等键
            if enabled && record.revision == 0 {
                record.state.next_discovery_at =
                    now + chrono::Duration::seconds(i64::from(OsRng.next_u32() % 60));
            }
            let lead = self.automation_settings().await?.reset_credit_lead_minutes;
            reschedule(&mut record, lead, now);
            self.save_automation(&mut record, Some(context)).await?;
            let settings = self
                .settings
                .load_runtime_settings()
                .await
                .map_err(|e| map_store_error(e, "runtime settings"))?;
            publish_committed(self.snapshot.as_ref(), settings.config_revision).await?;
        }
        self.automation_status(account_id).await
    }

    async fn save_automation(
        &self,
        record: &mut ResetCreditAutomation,
        context: Option<&MutationContext>,
    ) -> Result<(), AdminError> {
        record.revision = self
            .accounts
            .save_reset_credit_automation(record, context)
            .await
            .map_err(|e| map_store_error(e, "reset-credit automation"))?;
        Ok(())
    }

    fn observe_reset_credits(
        &self,
        record: &mut ResetCreditAutomation,
        credits: &ProviderResetCredits,
        now: DateTime<Utc>,
    ) -> Result<(), AdminError> {
        record.state.credits = credits
            .credits
            .iter()
            .filter(|credit| credit.status.as_deref() == Some("available"))
            .filter_map(|credit| {
                credit
                    .expires_at
                    .filter(|expiry| *expiry > now)
                    .map(|expires_at| PlannedResetCredit {
                        id: credit.id.clone(),
                        expires_at,
                    })
            })
            .collect();
        record.state.credits.sort_by_key(|credit| credit.expires_at);
        record.state.credits.dedup_by(|a, b| a.id == b.id);
        if record.state.next_discovery_at <= now
            || self
                .timezone
                .local(record.state.next_discovery_at)
                .date_naive()
                == self.timezone.local(now).date_naive()
        {
            record.state.next_discovery_at =
                next_discovery(now, self.timezone, OsRng.next_u64())
                    .ok_or_else(|| AdminError::internal("无法计算重置卡发现时间"))?;
        }
        if record.state.pending.is_none() {
            record.state.retry_at = None;
            record.state.failures = 0;
        }
        Ok(())
    }

    pub(super) async fn remember_reset_credits(
        &self,
        account_id: &ProviderAccountId,
        credits: &ProviderResetCredits,
    ) -> Result<(), AdminError> {
        let Some(mut record) = self
            .accounts
            .reset_credit_automation(account_id.as_str())
            .await
            .map_err(|e| map_store_error(e, "reset-credit automation"))?
        else {
            return Ok(());
        };
        if !record.enabled {
            return Ok(());
        }
        let now = Utc::now();
        self.observe_reset_credits(&mut record, credits, now)?;
        reschedule(
            &mut record,
            self.automation_settings().await?.reset_credit_lead_minutes,
            now,
        );
        self.save_automation(&mut record, None).await
    }

    async fn automation_failed(
        &self,
        record: &mut ResetCreditAutomation,
        lead: u32,
        now: DateTime<Utc>,
    ) -> Result<(), AdminError> {
        if record.state.pending.is_none()
            && !next_consumption(record, lead).is_some_and(|due| due <= now)
        {
            record.state.next_discovery_at =
                next_discovery(now, self.timezone, OsRng.next_u64())
                    .ok_or_else(|| AdminError::internal("无法计算重置卡发现时间"))?;
            record.state.retry_at = None;
            record.state.last_result = Some("discovery_failed".to_owned());
            reschedule(record, lead, now);
            return self.save_automation(record, None).await;
        }
        record.state.failures = record.state.failures.saturating_add(1);
        let delay = (30_i64 * 2_i64.pow(record.state.failures.min(4) - 1)).min(300)
            + i64::from(OsRng.next_u32() % 30);
        record.state.retry_at = Some(now + chrono::Duration::seconds(delay));
        record.state.last_result = Some(
            if record.state.pending.is_some() {
                "pending"
            } else {
                "discovery_failed"
            }
            .to_owned(),
        );
        reschedule(record, lead, now);
        self.save_automation(record, None).await
    }

    /// 暂不满足条件不丢弃卡片，也不创建未决消费，允许到期前再次判断
    async fn defer_automation(
        &self,
        record: &mut ResetCreditAutomation,
        lead: u32,
        reason: &str,
    ) -> Result<(), AdminError> {
        let now = Utc::now();
        let retry = now + chrono::Duration::seconds(30 + i64::from(OsRng.next_u32() % 30));
        record.state.retry_at = Some(
            record
                .state
                .credits
                .iter()
                .map(|credit| credit.expires_at)
                .min()
                .map_or(retry, |expiry| retry.min(expiry)),
        );
        record.state.last_result = Some(reason.to_owned());
        record.state.failures = 0;
        reschedule(record, lead, now);
        self.save_automation(record, None).await
    }

    pub(super) async fn remember_consumed_quota_refresh(
        &self,
        account_id: &ProviderAccountId,
        baseline: ResetCreditQuotaRefresh,
        code: &str,
    ) -> Result<(), AdminError> {
        let mut record = self
            .accounts
            .reset_credit_automation(account_id.as_str())
            .await
            .map_err(|error| map_store_error(error, "reset-credit quota refresh"))?
            .unwrap_or_else(|| new_automation(account_id, Utc::now()));
        record.state.quota_refresh = Some(baseline);
        record.state.last_result = Some(code.to_owned());
        reschedule(
            &mut record,
            self.automation_settings().await?.reset_credit_lead_minutes,
            Utc::now(),
        );
        self.save_automation(&mut record, None).await?;
        self.refresh_consumed_quota(account_id, &mut record).await
    }

    /// 额度刷新是消费确认后的单独阶段，失败或上游延迟只重查额度
    pub(super) async fn refresh_consumed_quota(
        &self,
        account_id: &ProviderAccountId,
        record: &mut ResetCreditAutomation,
    ) -> Result<(), AdminError> {
        let Some(refresh) = record.state.quota_refresh.clone() else {
            return Ok(());
        };
        let confirmed = match self.quota(account_id, true).await {
            Ok(item) if !item.account.quota.is_exhausted() => item
                .quota
                .usage_windows()
                .find(|(_, period)| *period == AccountUsagePeriod::Weekly)
                .is_some_and(|(window, _)| {
                    let used = window
                        .used_percent
                        .filter(|n| n.is_finite())
                        .map(|n| (n.clamp(0.0, 100.0) * 100.0).round() as u32);
                    match (refresh.previous_weekly_used, used) {
                        (Some(before), Some(after)) if after < before => true,
                        (_, Some(after))
                            if after < 10_000 && refresh.previous_weekly_used.is_none() =>
                        {
                            true
                        }
                        _ => window
                            .reset_at
                            .zip(refresh.previous_weekly_reset_at)
                            .is_some_and(|(after, before)| after > before),
                    }
                }),
            Ok(_) | Err(_) => false,
        };
        if confirmed {
            record.state.quota_refresh = None;
        } else if let Some(refresh) = &mut record.state.quota_refresh {
            refresh.next_attempt_at = Utc::now() + chrono::Duration::seconds(30);
        }
        reschedule(
            record,
            self.automation_settings().await?.reset_credit_lead_minutes,
            Utc::now(),
        );
        self.save_automation(record, None).await
    }

    pub(super) async fn quota_refresh_baseline(
        &self,
        account_id: &ProviderAccountId,
    ) -> ResetCreditQuotaRefresh {
        let quota = self.quota(account_id, false).await.ok();
        let weekly = quota.as_ref().and_then(|item| {
            item.quota
                .usage_windows()
                .find(|(_, period)| *period == AccountUsagePeriod::Weekly)
                .map(|(window, _)| window)
        });
        ResetCreditQuotaRefresh {
            previous_weekly_reset_at: weekly.and_then(|w| w.reset_at),
            previous_weekly_used: weekly
                .and_then(|w| w.used_percent)
                .filter(|n| n.is_finite())
                .map(|n| (n.clamp(0.0, 100.0) * 100.0).round() as u32),
            next_attempt_at: Utc::now(),
        }
    }

    async fn weekly_reset_credit_gate(
        &self,
        account_id: &ProviderAccountId,
        max_remaining: u32,
    ) -> Result<Option<&'static str>, AdminError> {
        let (_, provider) = self.provider_for_account(account_id).await?;
        let quota = provider
            .quota(ProviderQuotaRequest {
                account_id: account_id.clone(),
                refresh: true,
                rolling_usage: None,
            })
            .await
            .map_err(|error| map_provider_error(error, "reset-credit weekly quota"))?;
        // 复用账号周窗口归属，不把短期、月度或模型专属桶当作剩余周额度
        let remaining = quota
            .usage_windows()
            .find(|(_, period)| *period == AccountUsagePeriod::Weekly)
            .and_then(|(window, _)| window.used_percent)
            .filter(|used| used.is_finite())
            .map(|used| 100.0 - used.clamp(0.0, 100.0));
        Ok(match remaining {
            Some(remaining) if remaining > f64::from(max_remaining) => {
                Some("weekly_quota_above_threshold")
            }
            Some(_) => None,
            None => Some("weekly_quota_unavailable"),
        })
    }

    pub(super) async fn reconcile_automation(
        &self,
        account_id: &ProviderAccountId,
    ) -> Result<(), AdminError> {
        let lock = self.reset_credit_lock(account_id).await;
        let _guard = lock.lock().await;
        let settings = self.automation_settings().await?;
        let mut record = match self
            .accounts
            .reset_credit_automation(account_id.as_str())
            .await
            .map_err(|e| map_store_error(e, "reset-credit automation"))?
        {
            Some(record) => record,
            None if settings.reset_credit_default_enabled => {
                if !self.require_reset_credit_automation(account_id).await? {
                    return Ok(());
                }
                let mut record = new_automation(account_id, Utc::now());
                record.state.inherits_default = true;
                record.enabled = true;
                record
            }
            None => return Ok(()),
        };
        if record.state.inherits_default {
            record.enabled = settings.reset_credit_default_enabled;
        }
        if let Some(refresh) = &record.state.quota_refresh {
            if refresh.next_attempt_at <= Utc::now() {
                return self.refresh_consumed_quota(account_id, &mut record).await;
            }
            return Ok(());
        }
        if !record.enabled {
            return Ok(());
        }
        match self.require_reset_credit_automation(account_id).await {
            Ok(true) => {}
            Ok(false) => return Ok(()),
            Err(error) => {
                record.next_run_at = next_discovery(Utc::now(), self.timezone, OsRng.next_u64())
                    .ok_or_else(|| AdminError::internal("无法计算重置卡发现时间"))?;
                self.save_automation(&mut record, None).await?;
                return Err(error);
            }
        }
        let now = Utc::now();
        let settings = self.automation_settings().await?;
        let lead = settings.reset_credit_lead_minutes;
        record
            .state
            .credits
            .retain(|credit| credit.expires_at > now);
        if record
            .state
            .pending
            .as_ref()
            .is_some_and(|pending| pending.credit.expires_at <= now)
        {
            record.state.pending = None;
            record.state.retry_at = None;
            record.state.last_result = Some("expired_unknown".to_owned());
        }
        if record.state.retry_at.is_some_and(|retry| retry > now) {
            reschedule(&mut record, lead, now);
            return self.save_automation(&mut record, None).await;
        }
        let context = MutationContext {
            actor: MutationActor::System,
            request_id: "reset-credit-automation".to_owned(),
        };
        let weekly_check_due = record
            .state
            .next_quota_check_at
            .is_none_or(|due| due <= now);
        let mut weekly_exhausted = false;
        if weekly_check_due && record.state.pending.is_none() {
            record.state.next_quota_check_at =
                Some(now + chrono::Duration::seconds(300 + i64::from(OsRng.next_u32() % 60)));
            reschedule(&mut record, lead, now);
            self.save_automation(&mut record, None).await?;
            match self.quota(account_id, true).await {
                Ok(item) => {
                    weekly_exhausted = item
                        .quota
                        .usage_windows()
                        .find(|(_, period)| *period == AccountUsagePeriod::Weekly)
                        .and_then(|(window, _)| window.used_percent)
                        .is_some_and(|used| used.is_finite() && used >= 100.0)
                }
                Err(error) => {
                    record.state.last_result = Some("weekly_quota_unavailable".to_owned());
                    self.save_automation(&mut record, None).await?;
                    return Err(error);
                }
            }
        }
        let due_credit =
            weekly_exhausted || next_consumption(&record, lead).is_some_and(|due| due <= now);
        if record.state.pending.is_none()
            && (due_credit
                || record.state.next_discovery_at <= now
                || record.state.retry_at.is_some())
        {
            // 先保存下一次发现，查询被取消或进程重启也不会重复刷普通发现接口
            if record.state.next_discovery_at <= now {
                record.state.next_discovery_at =
                    next_discovery(now, self.timezone, OsRng.next_u64())
                        .ok_or_else(|| AdminError::internal("无法计算重置卡发现时间"))?;
            }
            if due_credit || record.state.retry_at.is_some() {
                record.state.retry_at = Some(now + chrono::Duration::seconds(30));
            }
            reschedule(&mut record, lead, now);
            self.save_automation(&mut record, None).await?;
            // 真正消费前重新核对卡片；每日发现结果也可直接承担本次核对
            match self
                .reset_credits_upstream(&context, account_id.clone())
                .await
            {
                Ok(credits) => self.observe_reset_credits(&mut record, &credits, Utc::now())?,
                Err(error) => {
                    self.automation_failed(&mut record, lead, Utc::now())
                        .await?;
                    return Err(error);
                }
            }
            if let Some(credit) = record
                .state
                .credits
                .iter()
                .find(|credit| {
                    weekly_exhausted || consume_at(account_id.as_str(), credit, lead) <= Utc::now()
                })
                .cloned()
            {
                if let Some(max_remaining) = settings.reset_credit_max_weekly_remaining_percent {
                    // 已发出的未决请求跳过此检查，继续沿原幂等键确认结果
                    match self
                        .weekly_reset_credit_gate(account_id, max_remaining)
                        .await
                    {
                        Ok(Some(reason)) => {
                            return self.defer_automation(&mut record, lead, reason).await;
                        }
                        Ok(None) => {}
                        Err(error) => {
                            self.defer_automation(&mut record, lead, "weekly_quota_unavailable")
                                .await?;
                            return Err(error);
                        }
                    }
                }
                // 刷新额度期间卡片可能到期，不能把已过期卡片持久化为待消费
                if credit.expires_at <= Utc::now() {
                    record
                        .state
                        .credits
                        .retain(|credit| credit.expires_at > Utc::now());
                    reschedule(&mut record, lead, Utc::now());
                    return self.save_automation(&mut record, None).await;
                }
                record.state.pending = Some(PendingResetCredit {
                    credit,
                    redeem_request_id: uuid::Uuid::new_v4().to_string(),
                });
                record.state.last_result = Some("pending".to_owned());
                record.state.retry_at = Some(Utc::now() + chrono::Duration::seconds(30));
                reschedule(&mut record, lead, Utc::now());
                // 必须在发送消费请求前落盘，进程退出或响应丢失继续使用相同幂等键
                self.save_automation(&mut record, None).await?;
            }
        }
        let Some(pending) = record.state.pending.clone() else {
            reschedule(&mut record, lead, Utc::now());
            return self.save_automation(&mut record, None).await;
        };
        let command = ConsumeProviderResetCredit {
            account_id: account_id.clone(),
            credit_id: Some(pending.credit.id.clone()),
            redeem_request_id: uuid::Uuid::parse_str(&pending.redeem_request_id)
                .map_err(|_| AdminError::internal("重置卡幂等键不合法"))?,
        };
        let refresh_baseline = self.quota_refresh_baseline(account_id).await;
        match self.consume_reset_credit_upstream(&context, command).await {
            Ok(result) => {
                let confirmed = matches!(result.code.as_str(), "reset" | "already_redeemed");
                record.state.pending = None;
                if result.code == "nothing_to_reset" {
                    return self
                        .defer_automation(&mut record, lead, "nothing_to_reset")
                        .await;
                }
                record
                    .state
                    .credits
                    .retain(|credit| credit.id != pending.credit.id);
                record.state.retry_at = None;
                record.state.failures = 0;
                record.state.last_result = Some(if confirmed {
                    result.code
                } else {
                    "not_redeemed".to_owned()
                });
                reschedule(&mut record, lead, Utc::now());
                if confirmed {
                    record.state.quota_refresh = Some(refresh_baseline);
                    reschedule(&mut record, lead, Utc::now());
                }
                // 消费终态与额度核验任务同时落盘，重启后只恢复核验
                self.save_automation(&mut record, None).await?;
                if confirmed {
                    self.refresh_consumed_quota(account_id, &mut record).await?;
                }
            }
            Err(error) => {
                self.automation_failed(&mut record, lead, Utc::now())
                    .await?;
                return Err(error);
            }
        }
        Ok(())
    }
}

fn new_automation(account_id: &ProviderAccountId, now: DateTime<Utc>) -> ResetCreditAutomation {
    ResetCreditAutomation {
        account_id: account_id.as_str().to_owned(),
        enabled: false,
        revision: 0,
        next_run_at: now,
        state: ResetCreditAutomationState {
            inherits_default: true,
            next_quota_check_at: None,
            quota_refresh: None,
            next_discovery_at: now,
            credits: vec![],
            pending: None,
            retry_at: None,
            failures: 0,
            last_result: None,
        },
    }
}

/// 当地次日随机发现时间；随机数由调用者提供，结果持久化后不会因重启重抽
fn next_discovery(
    now: DateTime<Utc>,
    timezone: DeploymentTimeZone,
    random: u64,
) -> Option<DateTime<Utc>> {
    let start = timezone.days_after(now, 1)?;
    let end = timezone.days_after(now, 2)?;
    let seconds = u64::try_from((end - start).num_seconds()).ok()?;
    if seconds == 0 {
        return None;
    }
    start.checked_add_signed(chrono::Duration::seconds(
        i64::try_from(random % seconds).ok()?,
    ))
}

/// 提前窗口的前 10% 用于账号错峰，最多一分钟，其余时间保留给网络失败恢复
fn consume_at(account_id: &str, credit: &PlannedResetCredit, lead_minutes: u32) -> DateTime<Utc> {
    let lead_seconds = i64::from(lead_minutes) * 60;
    let mut hash = Sha256::new();
    hash.update(account_id.as_bytes());
    hash.update([0]);
    hash.update(credit.id.as_bytes());
    let digest = hash.finalize();
    let offset =
        i64::from(u16::from_be_bytes([digest[0], digest[1]])) % (lead_seconds / 10).clamp(1, 60);
    credit.expires_at - chrono::Duration::seconds(lead_seconds - offset)
}

fn next_consumption(record: &ResetCreditAutomation, lead_minutes: u32) -> Option<DateTime<Utc>> {
    record
        .state
        .credits
        .iter()
        .map(|credit| consume_at(&record.account_id, credit, lead_minutes))
        .min()
}

fn reschedule(record: &mut ResetCreditAutomation, lead_minutes: u32, now: DateTime<Utc>) {
    record.next_run_at = if let Some(refresh) = &record.state.quota_refresh {
        refresh.next_attempt_at
    } else if let Some(retry) = record.state.retry_at {
        retry
    } else if record.state.pending.is_some() {
        now
    } else {
        let discovery = next_consumption(record, lead_minutes)
            .map_or(record.state.next_discovery_at, |due| {
                due.min(record.state.next_discovery_at)
            });
        record
            .state
            .next_quota_check_at
            .map_or(now, |due| due.min(discovery))
    };
}
