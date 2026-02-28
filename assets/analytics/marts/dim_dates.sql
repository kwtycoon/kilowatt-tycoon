

with date_spine as (
    





with rawdata as (

    

    

    with p as (
        select 0 as generated_number union all select 1
    ), unioned as (

    select

    
    p0.generated_number * power(2, 0)
     + 
    
    p1.generated_number * power(2, 1)
     + 
    
    p2.generated_number * power(2, 2)
     + 
    
    p3.generated_number * power(2, 3)
     + 
    
    p4.generated_number * power(2, 4)
     + 
    
    p5.generated_number * power(2, 5)
     + 
    
    p6.generated_number * power(2, 6)
     + 
    
    p7.generated_number * power(2, 7)
     + 
    
    p8.generated_number * power(2, 8)
     + 
    
    p9.generated_number * power(2, 9)
     + 
    
    p10.generated_number * power(2, 10)
     + 
    
    p11.generated_number * power(2, 11)
     + 
    
    p12.generated_number * power(2, 12)
     + 
    
    p13.generated_number * power(2, 13)
    
    
    + 1
    as generated_number

    from

    
    p as p0
     cross join 
    
    p as p1
     cross join 
    
    p as p2
     cross join 
    
    p as p3
     cross join 
    
    p as p4
     cross join 
    
    p as p5
     cross join 
    
    p as p6
     cross join 
    
    p as p7
     cross join 
    
    p as p8
     cross join 
    
    p as p9
     cross join 
    
    p as p10
     cross join 
    
    p as p11
     cross join 
    
    p as p12
     cross join 
    
    p as p13
    
    

    )

    select *
    from unioned
    where generated_number <= 11322
    order by generated_number



),

all_periods as (

    select (
        

    (cast('2020-01-01' as date) + cast(row_number() over (order by 1) - 1 as bigint) * interval 1 day)
    ) as date_day
    from rawdata

),

filtered as (

    select *
    from all_periods
    where date_day <= cast('2050-12-31' as date)

)

select * from filtered


)

select
    date_day as date_id,
    date_day,
    extract(year from date_day) as year,
    extract(month from date_day) as month,
    extract(day from date_day) as day,
    extract(dayofweek from date_day) as day_of_week,
    extract(quarter from date_day) as quarter,
    date_trunc('week', date_day) as week_start_date,
    date_trunc('month', date_day) as month_start_date,
    date_trunc('quarter', date_day) as quarter_start_date,
    date_trunc('year', date_day) as year_start_date
from date_spine