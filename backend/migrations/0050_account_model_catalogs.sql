set local lock_timeout = '5ms';
set local statement_timeout = '500ms';

-- 新表创建与对在线账号表的外键登记分开
create table provider_account_model_catalogs (
    account_id text primary key,
    credential_revision bigint not null check (credential_revision > 0),
    plan_type text,
    etag text,
    observed_at timestamptz not null,
    document bytea not null,
    document_hash bytea generated always as (sha256(document)) stored
);
