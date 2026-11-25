set local lock_timeout = '5ms';
set local statement_timeout = '500ms';

create table provider_account_native_refresh_plans (
    account_id text primary key,
    credential_revision bigint not null check (credential_revision > 0),
    plan_type text not null check (plan_type ~ '^[a-z0-9_-]{1,128}$' and plan_type <> 'unknown'),
    observed_at timestamptz not null
);
