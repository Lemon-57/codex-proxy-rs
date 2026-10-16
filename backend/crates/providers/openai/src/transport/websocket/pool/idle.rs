//! 普通会话空闲连接的账号级 LRU 预算，保留下游 WS 精确续写和在途连接

use tokio::time::Instant;

use super::CodexWebSocketPool;
use super::state::{
    CodexWebSocketPoolKey, PooledWebSocketConnection, WebSocketPoolSlot, WebSocketPoolState,
};

impl CodexWebSocketPool {
    pub(super) fn idle_expired(
        &self,
        key: &CodexWebSocketPoolKey,
        idle_since: Instant,
        now: Instant,
    ) -> bool {
        !key.preserves_downstream_continuation()
            && self
                .config
                .idle_timeout
                .is_some_and(|timeout| now.duration_since(idle_since) >= timeout)
    }

    pub(super) fn take_excess_idle(
        &self,
        state: &mut WebSocketPoolState,
        account_id: &str,
        now: Instant,
    ) -> Vec<PooledWebSocketConnection> {
        let mut idle = state
            .slots
            .iter()
            .filter_map(|(key, slot)| match slot {
                WebSocketPoolSlot::Idle { idle_since, .. }
                    if key.account_id() == account_id
                        && !key.preserves_downstream_continuation() =>
                {
                    Some((key.clone(), *idle_since))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        idle.sort_unstable_by_key(|(_, since)| std::cmp::Reverse(*since));
        let mut close = Vec::new();
        for (key, _) in idle.into_iter().skip(self.config.max_idle_per_account) {
            if let Some(WebSocketPoolSlot::Idle { connection, .. }) = state.slots.remove(&key) {
                // 明确记录续接丢失，不能把被驱逐的 previous_response_id 当成新链猜测重放
                state.remember_continuation_loss(
                    &key,
                    connection.continuation.latest_response_id(),
                    connection
                        .websocket
                        .observation()
                        .with_exit_reason("idle_budget_exceeded"),
                    now,
                );
                close.push(*connection);
            }
        }
        close
    }
}
