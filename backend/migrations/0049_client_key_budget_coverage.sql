set local lock_timeout = '5ms';
set local statement_timeout = '500ms';

-- 覆盖回填仅持有行锁，和新增列的排他锁分开
with coverage as (
    select coalesce(min(completed_at), now()) as since from client_api_key_charge_events
)
update client_api_key_budget_windows w set charge_history_since = coverage.since
from coverage, client_api_keys k
where k.id = w.client_api_key_id and k.created_at < coverage.since;
