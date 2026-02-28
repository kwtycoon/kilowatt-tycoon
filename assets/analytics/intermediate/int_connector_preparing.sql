




    with incremental_date_range as (
        select
            from_timestamp,
            

    (from_timestamp + cast(-30 as bigint) * interval 1 minute) as buffer_from_timestamp,
            least(
                

    (from_timestamp + cast(3 as bigint) * interval 1 month),
                (select max(incremental_ts) from "memory"."main"."int_status_changes"),
                (select max(ingested_timestamp) from "memory"."main"."stg_ocpp_logs")
            ) as to_timestamp
        from
            (
                select (select coalesce(min(ingested_timestamp), TIMESTAMP '2025-10-01') from "memory"."main"."stg_ocpp_logs") as from_timestamp
            )
    ),


-- Get status changes from the dedicated status changes model
status_changes_to_preparing as (
    select
        -- Request details
        charge_point_id,
        connector_id,
        unique_id,
        ingested_ts,
        payload_ts,
        status,
        previous_status,
        previous_ingested_ts,
        previous_payload_ts,
        next_status,
        next_ingested_ts,
        next_payload_ts,
        error_code,
        incremental_ts,
        
        -- Confirmation details
        confirmation_ingested_ts
    from "memory"."main"."int_status_changes"
    -- equal as we want to grab statuses updated when last status changes ran (and later, so greater or equal)
    where ingested_ts >= (select buffer_from_timestamp from incremental_date_range)
        and ingested_ts <= (select to_timestamp from incremental_date_range)
        and status = 'Preparing'
),

ocpp_logs as (
    select
        charge_point_id,
        action,
        ingested_timestamp as ingested_ts,
        message_type_id,
        payload,
        unique_id
    from "memory"."main"."stg_ocpp_logs"
    where ingested_timestamp >= (select buffer_from_timestamp from incremental_date_range)
        and ingested_timestamp <= (select to_timestamp from incremental_date_range)
),

incremental as (
    select
        max(ingested_ts) as incremental_ts
    from status_changes_to_preparing
),


-- Filter for charge attempt actions first
charge_attempt_events as (
    select *
    from ocpp_logs
    where action in ('Authorize', 'StartTransaction', 'StopTransaction', 'StatusNotification', 'RemoteStartTransaction', 'RemoteStopTransaction')
        and message_type_id = 2
),

charge_attempt_events_conf as (
    select req.*,
        conf.payload as conf_payload,
        
    case 
        when req.action in ('StatusNotification', 'StartTransaction', 'MeterValues', 'RemoteStartTransaction')
            then cast(

  json_extract_string(req.payload, '$.connectorId')
 
 as TEXT)
        else null
    end
 as connector_id,
        
    case 
        when req.action in ('StopTransaction', 'RemoteStopTransaction', 'MeterValues') 
            then cast(

  json_extract_string(req.payload, '$.transactionId')
 
 as TEXT)
        when req.action = 'StartTransaction'
            then cast(

  json_extract_string(conf.payload, '$.transactionId')
 
 as TEXT)
        else null
    end
 as transaction_id
    from charge_attempt_events req
    left join ocpp_logs conf on req.unique_id = conf.unique_id
        and conf.message_type_id = 3
        and conf.ingested_ts >= req.ingested_ts
        and conf.ingested_ts <= 

    (req.ingested_ts + cast(45 as bigint) * interval 1 second)

),

preparing_events_chaining as (
    select
        -- Status change details
        p.charge_point_id,
        p.connector_id,
        p.unique_id,
        p.ingested_ts,
        p.previous_status,
        p.status,
        p.next_status,
        p.confirmation_ingested_ts,
        p.previous_ingested_ts,
        p.next_ingested_ts,
        p.previous_payload_ts,
        p.next_payload_ts,
        p.payload_ts,
        
        -- Charge attempt event details
        e.action,
        e.payload,
        e.conf_payload
    from status_changes_to_preparing p
    left join charge_attempt_events_conf e on p.charge_point_id = e.charge_point_id
        and p.connector_id = e.connector_id
        and e.ingested_ts > coalesce(p.previous_ingested_ts, p.ingested_ts)
        and e.ingested_ts <= coalesce(p.next_ingested_ts, p.ingested_ts)

),

-- Extract relevant details based on action type
preparing_details as (
    select
        p.charge_point_id,
        p.connector_id,
        p.unique_id,
        p.ingested_ts,
        p.previous_status,
        p.status,
        p.next_status,
        p.confirmation_ingested_ts,
        p.previous_ingested_ts,
        p.next_ingested_ts,
        p.previous_payload_ts,
        p.next_payload_ts,
        p.payload_ts,
        
        
    case 
        when action in ('StartTransaction', 'RemoteStartTransaction') 
            then cast(

  json_extract_string(payload, '$.idTag')
 
 as TEXT)
        else null
    end
 as id_tag,
        
    case
        when action in ('StartTransaction', 'Authorize') 
            then cast(

  json_extract_string(conf_payload, '$.idTagInfo.status')
 
 as TEXT)
        else null
    end
 as id_tag_status,
        
    case 
        when action = 'Authorize'
            then cast(

  json_extract_string(conf_payload, '$.idTagInfo.idTag')
 
 as TEXT)
        else null
    end
 as parent_id_tag,
        -- Transaction details
        
    case 
        when action in ('StopTransaction', 'RemoteStopTransaction', 'MeterValues') 
            then cast(

  json_extract_string(payload, '$.transactionId')
 
 as TEXT)
        when action = 'StartTransaction'
            then cast(

  json_extract_string(conf_payload, '$.transactionId')
 
 as TEXT)
        else null
    end
 as transaction_id,

        -- Error details
        
    case
        when action = 'StatusNotification'
            then cast(

  json_extract_string(payload, '$.errorCode')
 
 as TEXT)
        else null
    end
 as error_code
    from preparing_events_chaining p
),


-- Group by status change details and aggregate into arrays
preparing_agg as (
    select
        -- Status change details (grouping keys)
        charge_point_id,
        connector_id,
        unique_id,
        ingested_ts,
        previous_status,
        status,
        next_status,
        confirmation_ingested_ts,
        previous_ingested_ts,
        next_ingested_ts,
        previous_payload_ts,
        next_payload_ts,
        payload_ts,
        -- Aggregate extracted details into arrays
        array_distinct(
    array_agg(id_tag)
) as id_tags,
        array_distinct(
    array_agg(id_tag_status)
) as id_tag_statuses,
        array_distinct(
    array_agg(parent_id_tag)
) as parent_id_tags,
        array_distinct(
    array_agg(transaction_id)
) as transaction_ids,
        array_distinct(
    array_agg(error_code)
) as error_codes
                
    from preparing_details
    group by 
        charge_point_id,
        connector_id,
        unique_id,
        ingested_ts,        
        payload_ts,
        previous_status,
        status,
        next_status,
        confirmation_ingested_ts,
        previous_ingested_ts,
        next_ingested_ts,
        previous_payload_ts,
        next_payload_ts
    )



select *,
    case 
        when transaction_ids is not null  and 
    
        len(transaction_ids)
    
 > 0
            then transaction_ids[1]
        else null
    end as transaction_id,
    (select incremental_ts from incremental) as incremental_ts,

    -- Count aggregations for testing
    case 
        when transaction_ids is not null 
            then 
    
        len(transaction_ids)
    

        else 0
    end as _unique_transaction_count

from 

    preparing_agg
