set local lock_timeout = '5ms';
set local statement_timeout = '500ms';

-- 旧进程自然排空期间继续读取原列，新增设置与旧上限保持事务内一致
alter table provider_accounts
    add column quota_settings jsonb not null default '{}'::jsonb,
    add column quota_warmup_state jsonb not null default '{}'::jsonb,
    add column quota_warmup_revision bigint not null default 0;

create function provider_account_quota_limits(settings jsonb) returns jsonb
language sql immutable strict as $function$
    select coalesce(jsonb_object_agg(key, value -> 'limitPercent'), '{}'::jsonb)
    from jsonb_each(settings)
    where value -> 'limitPercent' is not null and value -> 'limitPercent' <> 'null'::jsonb
$function$;

create function provider_account_quota_settings(limits jsonb, settings jsonb) returns jsonb
language sql immutable strict as $function$
    select coalesce(jsonb_object_agg(bucket.key, jsonb_build_object(
        'limitPercent', limits -> bucket.key,
        'warmup', settings -> bucket.key -> 'warmup'
    )), '{}'::jsonb)
    from (
        select key from jsonb_each(limits)
        union
        select key from jsonb_each(settings)
        where value -> 'warmup' is not null and value -> 'warmup' <> 'null'::jsonb
    ) bucket
$function$;

create function synchronize_provider_account_quota_settings() returns trigger
language plpgsql as $function$
begin
    if tg_op = 'INSERT' then
        if new.quota_settings = '{}'::jsonb and new.quota_limits <> '{}'::jsonb then
            new.quota_settings := provider_account_quota_settings(new.quota_limits, new.quota_settings);
        else
            new.quota_limits := provider_account_quota_limits(new.quota_settings);
        end if;
    elsif new.quota_limits is distinct from old.quota_limits
        and new.quota_settings is not distinct from old.quota_settings then
        -- 原版本的管理写入只替换上限，保留新版本已有的桶预热配置
        new.quota_settings := provider_account_quota_settings(new.quota_limits, new.quota_settings);
    elsif new.quota_settings is distinct from old.quota_settings then
        if new.quota_limits is distinct from old.quota_limits
            and new.quota_limits <> provider_account_quota_limits(new.quota_settings) then
            raise exception 'Conflicting account quota settings';
        end if;
        new.quota_limits := provider_account_quota_limits(new.quota_settings);
    end if;
    return new;
end
$function$;

create trigger provider_account_quota_settings_sync
before insert or update of quota_limits, quota_settings on provider_accounts
for each row execute function synchronize_provider_account_quota_settings();

update provider_accounts
set quota_settings = provider_account_quota_settings(quota_limits, quota_settings)
where quota_limits <> '{}'::jsonb;
