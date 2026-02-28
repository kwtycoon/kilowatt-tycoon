


    with incremental_date_range as (
        select
            from_timestamp,
            

    (from_timestamp + cast(-30 as bigint) * interval 1 minute) as buffer_from_timestamp,
            least(
                

    (from_timestamp + cast(3 as bigint) * interval 1 month),
                (select max(incremental_ts) from "memory"."main"."fact_charge_attempts")
            ) as to_timestamp
        from
            (
                select (select coalesce(min(ingested_timestamp), TIMESTAMP '2025-10-01') from "memory"."main"."stg_ocpp_logs") as from_timestamp
            )
    ),


-- Get charge attempts with location information
charge_attempts_with_location as (
    select
        att.charge_attempt_id,
        att.charge_point_id,
        p.location_id,
        p.port_id,
        att.connector_id,
        att.charge_attempt_start_ts,
        att.charge_attempt_stop_ts,
        att.id_tags,
        att.id_tag_statuses,
        att.energy_transferred_kwh,
        att.is_successful,
        att.preparing_ingested_ts,
        -- Extract first idTag from array (or null if empty)
        case 
            when att.id_tags is not null and 
    
        len(att.id_tags)
    
 > 0
            then att.id_tags[0]
            else null
        end as id_tag
    from "memory"."main"."fact_charge_attempts" att
    inner join "memory"."main"."stg_ports" p
        on att.charge_point_id = p.charge_point_id
        and att.connector_id = p.connector_id
    where att.incremental_ts > (select from_timestamp from incremental_date_range)
        and att.incremental_ts <= (select to_timestamp from incremental_date_range)
),

incremental as (
    select
        max(preparing_ingested_ts) as incremental_ts
    from charge_attempts_with_location
),

-- Step 1: Infer idTag for unauthorised attempts if there is authorised right after on the same port
unauthorised_attempts_chaining as (
    select
        att.*,
        -- Find the previous attempt at the same charge point and port
        lag(charge_attempt_stop_ts) over (
            partition by charge_point_id, port_id
            order by charge_attempt_start_ts
        ) as prev_attempt_stop_ts,
        lag(charge_attempt_start_ts) over (
            partition by charge_point_id, port_id
            order by charge_attempt_start_ts
        ) as prev_attempt_start_ts,
        lag(id_tag) over (
            partition by charge_point_id, port_id
            order by charge_attempt_start_ts
        ) as prev_id_tag,
        -- Find the next attempt at the same charge point and port
        lead(charge_attempt_start_ts) over (
            partition by charge_point_id, port_id
            order by charge_attempt_start_ts
        ) as next_attempt_start_ts,
        lead(id_tag) over (
            partition by charge_point_id, port_id
            order by charge_attempt_start_ts
        ) as next_id_tag
    from charge_attempts_with_location att
),

unauthorised_attempts_lag_lead as (
    select
        *,
        case
            -- Start of new group: no previous attempt OR gap from prev stop to current start exceeds 2 minutes OR different idTags
            when prev_attempt_stop_ts is null 
                or 
        (date_diff('minute', prev_attempt_stop_ts::timestamp, charge_attempt_start_ts::timestamp ))
     > 2
                or (id_tag is not null and prev_id_tag is not null and id_tag != prev_id_tag)
            then True
            else False
        end as is_step1_group_start,
        case
            -- End of new group: no next attempt OR gap from current stop to next start exceeds 2 minutes OR different idTags
            when next_attempt_start_ts is null
                or 
        (date_diff('minute', charge_attempt_stop_ts::timestamp, next_attempt_start_ts::timestamp ))
     > 2
                or (id_tag is not null and next_id_tag is not null and id_tag != next_id_tag)
            then True
            else False
        end as is_step1_group_end
    from unauthorised_attempts_chaining
),

step1_group_boundaries as (
    select
        charge_point_id,
        port_id,
        charge_attempt_start_ts as step1_group_start_ts,
        lead(charge_attempt_start_ts) over (
            partition by charge_point_id, port_id
            order by charge_attempt_start_ts
        ) as step1_group_end_ts
    from unauthorised_attempts_lag_lead
    where is_step1_group_start = True
),

-- Assign attempts to unauthorised groups and assign idTag if any attempt in group has one
attempts_with_inferred_id_tags as (
    select
        att.charge_attempt_id,
        att.charge_point_id,
        att.port_id,
        att.connector_id,
        att.charge_attempt_start_ts,
        att.charge_attempt_stop_ts,
        att.id_tags,
        att.id_tag_statuses,
        att.energy_transferred_kwh,
        att.location_id,
        att.is_successful,
        b.step1_group_start_ts,
        -- Assign idTag to whole group if any attempt in the group has an idTag
        max(att.id_tag) over (
            partition by att.charge_point_id, att.port_id, b.step1_group_start_ts
        ) as id_tag
    from step1_group_boundaries b
    inner join charge_attempts_with_location att
        on att.charge_point_id = b.charge_point_id
        and att.port_id = b.port_id
        and att.charge_attempt_start_ts >= b.step1_group_start_ts
        and (b.step1_group_end_ts is null or att.charge_attempt_start_ts < b.step1_group_end_ts)
),


-- Step 2: Group attempts by location_id + idTag, 30 min apart (if idTag exists), or by location_id + charge_point_id + port_id, 2 min apart (if no idTag)
attempts_with_grouping_strategies as (
    select
        att.*,
        -- Create grouping key: location_id + idTag (if idTag exists), otherwise location_id + charge_point_id + port_id
        case 
            when att.id_tag is not null 
            then att.location_id || '_' || att.id_tag
            else att.location_id || '_' || att.charge_point_id || '_' || att.port_id
        end as grouping_key,
        -- Determine time window: 30 minutes for authenticated visits, 2 minutes for unauthenticated visits
        case 
            when att.id_tag is not null then 30
            else 2
        end as time_window_minutes
    from attempts_with_inferred_id_tags att
),

attempts_chaining as (
    select
        att.*,
        -- Find the previous attempt's stop time in the same group
        lag(att.charge_attempt_stop_ts) over (
            partition by att.grouping_key
            order by att.charge_attempt_start_ts
        ) as prev_attempt_stop_ts,
        -- Find the next attempt's start time in the same group
        lead(att.charge_attempt_start_ts) over (
            partition by att.grouping_key
            order by att.charge_attempt_start_ts
        ) as next_attempt_start_ts
    from attempts_with_grouping_strategies att
),

attempts_lag_lead as (
    select
        *,
        case
            -- Start of visit: no previous attempt OR gap from prev stop to current start exceeds time window
            when prev_attempt_stop_ts is null 
                or 
        (date_diff('minute', prev_attempt_stop_ts::timestamp, charge_attempt_start_ts::timestamp ))
     > time_window_minutes
            then True
            else False
        end as is_visit_start,
        case
            -- End of visit: no next attempt OR gap from current stop to next start exceeds time window
            when next_attempt_start_ts is null
                or 
        (date_diff('minute', charge_attempt_stop_ts::timestamp, next_attempt_start_ts::timestamp ))
     > time_window_minutes
            then True
            else False
        end as is_visit_end
    from attempts_chaining
),

visit_boundaries as (
    select
        grouping_key,
        time_window_minutes,
        location_id,
        id_tag,
        charge_attempt_start_ts as visit_start_ts,
        lead(charge_attempt_start_ts) over (
            partition by grouping_key
            order by charge_attempt_start_ts
        ) as next_visit_start_ts
    from attempts_lag_lead
    where is_visit_start = True
),

attempts_grouping as (
    select
        att.charge_attempt_id,
        att.charge_point_id,
        att.port_id,
        att.connector_id,
        att.charge_attempt_start_ts,
        att.charge_attempt_stop_ts,
        att.location_id,
        att.id_tag,
        att.id_tag_statuses,
        att.energy_transferred_kwh,
        att.is_successful,
        b.visit_start_ts,
        att.grouping_key,
        att.time_window_minutes,
        -- Mark if this is the first attempt in the visit
        visit_start_ts = charge_attempt_start_ts as is_first_attempt,
        -- Mark if this is the last attempt in the visit
        row_number() over (
            partition by b.grouping_key, b.visit_start_ts
            order by att.charge_attempt_start_ts desc
        ) = 1 as is_last_attempt
    from attempts_with_grouping_strategies att
    inner join visit_boundaries b
        on att.grouping_key = b.grouping_key
        and att.charge_attempt_start_ts >= b.visit_start_ts
        and (b.next_visit_start_ts is null or att.charge_attempt_start_ts < b.next_visit_start_ts)
),

new_visits as (
    select
        grouping_key, 
        time_window_minutes,
        visit_start_ts, 
        max(id_tag) as id_tag,
        max(location_id) as location_id,
        max(charge_attempt_stop_ts) as visit_end_ts,
        count(*) as charge_attempt_count,
        array_distinct(
    array_agg(charge_attempt_id)
) as charge_attempt_ids,
        array_distinct(
    array_agg(charge_point_id)
) as charge_point_ids,
        sum(coalesce(energy_transferred_kwh, 0)) as total_energy_transferred_kwh,
        
        (date_diff('minute', min(charge_attempt_start_ts)::timestamp, max(charge_attempt_stop_ts)::timestamp ))
     as visit_duration_minutes,
        max(case when is_last_attempt then is_successful else null end) as is_successful,
        min(case when is_first_attempt then charge_attempt_id else null end) as first_charge_attempt_id,
        max(case when is_last_attempt then charge_attempt_id else null end) as last_charge_attempt_id,
        min(case when is_first_attempt then charge_point_id else null end) as first_charge_point_id,
        max(case when is_last_attempt then charge_point_id else null end) as last_charge_point_id,
        min(case when is_first_attempt then port_id else null end) as first_port_id,
        max(case when is_last_attempt then port_id else null end) as last_port_id
    from attempts_grouping
    group by grouping_key, time_window_minutes, visit_start_ts
),



    visits as (
        select * from new_visits
    )



select
    location_id,
    charge_point_ids,
    id_tag,
    visit_start_ts,
    visit_end_ts,
    charge_attempt_count,
    charge_attempt_ids,
    total_energy_transferred_kwh,
    first_charge_attempt_id,
    last_charge_attempt_id,
    first_charge_point_id,
    last_charge_point_id,
    first_port_id,
    last_port_id,
    is_successful,
    grouping_key,
    
        (date_diff('minute', visit_start_ts::timestamp, visit_end_ts::timestamp ))
     as visit_duration_minutes,
    md5(cast(coalesce(cast(location_id as TEXT), '_dbt_utils_surrogate_key_null_') || '-' || coalesce(cast(first_charge_point_id as TEXT), '_dbt_utils_surrogate_key_null_') || '-' || coalesce(cast(first_port_id as TEXT), '_dbt_utils_surrogate_key_null_') || '-' || coalesce(cast(visit_start_ts as TEXT), '_dbt_utils_surrogate_key_null_') as TEXT)) as visit_id,
    (select incremental_ts from incremental) as incremental_ts
from visits