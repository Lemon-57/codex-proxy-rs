-- 共享数据库在线升级保留旧实例读取列，迁移事务只短暂申请表锁
set local lock_timeout = '5ms';
set local statement_timeout = '500ms';

alter table runtime_settings
    add column account_warmup_cron text not null default '' check (
        octet_length(account_warmup_cron) <= 255
        and account_warmup_cron = btrim(account_warmup_cron)
        and account_warmup_cron !~ '[[:cntrl:]]'
    ),
    add column reset_credit_on_low_weekly_quota boolean not null default false,
    -- 同一部署的实例共用转发标记，启动时读取，不随请求访问数据库
    add column peer_routing_id uuid;

update runtime_settings set account_warmup_cron =
    case when account_warmup_schedule_time = '' then ''
    else split_part(account_warmup_schedule_time, ':', 2) || ' ' || split_part(account_warmup_schedule_time, ':', 1) || ' * * *' end,
    peer_routing_id = gen_random_uuid();

alter table runtime_settings add constraint runtime_settings_warmup_cron_enabled_ck
    check (not account_warmup_enabled or account_warmup_cron <> '');

-- 易变默认值分步登记，避免为增加单例标记重写整张设置表
alter table runtime_settings
    alter column peer_routing_id set not null,
    alter column peer_routing_id set default gen_random_uuid();

alter table provider_accounts
    add column quota_limits jsonb not null default '{}'::jsonb,
    add column quota_paused boolean not null default false;

-- 显式窗口复用 Key 费用账本，仅保留当前版本及最近一次窗口变更
alter table client_api_key_budget_windows
    add column daily_explicit boolean not null default false,
    add column weekly_explicit boolean not null default false,
    add column revision bigint not null default 0 check (revision >= 0),
    add column last_operation_request jsonb,
    add column charge_history_since timestamptz,
    add constraint daily_explicit_window_order check (not daily_explicit or daily_start < daily_end),
    add constraint weekly_explicit_window_order check (not weekly_explicit or weekly_start < weekly_end);

-- 窗口接管只扫描当前 Key 的周期内费用
-- 部署前先并发创建索引，此处只核对同名对象存在
create index if not exists client_api_key_charge_events_completed
    on client_api_key_charge_events (client_api_key_id, completed_at) include (amount_usd);

-- 旧业务备份恢复时会清空整张费用事件表，最早留存费用之前的历史无法确认
-- 在此之后创建的 Key 不可能有更早的消费
with coverage as (
    select coalesce(min(completed_at), now()) as since from client_api_key_charge_events
)
update client_api_key_budget_windows w set charge_history_since = coverage.since
from coverage, client_api_keys k
where k.id = w.client_api_key_id and k.created_at < coverage.since;
