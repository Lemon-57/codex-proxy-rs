//! 验证账号通知合并、慢订阅者隔离及取消后回收

use futures::{FutureExt, StreamExt};
use gateway_core::account::AccountStateNotifications;

#[test]
fn slow_subscribers_do_not_block_state_commits_or_other_consumers() {
    let source = AccountStateNotifications::default();
    let mut left = source.subscribe();
    let mut right = source.subscribe();
    for _ in 0..10_000 {
        source.notify();
    }
    assert_eq!(left.next().now_or_never(), Some(Some(())));
    assert!(left.next().now_or_never().is_none());
    source.notify();
    assert_eq!(left.next().now_or_never(), Some(Some(())));
    assert_eq!(right.next().now_or_never(), Some(Some(())));
    drop(right);
    source.notify();
    assert_eq!(left.next().now_or_never(), Some(Some(())));
}
