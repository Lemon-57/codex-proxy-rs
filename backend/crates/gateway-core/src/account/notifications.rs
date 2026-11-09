//! 账号持久状态变化的有界唤醒信号，不承载凭据或业务策略

use std::sync::{Arc, Mutex};

/// 通知可合并且不等待订阅者，消费者须重新读取权威状态并保留周期补偿
#[derive(Clone, Default)]
pub struct AccountStateNotifications {
    subscribers: Arc<Mutex<Vec<futures::channel::mpsc::Sender<()>>>>,
}

impl AccountStateNotifications {
    pub fn subscribe(&self) -> futures::channel::mpsc::Receiver<()> {
        let (sender, receiver) = futures::channel::mpsc::channel(0);
        self.subscribers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(sender);
        receiver
    }

    /// 仅在状态成功提交后调用，不在账号事务中执行插件
    pub fn notify(&self) {
        self.subscribers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .retain_mut(|sender| match sender.try_send(()) {
                Ok(()) => true,
                Err(error) => error.is_full(),
            });
    }
}
