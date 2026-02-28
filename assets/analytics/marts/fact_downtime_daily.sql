


    with incremental_date_range as (
        select
            from_timestamp,
            date_trunc('day', from_timestamp) as buffer_from_timestamp,
            

    (from_timestamp + cast(3 as bigint) * interval 1 month) as to_timestamp
        from (
            select (select coalesce(min(ingested_timestamp), TIMESTAMP '2025-10-01') from "memory"."main"."stg_ocpp_logs") as from_timestamp
        )
    ),


ports as (
    select
        charge_point_id,
        port_id,
        commissioned_ts,
        decommissioned_ts
    from "memory"."main"."stg_ports"
),

-- Get faulted outages first to filter offline outages
faulted_outages as (
    select
        charge_point_id,
        port_id,
        from_ts,
        to_ts,
        duration_minutes,
        incremental_ts,
        'FAULTED' as type
    from "memory"."main"."int_faulted_outages"
    where incremental_ts > (select buffer_from_timestamp from incremental_date_range)
        and incremental_ts <= (select to_timestamp from incremental_date_range)
),

-- for Offline outages (charge point level, need to join with ports)
-- Exclude the ones that started during a faulted outage - port reported faulted then went offline
offline_outages as (
    select
        o.charge_point_id,
        p.port_id,
        o.from_ts,
        o.to_ts,
        o.duration_minutes,
        o.incremental_ts,
        'OFFLINE' as type
    from "memory"."main"."int_offline_outages" o
    join ports p on o.charge_point_id = p.charge_point_id
    where o.incremental_ts > (select buffer_from_timestamp from incremental_date_range)
        and o.incremental_ts <= (select to_timestamp from incremental_date_range)
        and not exists (
            select 1
            from faulted_outages f
            where f.charge_point_id = o.charge_point_id
                and f.port_id = p.port_id
                and o.from_ts >= f.from_ts
                and o.from_ts < f.to_ts
        )
),

outages as (
    select * from offline_outages
    union all
    select * from faulted_outages
),

filtered_outages as (
    select
        o.*,
        d.date_id
    from outages o
    join "memory"."main"."dim_dates" d 
        on date_id between date_trunc('day', o.from_ts) and date_trunc('day', o.to_ts)
),

incremental as (
    select max(incremental_ts) as incremental_ts
    from filtered_outages
),

-- Compute per-day overlap
outage_days as (
    select
        o.charge_point_id,
        o.port_id,
        o.date_id,
        o.type,
        greatest(o.from_ts, o.date_id) as interval_start,
        least(o.to_ts, 

    (o.date_id + cast(1 as bigint) * interval 1 day)) as interval_end
    from filtered_outages o
),

per_day as (
    select
        charge_point_id,
        port_id,
        date_id,
        type,
        
        (date_diff('minutes', interval_start::timestamp, interval_end::timestamp ))
     as duration_minutes
    from outage_days
),

final as (
    select
        date_id,
        charge_point_id,
        port_id,
        type,
        sum(duration_minutes) as duration_minutes
    from per_day
    group by 1, 2, 3, 4
)

select *,
    -- Generate a deterministic unique ID from the composite key
    md5(cast(coalesce(cast(date_id as TEXT), '_dbt_utils_surrogate_key_null_') || '-' || coalesce(cast(charge_point_id as TEXT), '_dbt_utils_surrogate_key_null_') || '-' || coalesce(cast(port_id as TEXT), '_dbt_utils_surrogate_key_null_') || '-' || coalesce(cast(type as TEXT), '_dbt_utils_surrogate_key_null_') as TEXT)) as downtime_id,
    (select incremental_ts from incremental) as incremental_ts
from final