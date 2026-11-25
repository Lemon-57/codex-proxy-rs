-- 人工维护的供应商和到期时间不参与调度
alter table provider_accounts
    add column supplier text check (supplier is null or char_length(supplier) <= 100),
    add column manual_expires_at timestamptz;
