

with source as (
    select * from "RAW"."SEED"."ports"
),

renamed as (
    select
        cast(charge_point_id as TEXT) as charge_point_id,
        cast(location_id as TEXT) as location_id,
        cast(port_id as TEXT) as port_id,
        cast(connector_id as TEXT) as connector_id,
        cast(connector_type as TEXT) as connector_type,
        cast(commissioned_ts as timestamp) as commissioned_ts,
        cast(decommissioned_ts as timestamp) as decommissioned_ts
    from source
)

select * from renamed