

with source as (
    select * from "RAW"."SEED"."ocpp_1_6_synthetic_logs_14d"
),

renamed as (
    select
        -- Convert timestamp to proper timestamp type using dbt macro for cross-platform compatibility
        cast(timestamp as timestamp) as ingested_timestamp,
        
        -- Charge point identifier
        cast(id as TEXT) as charge_point_id,
        
        -- OCPP action type (can be null for response messages)
        cast(action as TEXT) as action,
                
        -- Extract MessageTypeId from the JSON array for easier filtering
        -- Using fivetran_utils json_extract macro
        cast(

  json_extract_string(msg, '$[0]')
 
 as TEXT) as message_type_id,
        
        -- Extract MessageId from the JSON array
        cast(

  json_extract_string(msg, '$[1]')
 
 as TEXT) as unique_id,
        
        -- Extract Payload from the JSON array
        case 
            when 

  json_extract_string(msg, '$[0]')
 
 = '2' 
            then 

  json_extract_string(msg, '$[3]')
 

            when 

  json_extract_string(msg, '$[0]')
 
 = '3' 
            then 

  json_extract_string(msg, '$[2]')
 

            else null
        end as payload

    from source
)

select * from renamed