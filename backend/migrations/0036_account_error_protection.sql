-- 保留旧实例仍读取的列，允许现有请求自然排空和回退
SET LOCAL lock_timeout = '5ms';
SET LOCAL statement_timeout = '250ms';
-- 新安装与升级均从关闭且无规则开始，不继承旧保护配置
alter table runtime_settings add column account_error_protection_json jsonb not null default '{"enabled":false,"rules":[],"threshold":12,"windowSeconds":600,"durationSeconds":7200,"probeEnabled":true,"probeModel":null}'::jsonb;
alter table runtime_settings add column account_error_protection_revision bigint not null default 1 check (account_error_protection_revision > 0);
update runtime_settings set account_error_protection_revision = config_revision;

-- 延续现有容量保护开关和阈值，不因升级静默关闭保护
update runtime_settings set account_error_protection_json = jsonb_build_object(
    'enabled', account_auto_freeze_enabled,
    'threshold', account_auto_freeze_threshold,
    'windowSeconds', account_auto_freeze_window_seconds,
    'durationSeconds', account_auto_freeze_duration_seconds,
    'probeEnabled', account_auto_freeze_probe_enabled,
    'probeModel', account_auto_freeze_probe_model,
    'rules', '[{"id":"capacity-code","name":"上游容量保护 code","conditions":[{"field":"provider","operator":"equals","values":["openai"]},{"field":"code","operator":"equals","values":["server_is_overloaded","slow_down"],"ignoreCase":true}]},{"id":"capacity-type","name":"上游容量保护 type","conditions":[{"field":"provider","operator":"equals","values":["openai"]},{"field":"type","operator":"equals","values":["server_is_overloaded","slow_down"],"ignoreCase":true}]},{"id":"capacity-message","name":"上游容量保护 message","conditions":[{"field":"provider","operator":"equals","values":["openai"]},{"field":"message","operator":"equals","values":["server_is_overloaded","slow_down"],"ignoreCase":true}]},{"id":"capacity-message-text","name":"上游容量描述","conditions":[{"field":"provider","operator":"equals","values":["openai"]},{"field":"message","operator":"contains","values":["selected model is at capacity","server is overloaded","server_overloaded"],"ignoreCase":true}]}]'::jsonb
);
