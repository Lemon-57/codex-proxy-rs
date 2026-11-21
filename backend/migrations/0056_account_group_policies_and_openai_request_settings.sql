-- 分组策略沿用账号模型规则，既有分组允许所有模型及子代理
set local lock_timeout = '100ms';
set local statement_timeout = '1500ms';
lock table account_groups, runtime_settings in access exclusive mode nowait;

alter table account_groups
  drop constraint account_groups_fast_mode_check,
  add constraint account_groups_fast_mode_check check (fast_mode in ('default', 'enabled', 'disabled', 'ultrafast')),
  add column allow_subagents boolean not null default true,
  add column model_access_json jsonb not null default '{"mode":"all","models":[]}'::jsonb,
  add constraint account_groups_model_access_ck check (
    jsonb_typeof(model_access_json) = 'object'
    and model_access_json ?& array['mode', 'models']
    and model_access_json - 'mode' - 'models' = '{}'::jsonb
    and octet_length(model_access_json::text) <= 131072
    and case when jsonb_typeof(model_access_json -> 'models') = 'array' then
      case model_access_json ->> 'mode'
        when 'all' then jsonb_array_length(model_access_json -> 'models') = 0
        when 'allowlist' then jsonb_array_length(model_access_json -> 'models') between 1 and 256
        when 'denylist' then jsonb_array_length(model_access_json -> 'models') between 1 and 256
        else false
      end
      and not jsonb_path_exists(model_access_json, '$.models[*] ? (@.type() != "string")')
    else false end
  );

-- 保留既有实例的审核模型与请求国家行为
alter table runtime_settings
  add column openai_oauth_guardian_review_model boolean not null default false,
  add column openai_request_country_json jsonb not null default '{"mode":"passthrough"}'::jsonb,
  add constraint runtime_settings_openai_request_country_ck check (
    openai_request_country_json in ('{"mode":"default"}', '{"mode":"passthrough"}', '{"mode":"follow_location"}')
    or (
      openai_request_country_json ?& array['mode', 'country']
      and openai_request_country_json = jsonb_build_object('mode', 'custom', 'country', openai_request_country_json->>'country')
      and (openai_request_country_json->>'country') collate "C" ~ '^[A-Z]{2}$'
      and jsonb_typeof(openai_request_country_json->'country') = 'string'
    )
  );

alter table runtime_settings
  alter column openai_oauth_guardian_review_model set default true,
  alter column openai_request_country_json set default '{"mode":"follow_location"}'::jsonb;
