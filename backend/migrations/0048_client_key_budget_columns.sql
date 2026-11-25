set local lock_timeout = '5ms';
set local statement_timeout = '500ms';

alter table client_api_key_budget_windows
    add column daily_explicit boolean not null default false,
    add column weekly_explicit boolean not null default false,
    add column revision bigint not null default 0,
    add column last_operation_request jsonb,
    add column charge_history_since timestamptz,
    add constraint client_key_budget_revision_ck check (revision >= 0) not valid,
    add constraint daily_explicit_window_order check (not daily_explicit or daily_start < daily_end) not valid,
    add constraint weekly_explicit_window_order check (not weekly_explicit or weekly_start < weekly_end) not valid;
