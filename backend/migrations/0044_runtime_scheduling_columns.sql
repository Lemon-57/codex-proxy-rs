set local lock_timeout = '5ms';
set local statement_timeout = '500ms';

-- 常量默认值只登记元数据，旧读取列继续保留
alter table runtime_settings
    add column account_warmup_cron text not null default '',
    add column reset_credit_on_low_weekly_quota boolean not null default false,
    add column peer_routing_id uuid;
