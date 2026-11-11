set local lock_timeout = '5ms';
set local statement_timeout = '500ms';

alter table runtime_settings
    alter column peer_routing_id set not null,
    alter column peer_routing_id set default gen_random_uuid(),
    add constraint runtime_settings_warmup_cron_ck check (
        octet_length(account_warmup_cron) <= 255
        and account_warmup_cron = btrim(account_warmup_cron)
        and account_warmup_cron !~ '[[:cntrl:]]'
        and (not account_warmup_enabled or account_warmup_cron <> '')
    ) not valid;
