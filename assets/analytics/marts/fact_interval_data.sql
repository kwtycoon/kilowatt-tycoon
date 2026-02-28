


    with incremental_date_range as (
        select
            from_timestamp,
            

    (from_timestamp + cast(-30 as bigint) * interval 1 minute) as buffer_from_timestamp,
            least(
                

    (from_timestamp + cast(3 as bigint) * interval 1 month),
                (select max(incremental_ts) from "memory"."main"."int_meter_values")
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

    meter_values as (
        select
            charge_point_id,
            transaction_id,
            ingested_ts,
            connector_id,
            measurand,
            unit,
            phase,
            

    (date_trunc('minute', first_measurement_ts) + cast(-(minute(first_measurement_ts) % 15) as bigint) * interval 1 minute) as first_interval,
            

    (date_trunc('minute', last_measurement_ts) + cast(-(minute(last_measurement_ts) % 15) as bigint) * interval 1 minute) as last_interval,
            first_measurement_ts,
            last_measurement_ts
        from "memory"."main"."int_meter_values"
    ),

    incremental as (
        select
            max(ingested_ts) as incremental_ts
        from ocpp_logs
    ),

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

    meter_value_records as (
        select
            charge_point_id,
            transaction_id,
            connector_id,
            -- Extract timestamp from the meter value object
            cast(

  json_extract_string(mv.value, '$.timestamp')
 
 as timestamp) as meter_timestamp,
            -- Keep the full meter value object for now
            

  json_extract_string(mv.value, '$.sampledValue')
 
 as sample_values
        from meter_value_logs
        
    
    
        cross join unnest(meter_values::JSON[]) as mv(value)
        where meter_values is not null
            and mv.value is not null
    ),

    sample_values as (
        select
            charge_point_id,
            transaction_id,
            connector_id,
            meter_timestamp,
            mv.value as sample_values
        from meter_value_records
        
    
    
        cross join unnest(sample_values::JSON[]) as mv(value)
    ),

    measurements as (
        select
            charge_point_id,
            transaction_id,
            connector_id,
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

    measurements_with_context as (
        select
            m.charge_point_id,
            m.connector_id,
            m.transaction_id,
            mv.ingested_ts,
            mv.first_interval,
            mv.last_interval,
            mv.first_measurement_ts,
            mv.last_measurement_ts,
            m.meter_timestamp,
            m.meter_15min_interval_start,
            m.measurand,
            m.unit,
            m.phase,
            m.value
        from measurements m
        left join meter_values mv on m.charge_point_id = mv.charge_point_id
            and m.connector_id = mv.connector_id
            and m.transaction_id = mv.transaction_id
            and m.measurand = mv.measurand
            and m.unit = mv.unit
            and ((m.phase is null and mv.phase is null) or m.phase = mv.phase)
            and m.meter_timestamp >= mv.first_measurement_ts
            and m.meter_timestamp <= mv.last_measurement_ts
    ),

    intervals_15min as (
            select
            charge_point_id,
            transaction_id,
            connector_id,
            ingested_ts,
            -- Rebates reporting requires 15-minute interval data (e.g., 10:00, 10:15, 10:30).
            -- The first and last intervals correspond to when energy transfer starts and stops.
            case 
                when meter_15min_interval_start = first_interval then first_measurement_ts
                else meter_15min_interval_start 
            end as meter_15min_interval_start,
            case 
                when meter_15min_interval_start = last_interval then last_measurement_ts 
                else 

    (meter_15min_interval_start + cast(15 as bigint) * interval 1 minute)
            end as meter_15min_interval_stop,            
            measurand,
            unit,
            phase,
            value
        from measurements_with_context
        where value is not null and value != ''
    ),

    agg_15min as (
        select
            charge_point_id,
            transaction_id,
            connector_id,
            ingested_ts,
            meter_15min_interval_start,
            meter_15min_interval_stop,
            measurand,
            unit,
            phase,
            
            -- interval avg value
            avg(cast(value as float)) as avg_value,
            count(*) as _count
        from intervals_15min
        group by
            charge_point_id,
            transaction_id,
            connector_id,
            ingested_ts,
            meter_15min_interval_start,
            meter_15min_interval_stop,
            measurand,
            unit,
            phase
    ),

    final as (
    

        select
            *
        from agg_15min
    
    )

    select
        charge_point_id,
        transaction_id,
        ingested_ts,
        connector_id,
        measurand,
        unit,
        phase,
        meter_15min_interval_start,
        meter_15min_interval_stop,
        avg_value,
        _count,
        -- Generate a deterministic unique ID from the composite key
        md5(cast(coalesce(cast(charge_point_id as TEXT), '_dbt_utils_surrogate_key_null_') || '-' || coalesce(cast(transaction_id as TEXT), '_dbt_utils_surrogate_key_null_') || '-' || coalesce(cast(ingested_ts as TEXT), '_dbt_utils_surrogate_key_null_') || '-' || coalesce(cast(connector_id as TEXT), '_dbt_utils_surrogate_key_null_') || '-' || coalesce(cast(measurand as TEXT), '_dbt_utils_surrogate_key_null_') || '-' || coalesce(cast(unit as TEXT), '_dbt_utils_surrogate_key_null_') || '-' || coalesce(cast(phase as TEXT), '_dbt_utils_surrogate_key_null_') || '-' || coalesce(cast(meter_15min_interval_start as TEXT), '_dbt_utils_surrogate_key_null_') as TEXT)) as interval_data_id,
        (select incremental_ts from incremental) as incremental_ts
    from final