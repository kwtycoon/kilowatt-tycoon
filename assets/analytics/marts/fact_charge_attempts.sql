




    with incremental_date_range as (
        select
            from_timestamp,
            

    (from_timestamp + cast(-30 as bigint) * interval 1 minute) as buffer_from_timestamp,
            least(
                

    (from_timestamp + cast(3 as bigint) * interval 1 month),
                (select max(incremental_ts) from "memory"."main"."int_connector_preparing"),
                (select max(incremental_ts) from "memory"."main"."int_transactions")
            ) as to_timestamp
        from
            (
                select (select coalesce(min(ingested_timestamp), TIMESTAMP '2025-10-01') from "memory"."main"."stg_ocpp_logs") as from_timestamp
            )
    ),


preparing as (
    select
        charge_point_id,
        connector_id,
        unique_id as preparing_unique_id,
        ingested_ts as preparing_ingested_ts,
        previous_ingested_ts,
        next_ingested_ts,
        previous_status,
        status,
        next_status,
        payload_ts,
        next_payload_ts,
        id_tags,
        id_tag_statuses,
        transaction_id,
        error_codes,
        incremental_ts,

        -- Attempt start timestamp: use payload_ts if available, otherwise ingested_ts
        coalesce(payload_ts, ingested_ts) as preparing_start_ts,
        coalesce(next_payload_ts, next_ingested_ts) as preparing_stop_ts
    from "memory"."main"."int_connector_preparing"
    where ingested_ts > (select from_timestamp from incremental_date_range)
        and ingested_ts <= (select to_timestamp from incremental_date_range)
),

transactions as (
    select
        charge_point_id,
        connector_id,
        transaction_id,
        ingested_ts as transaction_ingested_ts,
        transaction_start_ts,
        transaction_stop_ts,
        transaction_stop_reason,
        id_tags,
        id_tag_statuses,
        meter_start_wh,
        meter_stop_wh,
        energy_transferred_kwh,
        error_codes,
        incremental_ts as transaction_incremental_ts
    from "memory"."main"."int_transactions"
    where ingested_ts > (select from_timestamp from incremental_date_range)
        and ingested_ts <= (select to_timestamp from incremental_date_range)
),

incremental as (
    select
        greatest(
            coalesce(
                (select max(preparing_ingested_ts) from preparing), 
                '1900-01-01'::timestamp
            ),
            coalesce(
                (select max(transaction_ingested_ts) from transactions), 
                '1900-01-01'::timestamp
            )
        ) as incremental_ts
),

attempts_and_transactions as (
    select
        -- Charge attempt identifiers
        coalesce(p.charge_point_id, t.charge_point_id) as charge_point_id,
        coalesce(p.connector_id, t.connector_id) as connector_id,

        -- Attempt start and stop timestamps depending on what we know
        coalesce(p.preparing_start_ts, t.transaction_start_ts) as charge_attempt_start_ts,
        coalesce(t.transaction_stop_ts, p.preparing_stop_ts) as charge_attempt_stop_ts,
        
        -- Charge attempt details
        p.preparing_ingested_ts,
        p.preparing_unique_id,
        p.previous_status,
        p.status,
        p.next_status,
        p.payload_ts as preparing_payload_ts,
        p.next_payload_ts as preparing_next_payload_ts,
        array_distinct(
    
    
        
    

    case
        when p.id_tags is null and t.id_tags is null
            then null
        when p.id_tags is null
            then t.id_tags
        when t.id_tags is null
            then p.id_tags
        else list_concat(p.id_tags, t.id_tags)
    end
) as id_tags,
        array_distinct(
    
    
        
    

    case
        when p.id_tag_statuses is null and t.id_tag_statuses is null
            then null
        when p.id_tag_statuses is null
            then t.id_tag_statuses
        when t.id_tag_statuses is null
            then p.id_tag_statuses
        else list_concat(p.id_tag_statuses, t.id_tag_statuses)
    end
) as id_tag_statuses,

        -- Transaction details
        coalesce(p.transaction_id, t.transaction_id) as transaction_id,
        t.transaction_start_ts,
        t.transaction_stop_ts,
        t.transaction_ingested_ts,
        t.transaction_stop_reason,
        t.meter_start_wh,
        t.meter_stop_wh,
        t.energy_transferred_kwh,
        
        -- Error details - concatenate error codes from both sources
        array_distinct(
    
    
        
    

    case
        when p.error_codes is null and t.error_codes is null
            then null
        when p.error_codes is null
            then t.error_codes
        when t.error_codes is null
            then p.error_codes
        else list_concat(p.error_codes, t.error_codes)
    end
) as error_codes
        
    from preparing p
    full outer join transactions t
        on p.charge_point_id = t.charge_point_id
        and p.connector_id = t.connector_id
        and p.transaction_id = t.transaction_id
        and t.transaction_ingested_ts > 

    (coalesce(p.previous_ingested_ts, p.preparing_ingested_ts) + cast(-300 as bigint) * interval 1 second)
        and t.transaction_ingested_ts <= 

    (coalesce(p.next_ingested_ts, p.preparing_ingested_ts) + cast(300 as bigint) * interval 1 second)
        
)



select *,
    -- Generate a deterministic unique ID from the composite key
    md5(cast(coalesce(cast(charge_point_id as TEXT), '_dbt_utils_surrogate_key_null_') || '-' || coalesce(cast(connector_id as TEXT), '_dbt_utils_surrogate_key_null_') || '-' || coalesce(cast(charge_attempt_start_ts as TEXT), '_dbt_utils_surrogate_key_null_') as TEXT)) as charge_attempt_id,
    case
        when transaction_id is not null
            and (next_status is null or next_status != 'Faulted')
            and transaction_stop_reason in ('Local', 'Remote', 'EVDisconnected')
            and energy_transferred_kwh is not null and energy_transferred_kwh > 0.1
        then true
        else false
    end as is_successful,
    (select incremental_ts from incremental) as incremental_ts
from 

    attempts_and_transactions
