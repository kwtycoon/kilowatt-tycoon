


    with incremental_date_range as (
        select
            from_timestamp,
            

    (from_timestamp + cast(-30 as bigint) * interval 1 minute) as buffer_from_timestamp,
            

    least(
        (from_timestamp + cast(3 as bigint) * interval 1 month),
        (select max(ingested_timestamp) from "memory"."main"."stg_ocpp_logs")
    ) as to_timestamp
        from
            (
                select
                    greatest(
                        (select coalesce(min(ingested_timestamp), TIMESTAMP '2025-10-01') from "memory"."main"."stg_ocpp_logs"),
                        (select min(ingested_timestamp) from "memory"."main"."stg_ocpp_logs")
                    ) as from_timestamp
            )
    ),


    ocpp_logs as (
        select
            charge_point_id,
            action,
            ingested_timestamp,
            message_type_id,
            payload,
            unique_id
        from "memory"."main"."stg_ocpp_logs"
        where ingested_timestamp > (select from_timestamp from incremental_date_range)
            and ingested_timestamp <= (select to_timestamp from incremental_date_range)
    ),

    incremental as (
        select
            max(ingested_timestamp) as incremental_ts
        from ocpp_logs
    ),

    -- Filter for StatusNotification events
    status_notification_events as (
        select
            ingested_timestamp,
            charge_point_id,
            unique_id,
            action,
            payload,
            
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

  json_extract_string(payload, '$.status')
 
 as TEXT)
        else null
    end
 as status,
            
    case
        when action = 'StatusNotification'
            then cast(

  json_extract_string(payload, '$.errorCode')
 
 as TEXT)
        else null
    end
 as error_code,
            
    case 
        when action in ('StatusNotification', 'StartTransaction', 'StopTransaction')
            then cast(

  json_extract_string(payload, '$.timestamp')
 
 as timestamp)
        else null
    end
 as payload_ts
        from ocpp_logs
        where action = 'StatusNotification'
            and message_type_id = 2
    ),

    -- Join status notifications with their confirmations and ports
    status_with_confirmation as (
        select
            -- Request details
            req.charge_point_id,
            req.connector_id,
            p.port_id,
            req.ingested_timestamp as ingested_ts,
            req.unique_id,
            req.status,
            req.error_code,
            req.payload,
            req.payload_ts,
            
            -- Confirmation details
            conf.ingested_timestamp as confirmation_ingested_ts
            
        from status_notification_events req
        left join "memory"."main"."stg_ports" p
            on req.charge_point_id = p.charge_point_id
            and req.connector_id = p.connector_id
        left join ocpp_logs conf
            on req.unique_id = conf.unique_id
            and conf.message_type_id = 3
            and conf.ingested_timestamp >= req.ingested_timestamp
            and conf.ingested_timestamp <= 

    (req.ingested_timestamp + cast(15 as bigint) * interval 1 second)
    ),


    statuses_with_buffer as (
        select 
            *,
            cast(null as TEXT) as previous_status,
            cast(null as timestamp) as previous_ingested_ts,
            cast(null as timestamp) as previous_payload_ts
        from status_with_confirmation
    ),


    -- Add previous status using window function on combined data
    -- Use coalesce to prefer existing previous_status from buffer over recalculated values
    status_with_lag as (
        select
            charge_point_id,
            connector_id,
            port_id,
            ingested_ts,
            unique_id,
            status,
            error_code,
            payload,
            payload_ts,
            confirmation_ingested_ts,

            coalesce(
                previous_status,
                lag(status) over (
                    partition by charge_point_id, connector_id order by ingested_ts
                )
            ) as previous_status,
            coalesce(
                previous_ingested_ts,
                lag(ingested_ts) over (
                    partition by charge_point_id, connector_id order by ingested_ts
                )
            ) as previous_ingested_ts,
            coalesce(
                previous_payload_ts,
                lag(payload_ts) over (
                    partition by charge_point_id, connector_id order by ingested_ts
                )
            ) as previous_payload_ts
        from statuses_with_buffer
    ),

    change_from_lag as (
        select *
        from status_with_lag
        where previous_status is null or previous_status <> status
    ),

    -- Add next status using window function (will be null for edge cases, updated in next run)
    status_with_lead as (
        select
            *,
            lead(status) over (
                partition by charge_point_id, connector_id order by ingested_ts
            ) as next_status,
            lead(ingested_ts) over (
                partition by charge_point_id, connector_id order by ingested_ts
            ) as next_ingested_ts,
            lead(payload_ts) over (
                partition by charge_point_id, connector_id order by ingested_ts
            ) as next_payload_ts
        from change_from_lag
    )

 select *,
    (select incremental_ts from incremental) as incremental_ts
 from status_with_lead