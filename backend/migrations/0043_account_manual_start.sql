-- 候选实例遇到占用立即退出，避免等待锁阻塞现有业务
SET LOCAL lock_timeout = '5ms';
SET LOCAL statement_timeout = '500ms';
alter table provider_accounts add column manual_starts_at timestamptz;
