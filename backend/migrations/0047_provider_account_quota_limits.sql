set local lock_timeout = '5ms';
set local statement_timeout = '500ms';

alter table provider_accounts
    add column quota_limits jsonb not null default '{}'::jsonb,
    add column quota_paused boolean not null default false;
