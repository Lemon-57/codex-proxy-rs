set local lock_timeout = '5ms';
set local statement_timeout = '500ms';

-- DML 单独提交，生成部署标记时不持有 DDL 排他锁
update runtime_settings set account_warmup_cron =
    split_part(account_warmup_schedule_time, ':', 2) || ' ' ||
    split_part(account_warmup_schedule_time, ':', 1) || ' * * *',
    peer_routing_id = gen_random_uuid();
