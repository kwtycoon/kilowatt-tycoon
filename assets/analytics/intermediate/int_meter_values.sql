


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
            ingested_timestamp as ingested_ts,
            message_type_id,
            payload
        from "memory"."main"."stg_ocpp_logs"
        where ingested_timestamp > (select from_timestamp from incremental_date_range)
            and ingested_timestamp <= (select to_timestamp from incremental_date_range)
    ),

    transactions as (
        select
            charge_point_id,
            transaction_id,
            connector_id,
            ingested_ts,
            last_ingested_ts
        from "memory"."main"."int_transactions"
    ),

    incremental as (
        select
            max(ingested_ts) as incremental_ts
        from ocpp_logs
    ),

    -- Example:
    -- [
    --     {
    --         sampledValue:
    --         [
    --             {measurand:Energy.Active.Import.Register,unit:Wh,value:2300270.0},
    --             {measurand:Voltage,phase:L1,unit:V,value:211.6},
    --             {measurand:Current.Import,phase:L1,unit:A,value:0.41},
    --             {measurand:Power.Offered,unit:W,value:1},
    --             {measurand:Power.Active.Import,unit:W,value:1}
    --         ],
    --         timestamp:2025-10-03T18:02:01.700Z
    --     }
    -- ]

    meter_value_logs as (
        select
            ingested_ts,
            charge_point_id,
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
        when action in ('StopTransaction', 'RemoteStopTransaction', 'MeterValues') 
            then cast(

  json_extract_string(payload, '$.transactionId')
 
 as TEXT)
        when action = 'StartTransaction'
            then cast(

  json_extract_string(null, '$.transactionId')
 
 as TEXT)
        else null
    end
 as transaction_id,
            
    case
        when action = 'MeterValues'
            then
                
                    (payload::JSON -> 'meterValue')
                
        else null
    end
 as meter_values
        from ocpp_logs
        where action = 'MeterValues'
            and message_type_id = 2
    ),

    meter_value_messages as (
        select
            l.charge_point_id,
            t.ingested_ts,
            l.connector_id,
            l.transaction_id,
            l.meter_values
        from meter_value_logs l
        left join transactions t on l.charge_point_id = t.charge_point_id
            and l.connector_id = t.connector_id
            and l.transaction_id = t.transaction_id
            and l.ingested_ts >= t.ingested_ts
            and l.ingested_ts <= t.last_ingested_ts
    ),

    meter_values as (
        select
            charge_point_id,
            transaction_id,
            connector_id,
            ingested_ts,
            -- Extract timestamp from the meter value object
            cast(

  json_extract_string(mv.value, '$.timestamp')
 
 as timestamp) as meter_timestamp,
            -- Keep the full meter value object for now
            

  json_extract_string(mv.value, '$.sampledValue')
 
 as sample_values
        from meter_value_messages
        
    
    
        cross join unnest(meter_values::JSON[]) as mv(value)
        where meter_values is not null
            and mv.value is not null
    ),

    sample_values as (
        select
            charge_point_id,
            transaction_id,
            connector_id,
            ingested_ts,
            meter_timestamp,
            mv.value as sample_values
        from meter_values
        
    
    
        cross join unnest(sample_values::JSON[]) as mv(value)
    ),

    measurements as (
        select
            charge_point_id,
            transaction_id,
            connector_id,
            ingested_ts,
            meter_timestamp,
            

    (date_trunc('minute', meter_timestamp) + cast(-(minute(meter_timestamp) % 15) as bigint) * interval 1 minute) as meter_15min_interval_start,
            replace( 

  json_extract_string(sample_values, '$.measurand')
 
, '"', '') as measurand,
replace( 

  json_extract_string(sample_values, '$.value')
 
, '"', '') as value,
replace( 

  json_extract_string(sample_values, '$.unit')
 
, '"', '') as unit,
replace( 

  json_extract_string(sample_values, '$.phase')
 
, '"', '') as phase

        from sample_values
    ),

    agg_transaction as (
        select
            charge_point_id,
            transaction_id,
            connector_id,
            ingested_ts,
            measurand,
            unit,
            phase,
            -- Keep the first and last timestamps for context
            min(meter_timestamp) as first_measurement_ts,
            max(meter_timestamp) as last_measurement_ts,
            -- Aggregated values
            min(cast(value as float)) as min_value,
            max(cast(value as float)) as max_value,
            avg(cast(value as float)) as avg_value,

            count(*) as _count
        from measurements
        where value is not null and value != ''
        group by
            charge_point_id,
            transaction_id,
            connector_id,
            ingested_ts,
            measurand,
            unit,
            phase
    ),

    final as (
    

        select
            *
        from agg_transaction
    
    )

    select
        charge_point_id,
        transaction_id,
        ingested_ts,
        connector_id,
        measurand,
        unit,
        phase,
        first_measurement_ts,
        last_measurement_ts,
        min_value,
        max_value,
        avg_value,
        _count,
        (select incremental_ts from incremental) as incremental_ts
    from final