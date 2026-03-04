<?xml version="1.0" encoding="UTF-8"?>
<tileset version="1.10" tiledversion="1.11.0" name="kilowatt_tiles" tilewidth="64" tileheight="64" tilecount="142" columns="0">
 <grid orientation="orthogonal" width="1" height="1"/>

 <tile id="0" type="Grass">
  <properties>
   <property name="content_type" value="Grass"/>
   <property name="buildable" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_grass.png"/>
 </tile>

 <tile id="1" type="Road">
  <properties>
   <property name="content_type" value="Road"/>
   <property name="locked" type="bool" value="true"/>
   <property name="driveable" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_street_road.png"/>
 </tile>

 <tile id="2" type="Entry">
  <properties>
   <property name="content_type" value="Entry"/>
   <property name="locked" type="bool" value="true"/>
   <property name="driveable" type="bool" value="true"/>
   <property name="is_entry" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_driveway_apron.png"/>
 </tile>

 <tile id="4" type="Lot">
  <properties>
   <property name="content_type" value="Lot"/>
   <property name="driveable" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_asphalt_clean.png"/>
 </tile>

 <tile id="5" type="ParkingBayNorth">
  <properties>
   <property name="content_type" value="ParkingBayNorth"/>
   <property name="driveable" type="bool" value="true"/>
   <property name="is_parking" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_parking_bay_north.png"/>
 </tile>

 <tile id="6" type="ParkingBaySouth">
  <properties>
   <property name="content_type" value="ParkingBaySouth"/>
   <property name="driveable" type="bool" value="true"/>
   <property name="is_parking" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_parking_bay.png"/>
 </tile>

 <tile id="7" type="Concrete">
  <properties>
   <property name="content_type" value="Concrete"/>
   <property name="locked" type="bool" value="true"/>
   <property name="driveable" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_concrete.png"/>
 </tile>

 <tile id="8" type="GarageFloor">
  <properties>
   <property name="content_type" value="GarageFloor"/>
   <property name="locked" type="bool" value="true"/>
   <property name="driveable" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_garage_floor.png"/>
 </tile>

 <tile id="9" type="GaragePillar">
  <properties>
   <property name="content_type" value="GaragePillar"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_garage_pillar.png"/>
 </tile>

 <tile id="10" type="MallFacade">
  <properties>
   <property name="content_type" value="MallFacade"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_mall_facade.png"/>
 </tile>

 <tile id="11" type="StoreWall">
  <properties>
   <property name="content_type" value="StoreWall"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_store_wall.png"/>
 </tile>

 <tile id="12" type="StoreEntrance">
  <properties>
   <property name="content_type" value="StoreEntrance"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_store_entrance.png"/>
 </tile>

 <tile id="13" type="Storefront">
  <properties>
   <property name="content_type" value="Storefront"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_storefront.png"/>
 </tile>

 <tile id="14" type="PumpIsland">
  <properties>
   <property name="content_type" value="PumpIsland"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_pump_island.png"/>
 </tile>

 <tile id="15" type="Canopy">
  <properties>
   <property name="content_type" value="Canopy"/>
   <property name="locked" type="bool" value="true"/>
   <property name="driveable" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_canopy_floor.png"/>
 </tile>

 <tile id="16" type="FuelCap">
  <properties>
   <property name="content_type" value="FuelCap"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_fuel_cap_covered.png"/>
 </tile>

 <tile id="17" type="CanopyShadow">
  <properties>
   <property name="content_type" value="CanopyShadow"/>
   <property name="locked" type="bool" value="true"/>
   <property name="driveable" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_canopy_shadow.png"/>
 </tile>

 <tile id="18" type="BrickSidewalk">
  <properties>
   <property name="content_type" value="BrickSidewalk"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_brick_sidewalk.png"/>
 </tile>

 <tile id="19" type="BikeLane">
  <properties>
   <property name="content_type" value="BikeLane"/>
   <property name="locked" type="bool" value="true"/>
   <property name="driveable" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_bike_lane.png"/>
 </tile>

 <tile id="20" type="StreetRoad">
  <properties>
   <property name="content_type" value="StreetRoad"/>
   <property name="locked" type="bool" value="true"/>
   <property name="driveable" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_street_road.png"/>
 </tile>

 <tile id="21" type="Crosswalk">
  <properties>
   <property name="content_type" value="Crosswalk"/>
   <property name="locked" type="bool" value="true"/>
   <property name="driveable" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_crosswalk.png"/>
 </tile>

 <tile id="22" type="ReservedSpot">
  <properties>
   <property name="content_type" value="ReservedSpot"/>
   <property name="locked" type="bool" value="true"/>
   <property name="driveable" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_reserved_spot.png"/>
 </tile>

 <tile id="23" type="OfficeBackdrop">
  <properties>
   <property name="content_type" value="OfficeBackdrop"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_office_backdrop.png"/>
 </tile>

 <tile id="24" type="PorteCochere">
  <properties>
   <property name="content_type" value="PorteCochere"/>
   <property name="locked" type="bool" value="true"/>
   <property name="driveable" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_porte_cochere.png"/>
 </tile>

 <tile id="25" type="ValetLane">
  <properties>
   <property name="content_type" value="ValetLane"/>
   <property name="locked" type="bool" value="true"/>
   <property name="driveable" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_valet_lane.png"/>
 </tile>

 <tile id="26" type="HotelEntrance">
  <properties>
   <property name="content_type" value="HotelEntrance"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_hotel_entrance.png"/>
 </tile>

 <tile id="27" type="FountainBase">
  <properties>
   <property name="content_type" value="FountainBase"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_fountain_base.png"/>
 </tile>

 <tile id="28" type="GardenBed">
  <properties>
   <property name="content_type" value="GardenBed"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_garden_bed.png"/>
 </tile>

 <tile id="29" type="Cobblestone">
  <properties>
   <property name="content_type" value="Cobblestone"/>
   <property name="locked" type="bool" value="true"/>
   <property name="driveable" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_cobblestone.png"/>
 </tile>

 <tile id="30" type="LoadingZone">
  <properties>
   <property name="content_type" value="LoadingZone"/>
   <property name="locked" type="bool" value="true"/>
   <property name="driveable" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_loading_zone.png"/>
 </tile>

 <tile id="31" type="AsphaltWorn">
  <properties>
   <property name="content_type" value="AsphaltWorn"/>
   <property name="locked" type="bool" value="true"/>
   <property name="driveable" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_asphalt_worn.png"/>
 </tile>

 <tile id="32" type="AsphaltSkid">
  <properties>
   <property name="content_type" value="AsphaltSkid"/>
   <property name="locked" type="bool" value="true"/>
   <property name="driveable" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_asphalt_skid.png"/>
 </tile>

 <tile id="33" type="Planter">
  <properties>
   <property name="content_type" value="Planter"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_planter.png"/>
 </tile>

 <tile id="34" type="CurbAsphaltGrass">
  <properties>
   <property name="content_type" value="CurbAsphaltGrass"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_curb_asphalt_grass.png"/>
 </tile>

 <tile id="35" type="CurbAsphaltConcrete">
  <properties>
   <property name="content_type" value="CurbAsphaltConcrete"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_curb_asphalt_concrete.png"/>
 </tile>

 <tile id="36" type="ChargerPad">
  <properties>
   <property name="content_type" value="ChargerPad"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_charger_pad.png"/>
 </tile>

 <tile id="37" type="TransformerPad">
  <properties>
   <property name="content_type" value="TransformerPad"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_concrete.png"/>
 </tile>

 <tile id="38" type="SolarPad">
  <properties>
   <property name="content_type" value="SolarPad"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_grass.png"/>
 </tile>

 <tile id="39" type="BatteryPad">
  <properties>
   <property name="content_type" value="BatteryPad"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_concrete.png"/>
 </tile>

 <tile id="40" type="Empty">
  <properties>
   <property name="content_type" value="Empty"/>
   <property name="buildable" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_grass.png"/>
 </tile>

 <tile id="41" type="Bollard">
  <properties>
   <property name="content_type" value="Bollard"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_bollard.png"/>
 </tile>

 <tile id="42" type="WheelStop">
  <properties>
   <property name="content_type" value="WheelStop"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_asphalt_lines.png"/>
 </tile>

 <tile id="43" type="StreetTree">
  <properties>
   <property name="content_type" value="StreetTree"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_tree_grate.png"/>
 </tile>

 <tile id="44" type="LightPole">
  <properties>
   <property name="content_type" value="LightPole"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_light_pole.png"/>
 </tile>

 <tile id="45" type="CanopyColumn">
  <properties>
   <property name="content_type" value="CanopyColumn"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_canopy_column.png"/>
 </tile>

 <tile id="46" type="GasStationSign">
  <properties>
   <property name="content_type" value="GasStationSign"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_gas_station_sign.png"/>
 </tile>

 <tile id="47" type="DumpsterPad">
  <properties>
   <property name="content_type" value="DumpsterPad"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_concrete.png"/>
 </tile>

 <tile id="48" type="DumpsterOccupied">
  <properties>
   <property name="content_type" value="DumpsterOccupied"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_dumpster_occupied.png"/>
 </tile>

 <tile id="49" type="TransformerOccupied">
  <properties>
   <property name="content_type" value="TransformerOccupied"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_concrete.png"/>
 </tile>

 <tile id="50" type="SolarOccupied">
  <properties>
   <property name="content_type" value="SolarOccupied"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_grass.png"/>
 </tile>

 <tile id="51" type="BatteryOccupied">
  <properties>
   <property name="content_type" value="BatteryOccupied"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_concrete.png"/>
 </tile>

 <tile id="52" type="AmenityWifiRestrooms">
  <properties>
   <property name="content_type" value="AmenityWifiRestrooms"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_concrete.png"/>
 </tile>

 <tile id="53" type="AmenityLoungeSnacks">
  <properties>
   <property name="content_type" value="AmenityLoungeSnacks"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_concrete.png"/>
 </tile>

 <tile id="54" type="AmenityRestaurant">
  <properties>
   <property name="content_type" value="AmenityRestaurant"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_concrete.png"/>
 </tile>

 <tile id="55" type="AmenityOccupied">
  <properties>
   <property name="content_type" value="AmenityOccupied"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_concrete.png"/>
 </tile>

 <tile id="56" type="RoadYellowLine">
  <properties>
   <property name="content_type" value="RoadYellowLine"/>
   <property name="locked" type="bool" value="true"/>
   <property name="driveable" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_asphalt_lines.png"/>
 </tile>

 <tile id="57" type="AirVacuum">
  <properties>
   <property name="content_type" value="AirVacuum"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_air_vacuum.png"/>
 </tile>

 <tile id="58" type="Bench">
  <properties>
   <property name="content_type" value="Bench"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_bench.png"/>
 </tile>

 <tile id="59" type="CartReturn">
  <properties>
   <property name="content_type" value="CartReturn"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_cart_return.png"/>
 </tile>

 <tile id="60" type="ExitSign">
  <properties>
   <property name="content_type" value="ExitSign"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_exit_sign.png"/>
 </tile>

 <tile id="61" type="FireHydrant">
  <properties>
   <property name="content_type" value="FireHydrant"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_fire_hydrant.png"/>
 </tile>

 <tile id="62" type="GasPumpDisabled">
  <properties>
   <property name="content_type" value="GasPumpDisabled"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_gas_pump_disabled.png"/>
 </tile>

 <tile id="63" type="MallDirectory">
  <properties>
   <property name="content_type" value="MallDirectory"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_mall_directory.png"/>
 </tile>

 <tile id="64" type="NewspaperBox">
  <properties>
   <property name="content_type" value="NewspaperBox"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_newspaper_box.png"/>
 </tile>

 <tile id="65" type="OutdoorHeater">
  <properties>
   <property name="content_type" value="OutdoorHeater"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_outdoor_heater.png"/>
 </tile>

 <tile id="66" type="ParkingMeter">
  <properties>
   <property name="content_type" value="ParkingMeter"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_parking_meter.png"/>
 </tile>

 <tile id="67" type="PlanterUrn">
  <properties>
   <property name="content_type" value="PlanterUrn"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_planter_urn.png"/>
 </tile>

 <tile id="68" type="QuickmartFacade">
  <properties>
   <property name="content_type" value="QuickmartFacade"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_quickmart_facade.png"/>
 </tile>

 <tile id="69" type="ReservedSign">
  <properties>
   <property name="content_type" value="ReservedSign"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_reserved_sign.png"/>
 </tile>

 <tile id="70" type="RopeBarrier">
  <properties>
   <property name="content_type" value="RopeBarrier"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_rope_barrier.png"/>
 </tile>

 <tile id="71" type="SpeedBump">
  <properties>
   <property name="content_type" value="SpeedBump"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_speed_bump.png"/>
 </tile>

 <tile id="72" type="StreetLamp">
  <properties>
   <property name="content_type" value="StreetLamp"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_street_lamp.png"/>
 </tile>

 <tile id="73" type="TrashCan">
  <properties>
   <property name="content_type" value="TrashCan"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_trash_can.png"/>
 </tile>

 <tile id="74" type="UtilityCabinet">
  <properties>
   <property name="content_type" value="UtilityCabinet"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_utility_cabinet.png"/>
 </tile>

 <tile id="75" type="ValetStand">
  <properties>
   <property name="content_type" value="ValetStand"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_valet_stand.png"/>
 </tile>

 <tile id="76" type="BusStop">
  <properties>
   <property name="content_type" value="BusStop"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_bus_stop.png"/>
 </tile>

 <tile id="77" type="ElevatorLobby">
  <properties>
   <property name="content_type" value="ElevatorLobby"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_elevator_lobby.png"/>
 </tile>

 <tile id="78" type="ExecutiveSpot">
  <properties>
   <property name="content_type" value="ExecutiveSpot"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_executive_spot.png"/>
 </tile>

 <tile id="79" type="FireLane">
  <properties>
   <property name="content_type" value="FireLane"/>
   <property name="locked" type="bool" value="true"/>
   <property name="driveable" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_fire_lane.png"/>
 </tile>

 <tile id="80" type="GarageCeiling">
  <properties>
   <property name="content_type" value="GarageCeiling"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_garage_ceiling.png"/>
 </tile>

 <tile id="81" type="GarageLevel1">
  <properties>
   <property name="content_type" value="GarageLevel1"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_garage_level1.png"/>
 </tile>

 <tile id="82" type="GarageRamp">
  <properties>
   <property name="content_type" value="GarageRamp"/>
   <property name="locked" type="bool" value="true"/>
   <property name="driveable" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_garage_ramp.png"/>
 </tile>

 <tile id="83" type="Gutter">
  <properties>
   <property name="content_type" value="Gutter"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_gutter.png"/>
 </tile>

 <tile id="84" type="Manhole">
  <properties>
   <property name="content_type" value="Manhole"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_manhole.png"/>
 </tile>

 <tile id="85" type="MeterZone">
  <properties>
   <property name="content_type" value="MeterZone"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_meter_zone.png"/>
 </tile>

 <tile id="86" type="PathwayStone">
  <properties>
   <property name="content_type" value="PathwayStone"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_pathway_stone.png"/>
 </tile>

 <tile id="87" type="PoolDeck">
  <properties>
   <property name="content_type" value="PoolDeck"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_pool_deck.png"/>
 </tile>

 <tile id="88" type="StreetCorner">
  <properties>
   <property name="content_type" value="StreetCorner"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_street_corner.png"/>
 </tile>

 <tile id="89" type="StreetTreeTile">
  <properties>
   <property name="content_type" value="StreetTreeTile"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_street_tree.png"/>
 </tile>

 <tile id="90" type="UtilityTrench">
  <properties>
   <property name="content_type" value="UtilityTrench"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_utility_trench.png"/>
 </tile>

 <tile id="91" type="WheelStopTile">
  <properties>
   <property name="content_type" value="WheelStopTile"/>
   <property name="locked" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_wheel_stop.png"/>
 </tile>

 <tile id="93" type="RoadLaneBottom">
  <properties>
   <property name="content_type" value="RoadLaneBottom"/>
   <property name="locked" type="bool" value="true"/>
   <property name="driveable" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_road_lane_bottom.png"/>
 </tile>

 <tile id="94" type="RoadLaneTop">
  <properties>
   <property name="content_type" value="RoadLaneTop"/>
   <property name="locked" type="bool" value="true"/>
   <property name="driveable" type="bool" value="true"/>
  </properties>
  <image width="64" height="64" source="../world/tiles/tile_road_lane_top.png"/>
 </tile>

 <tile id="92">
  <image width="64" height="64" source="../fixed/fuel_cover.png"/>
 </tile>

 <tile id="98">
  <image width="64" height="64" source="../fixed/garbage_bin_bot_right.png"/>
 </tile>

 <tile id="99">
  <image width="64" height="64" source="../fixed/garbage_bin_bot_left.png"/>
 </tile>

 <tile id="100">
  <image width="64" height="64" source="../fixed/garbage_bin_top_right.png"/>
 </tile>

 <tile id="101">
  <image width="64" height="64" source="../fixed/garbage_bin_top_left.png"/>
 </tile>

 <tile id="102">
  <image width="64" height="64" source="../fixed/qce_r0c0.png"/>
 </tile>

 <tile id="103">
  <image width="64" height="64" source="../fixed/qce_r0c1.png"/>
 </tile>

 <tile id="104">
  <image width="64" height="64" source="../fixed/qce_r0c2.png"/>
 </tile>

 <tile id="105">
  <image width="64" height="64" source="../fixed/qce_r0c3.png"/>
 </tile>

 <tile id="106">
  <image width="64" height="64" source="../fixed/qce_r1c0.png"/>
 </tile>

 <tile id="107">
  <image width="64" height="64" source="../fixed/qce_r1c1.png"/>
 </tile>

 <tile id="108">
  <image width="64" height="64" source="../fixed/qce_r1c2.png"/>
 </tile>

 <tile id="109">
  <image width="64" height="64" source="../fixed/qce_r1c3.png"/>
 </tile>

 <tile id="110">
  <image width="64" height="64" source="../fixed/qce_r2c0.png"/>
 </tile>

 <tile id="111">
  <image width="64" height="64" source="../fixed/qce_r2c1.png"/>
 </tile>

 <tile id="112">
  <image width="64" height="64" source="../fixed/qce_r2c2.png"/>
 </tile>

 <tile id="113">
  <image width="64" height="64" source="../fixed/qce_r2c3.png"/>
 </tile>

 <tile id="114">
  <image width="64" height="64" source="../fixed/qceg_r0c0.png"/>
 </tile>

 <tile id="115">
  <image width="64" height="64" source="../fixed/qceg_r0c1.png"/>
 </tile>

 <tile id="116">
  <image width="64" height="64" source="../fixed/qceg_r0c2.png"/>
 </tile>

 <tile id="117">
  <image width="64" height="64" source="../fixed/qceg_r0c3.png"/>
 </tile>

 <tile id="118">
  <image width="64" height="64" source="../fixed/qceg_r0c4.png"/>
 </tile>

 <tile id="119">
  <image width="64" height="64" source="../fixed/qceg_r0c5.png"/>
 </tile>

 <tile id="120">
  <image width="64" height="64" source="../fixed/qceg_r0c6.png"/>
 </tile>

 <tile id="121">
  <image width="64" height="64" source="../fixed/qceg_r0c7.png"/>
 </tile>

 <tile id="122">
  <image width="64" height="64" source="../fixed/qceg_r1c0.png"/>
 </tile>

 <tile id="123">
  <image width="64" height="64" source="../fixed/qceg_r1c1.png"/>
 </tile>

 <tile id="124">
  <image width="64" height="64" source="../fixed/qceg_r1c2.png"/>
 </tile>

 <tile id="125">
  <image width="64" height="64" source="../fixed/qceg_r1c3.png"/>
 </tile>

 <tile id="126">
  <image width="64" height="64" source="../fixed/qceg_r1c4.png"/>
 </tile>

 <tile id="127">
  <image width="64" height="64" source="../fixed/qceg_r1c5.png"/>
 </tile>

 <tile id="128">
  <image width="64" height="64" source="../fixed/qceg_r1c6.png"/>
 </tile>

 <tile id="129">
  <image width="64" height="64" source="../fixed/qceg_r1c7.png"/>
 </tile>

 <tile id="130">
  <image width="64" height="64" source="../fixed/qceg_r2c0.png"/>
 </tile>

 <tile id="131">
  <image width="64" height="64" source="../fixed/qceg_r2c1.png"/>
 </tile>

 <tile id="132">
  <image width="64" height="64" source="../fixed/qceg_r2c2.png"/>
 </tile>

 <tile id="133">
  <image width="64" height="64" source="../fixed/qceg_r2c3.png"/>
 </tile>

 <tile id="134">
  <image width="64" height="64" source="../fixed/qceg_r2c4.png"/>
 </tile>

 <tile id="135">
  <image width="64" height="64" source="../fixed/qceg_r2c5.png"/>
 </tile>

 <tile id="136">
  <image width="64" height="64" source="../fixed/qceg_r2c6.png"/>
 </tile>

 <tile id="137">
  <image width="64" height="64" source="../fixed/qceg_r2c7.png"/>
 </tile>

 <tile id="138">
  <image width="64" height="64" source="../fixed/qceg_r3c0.png"/>
 </tile>

 <tile id="139">
  <image width="64" height="64" source="../fixed/qceg_r3c1.png"/>
 </tile>

 <tile id="140">
  <image width="64" height="64" source="../fixed/qceg_r3c2.png"/>
 </tile>

 <tile id="141">
  <image width="64" height="64" source="../fixed/qceg_r3c3.png"/>
 </tile>

 <tile id="142">
  <image width="64" height="64" source="../fixed/qceg_r3c4.png"/>
 </tile>

 <tile id="143">
  <image width="64" height="64" source="../fixed/qceg_r3c5.png"/>
 </tile>

 <tile id="144">
  <image width="64" height="64" source="../fixed/qceg_r3c6.png"/>
 </tile>

 <tile id="145">
  <image width="64" height="64" source="../fixed/qceg_r3c7.png"/>
 </tile>

</tileset>