-- 提前窗口只要求正数，保留现有整数存储类型和其他运行设置
-- 锁等待失败时回滚本次迁移，不让 DDL 排队阻挡在用实例
SET LOCAL lock_timeout = '5ms';
SET LOCAL statement_timeout = '250ms';

ALTER TABLE runtime_settings
    DROP CONSTRAINT runtime_settings_reset_credit_lead_minutes_check,
    ADD CONSTRAINT runtime_settings_reset_credit_lead_minutes_check
        CHECK (reset_credit_lead_minutes >= 1);
