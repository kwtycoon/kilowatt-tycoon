




    with incremental_date_range as (
        select
            from_timestamp,
            

    least(
        (from_timestamp + cast(3 as bigint) * interval 1 month),
        (select max(ingested_timestamp) from "memory"."main"."stg_ocpp_logs")
    ) as to_timestamp
        from
            (
                select (select coalesce(min(ingested_timestamp), TIMESTAMP '2025-10-01') from "memory"."main"."stg_ocpp_logs") as from_timestamp
            )
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
    where ingested_timestamp > (select from_timestamp from incremental_date_range)
        and ingested_timestamp <= (select to_timestamp from incremental_date_range)
),

incremental as (
    select
        max(ingested_ts) as incremental_ts
    from ocpp_logs
),

-- Filter for charge attempt actions first
transaction_events as (
    select
        charge_point_id,
        action,
        ingested_ts,
        message_type_id,
        payload,
        unique_id,
        
    case 
        when action in ('StatusNotification', 'StartTransaction', 'MeterValues', 'RemoteStartTransaction')
            then cast(

  json_extract_string(payload, '$.connectorId')
 
 as TEXT)
        else null
    end
 as connector_id
    from ocpp_logs
    where action in ('StartTransaction', 'StopTransaction', 'RemoteStartTransaction', 'RemoteStopTransaction', 'MeterValues')
),

transaction_events_conf as (
    select req.*,
        conf.payload as conf_payload
    from transaction_events req
    left join ocpp_logs conf on req.unique_id = conf.unique_id
        and conf.message_type_id = 3
        and conf.ingested_ts >= req.ingested_ts
        and conf.ingested_ts <= 

    (req.ingested_ts + cast(15 as bigint) * interval 1 second)

),

-- Extract relevant details based on action type
transaction_details as (
    select
        -- Charge attempts details
        e.charge_point_id,
        e.connector_id,
        e.ingested_ts,

        
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
        -- Extract details based on action type using reusable macros
        
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
        -- Transaction details
        
    case 
        when action = 'StartTransaction'
            then cast(

  json_extract_string(payload, '$.timestamp')
 
 as timestamp)
        else null
    end
 as transaction_start_ts,
        
    case 
        when action = 'StopTransaction'
            then cast(

  json_extract_string(payload, '$.timestamp')
 
 as timestamp)
        else null
    end
 as transaction_stop_ts,
        
    case 
        when action = 'StopTransaction'
            -- If a transaction is ended in a normal way (e.g. EV-driver presented his identification to stop the transaction), the
            -- Reason element MAY be omitted and the Reason SHOULD be assumed 'Local'.
            then coalesce(cast(

  json_extract_string(payload, '$.reason')
 
 as TEXT), 'Local')
        else null
    end
 as transaction_stop_reason,
        -- Meter details
        
    case 
        when action = 'StartTransaction' 
            then cast(

  json_extract_string(payload, '$.meterStart')
 
 as numeric(28,6))
        else null    
    end
 as meter_start,
        
    case
        when action = 'StopTransaction' 
            then cast(

  json_extract_string(payload, '$.meterStop')
 
 as numeric(28,6))
        else null
    end
 as meter_stop,
        
    case
        when action = 'MeterValues'
            then cast(

  json_extract_string(payload, '$.meterValue')
 
 as numeric(28,6))
        else null
    end
 as meter_value,
    from transaction_events_conf e
),

-- Group by transaction_id and extract transaction-level details
transactions as (
    select
        transaction_id,
        charge_point_id,

        array_distinct(
    array_agg(connector_id)
) as connector_ids,
        
        -- Transaction timing details
        min(ingested_ts) as ingested_ts,
        min(transaction_start_ts) as transaction_start_ts,
        max(transaction_stop_ts) as transaction_stop_ts,
        max(ingested_ts) as last_ingested_ts,
        min(transaction_stop_reason) as transaction_stop_reason,
        
        --Authentication details
        array_distinct(
    array_agg(id_tag)
) as id_tags,
        array_distinct(
    array_agg(id_tag_status)
) as id_tag_statuses,
        
        -- Energy transfer details
        min(meter_start) as meter_start_wh,
        max(meter_stop) as meter_stop_wh
        
    from transaction_details
    where transaction_id is not null
    group by 
        transaction_id,
        charge_point_id
),

status_notifications as (
    select
        charge_point_id,
        ingested_ts,
        
    case 
        when action in ('StatusNotification', 'StartTransaction', 'MeterValues', 'RemoteStartTransaction')
            then cast(

  json_extract_string(payload, '$.connectorId')
 
 as TEXT)
        else null
    end
 as connector_id,
        
    case
        when action = 'StatusNotification'
            then cast(

  json_extract_string(payload, '$.errorCode')
 
 as TEXT)
        else null
    end
 as error_code
    from ocpp_logs
    where action = 'StatusNotification'
        and message_type_id = 2
),

-- Join StatusNotification events that occurred during each transaction
transaction_status_notifications as (
    select
        t.transaction_id,
        t.charge_point_id,
        array_distinct(
    array_agg(sn.error_code)
) as error_codes
    from transactions t
    left join status_notifications sn
        on t.charge_point_id = sn.charge_point_id
        and sn.ingested_ts >= t.transaction_start_ts
        and sn.ingested_ts <= coalesce(t.transaction_stop_ts, t.last_ingested_ts)
        and 
    
    
        list_contains(t.connector_ids, sn.connector_id)
    

    group by 
        t.transaction_id,
        t.charge_point_id
)



select 
    t.*,
    tsn.error_codes,
        
    -- Calculate energy transferred from meterStart and meterStop values
    cast(
        case 
            when t.meter_start_wh is not null and t.meter_stop_wh is not null
            then (t.meter_stop_wh - t.meter_start_wh)/1000.0
            else null
        end as numeric(28,6)
    ) as energy_transferred_kwh,
    case 
        when t.connector_ids is not null and 
    
        len(t.connector_ids)
    
 > 0
            then t.connector_ids[1]
        else null
    end as connector_id,

    -- Count aggregations for testing
    case 
        when t.connector_ids is not null 
            then 
    
        len(t.connector_ids)
    

        else 0
    end as _unique_connectors_count,

    (select incremental_ts from incremental) as incremental_ts

from 

    transactions t

left join transaction_status_notifications tsn
    on t.transaction_id = tsn.transaction_id
    and t.charge_point_id = tsn.charge_point_id