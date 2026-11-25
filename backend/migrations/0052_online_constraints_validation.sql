set local lock_timeout = '5ms';
set local statement_timeout = '500ms';

-- 验证锁允许普通读写，不把表扫描放进新增列的排他锁事务
alter table runtime_settings validate constraint runtime_settings_warmup_cron_ck;
alter table client_api_key_budget_windows
    validate constraint client_key_budget_revision_ck,
    validate constraint daily_explicit_window_order,
    validate constraint weekly_explicit_window_order;
alter table provider_account_model_catalogs
    validate constraint provider_account_model_catalogs_account_id_fkey;
-- 生产上线前并发创建索引，迁移仅核对已登记对象
create index if not exists client_api_key_charge_events_completed
    on client_api_key_charge_events(client_api_key_id, completed_at) include(amount_usd);
