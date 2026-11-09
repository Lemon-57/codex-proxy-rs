-- 默认策略与账号独立开关分离，不覆盖已有账号选择
SET LOCAL lock_timeout = '5ms';
SET LOCAL statement_timeout = '250ms';
alter table runtime_settings add column reset_credit_default_enabled boolean not null default false;
