set local lock_timeout = '5ms';
set local statement_timeout = '500ms';

alter table provider_account_model_catalogs
    add constraint provider_account_model_catalogs_account_id_fkey
    foreign key(account_id) references provider_accounts(id) on delete cascade not valid;
