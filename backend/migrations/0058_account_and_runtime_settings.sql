-- v3.55.2 在定制编号下的兼容迁移，承接上游 0046_account_and_runtime_settings。
-- 定制版 0001-0057 已在生产按定制编号应用，校验和必须保持，故本迁移改号为 0058。
-- 亲和配置采用新增列 + 回填，旧 openai_* 列与旧约束保留：切换期间旧 3.54.5 实例仍读取旧列，
-- 待旧实例全部退役后再用后续迁移删除旧列与旧约束。新代码只读写新列。

-- 上游明确指出耗尽窗口时，随该次拒绝保留其时长
do $$
begin
    if not exists (
        select 1
        from information_schema.columns
        where table_schema = current_schema()
          and table_name = 'provider_accounts'
          and column_name = 'quota_limit_window_minutes'
    ) then
        alter table provider_accounts
            add column quota_limit_window_minutes integer
            check (quota_limit_window_minutes between 1 and 65535);
    end if;
end $$;

-- 记录最近一次运行设置写入对应的全局发布版本，供编辑草稿判断冲突
do $$
begin
    if not exists (
        select 1
        from information_schema.columns
        where table_schema = current_schema()
          and table_name = 'runtime_settings'
          and column_name = 'settings_revision'
    ) then
        alter table runtime_settings add column settings_revision bigint not null default 1;
        update runtime_settings set settings_revision = config_revision;
    end if;
    if not exists (
        select 1
        from pg_constraint
        where conname = 'runtime_settings_edit_revision_ck'
          and conrelid = 'runtime_settings'::regclass
    ) then
        alter table runtime_settings add constraint runtime_settings_edit_revision_ck
            check (settings_revision between 1 and config_revision);
    end if;
end $$;

-- 亲和配置由所有 Provider 共用；新增列后立即从旧列回填一次，保留旧列供旧实例过渡读取
do $$
begin
    if not exists (
        select 1
        from information_schema.columns
        where table_schema = current_schema()
          and table_name = 'runtime_settings'
          and column_name = 'account_affinity'
    ) then
        alter table runtime_settings
            add column account_affinity text not null default 'strict'
            constraint runtime_settings_account_affinity_check
            check (account_affinity in ('relaxed', 'preferred', 'strict'));
        update runtime_settings set account_affinity = openai_account_affinity;
    end if;
    if not exists (
        select 1
        from information_schema.columns
        where table_schema = current_schema()
          and table_name = 'runtime_settings'
          and column_name = 'session_affinity_ttl_hours'
    ) then
        alter table runtime_settings
            add column session_affinity_ttl_hours bigint not null default 24
            constraint runtime_settings_session_affinity_ttl_hours_check
            check (session_affinity_ttl_hours between 1 and 720);
        update runtime_settings set session_affinity_ttl_hours = openai_session_affinity_ttl_hours;
    end if;
end $$;
