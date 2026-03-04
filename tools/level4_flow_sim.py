#!/usr/bin/env python3
"""
Level 4 flow simulator -- stress-test vehicle throughput on a TMX map.

Parses the TMX tile layer + zone objects, builds a passability grid,
places L2 charger stations on every ChargerPad, then runs a discrete-tick
simulation with A*-routed blocking vehicles.

Usage:
    python tools/level4_flow_sim.py                         # defaults
    python tools/level4_flow_sim.py --ticks 3000 --spawn-interval 3
    python tools/level4_flow_sim.py --seeds 5 --json        # multi-seed
    python tools/level4_flow_sim.py --map assets/maps/04_scooter_alley.tmx
"""

from __future__ import annotations

import argparse
import heapq
import json
import math
import os
import random
import sys
import xml.etree.ElementTree as ET
from dataclasses import dataclass, field
from enum import Enum, auto
from pathlib import Path
from typing import Optional

# ---------------------------------------------------------------------------
# Tile content model (mirrors Rust TileContent + is_driveable / is_parking)
# ---------------------------------------------------------------------------

TILE_ID_TO_CONTENT: dict[int, str] = {
    0: "Grass", 1: "Road", 2: "Entry", 3: "Exit", 4: "Lot",
    5: "ParkingBayNorth", 6: "ParkingBaySouth", 7: "Concrete",
    8: "GarageFloor", 9: "GaragePillar", 10: "MallFacade",
    11: "StoreWall", 12: "StoreEntrance", 13: "Storefront",
    14: "PumpIsland", 15: "Canopy", 16: "FuelCap", 17: "CanopyShadow",
    18: "Grass", 19: "Grass", 20: "Road", 21: "Road",
    22: "ReservedSpot", 23: "OfficeBackdrop", 24: "Concrete",
    25: "Road", 26: "Concrete", 27: "Grass", 28: "Grass",
    29: "Concrete", 30: "LoadingZone", 31: "AsphaltWorn",
    32: "AsphaltSkid", 33: "Planter", 34: "Grass", 35: "Grass",
    36: "ChargerPad", 37: "TransformerPad", 38: "SolarPad",
    39: "BatteryPad", 40: "Empty", 41: "Bollard", 42: "WheelStop",
    43: "StreetTree", 44: "LightPole", 45: "CanopyColumn",
    46: "GasStationSign", 47: "DumpsterPad", 48: "DumpsterOccupied",
    49: "TransformerOccupied", 50: "SolarOccupied",
    51: "BatteryOccupied", 52: "AmenityWifiRestrooms",
    53: "AmenityLoungeSnacks", 54: "AmenityRestaurant",
    55: "AmenityOccupied", 56: "Road", 57: "Road",
    93: "Road", 94: "Road",
}

DRIVEABLE = frozenset({
    "Road", "Lot", "Entry", "Exit", "Canopy", "CanopyShadow",
    "AsphaltWorn", "AsphaltSkid", "GarageFloor", "ReservedSpot",
    "LoadingZone", "Concrete",
})

PARKING = frozenset({"ParkingBayNorth", "ParkingBaySouth"})


def content_for_tile_id(tile_id: int) -> str:
    if tile_id in TILE_ID_TO_CONTENT:
        return TILE_ID_TO_CONTENT[tile_id]
    if 58 <= tile_id <= 92:
        return "Grass"
    if 95 <= tile_id <= 149:
        return "Grass"
    return "Grass"


def is_passable(content: str) -> bool:
    return content in DRIVEABLE or content in PARKING


# ---------------------------------------------------------------------------
# TMX / TSX parsing
# ---------------------------------------------------------------------------

Coord = tuple[int, int]

CARDINALS: list[Coord] = [(0, -1), (0, 1), (-1, 0), (1, 0)]


@dataclass
class GridMap:
    width: int
    height: int
    content: list[list[str]]       # [tiled_y][x]
    passable: list[list[bool]]     # [tiled_y][x]
    charger_pads: list[Coord]      # tiled coords
    parking_bays: list[Coord]      # tiled coords
    entries: list[Coord] = field(default_factory=list)
    exit: Coord = (0, 0)


def parse_tmx(tmx_path: str) -> GridMap:
    tree = ET.parse(tmx_path)
    root = tree.getroot()
    width = int(root.attrib["width"])
    height = int(root.attrib["height"])
    firstgid = 1
    for ts in root.findall("tileset"):
        firstgid = int(ts.attrib.get("firstgid", "1"))
        break

    content: list[list[str]] = []
    passable: list[list[bool]] = []
    charger_pads: list[Coord] = []
    parking_bays: list[Coord] = []

    layer = root.find(".//layer[@name='tiles']")
    if layer is None:
        raise ValueError("No 'tiles' layer found in TMX")
    data_elem = layer.find("data")
    if data_elem is None or data_elem.text is None:
        raise ValueError("No CSV data in tiles layer")

    csv_text = data_elem.text.strip()
    rows = csv_text.split("\n")
    for tiled_y, row_str in enumerate(rows):
        cells = [int(v.strip()) for v in row_str.rstrip(",").split(",") if v.strip()]
        row_content: list[str] = []
        row_pass: list[bool] = []
        for x, raw_id in enumerate(cells):
            tile_id = raw_id - firstgid if raw_id > 0 else 0
            c = content_for_tile_id(tile_id)
            row_content.append(c)
            row_pass.append(is_passable(c))
            if c == "ChargerPad":
                charger_pads.append((x, tiled_y))
            if c in PARKING:
                parking_bays.append((x, tiled_y))
        content.append(row_content)
        passable.append(row_pass)

    entries: list[Coord] = []
    exit_pos: Coord = (width - 1, height - 1)
    zones = root.find(".//objectgroup[@name='zones']")
    if zones is not None:
        for obj in zones.findall("object"):
            obj_type = obj.attrib.get("type", "")
            px = float(obj.attrib.get("x", "0"))
            py = float(obj.attrib.get("y", "0"))
            tx = int(px) // 64
            ty = int(py) // 64
            if obj_type == "entry":
                entries.append((tx, ty))
            elif obj_type == "exit":
                exit_pos = (tx, ty)

    if not entries:
        entries = [(0, height - 1)]

    return GridMap(
        width=width, height=height,
        content=content, passable=passable,
        charger_pads=charger_pads, parking_bays=parking_bays,
        entries=entries, exit=exit_pos,
    )


# ---------------------------------------------------------------------------
# Charger station model
# ---------------------------------------------------------------------------

class StationState(Enum):
    FREE = auto()
    ASSIGNED = auto()
    CHARGING = auto()


@dataclass
class ChargerStation:
    pad: Coord           # charger equipment location (not passable)
    bay: Coord           # adjacent parking bay where vehicle parks
    state: StationState = StationState.FREE
    vehicle_id: int = -1


def build_stations(grid: GridMap) -> list[ChargerStation]:
    """For every ChargerPad, find the nearest adjacent ParkingBay to serve as
    the vehicle destination."""
    bay_set = set(grid.parking_bays)
    stations: list[ChargerStation] = []
    for px, py in grid.charger_pads:
        for dx, dy in CARDINALS:
            nx, ny = px + dx, py + dy
            if (nx, ny) in bay_set:
                stations.append(ChargerStation(pad=(px, py), bay=(nx, ny)))
                break
    return stations


# ---------------------------------------------------------------------------
# A* pathfinder with dynamic blocking
# ---------------------------------------------------------------------------

def astar(
    start: Coord,
    goal: Coord,
    grid: GridMap,
    blocked: set[Coord],
    allow_goal_occupied: bool = True,
) -> Optional[list[Coord]]:
    """4-direction A* on the grid. Tiles in `blocked` are impassable except
    optionally the goal itself (a vehicle is allowed to path TO a blocked
    destination because it will claim it)."""
    if start == goal:
        return [start]
    w, h = grid.width, grid.height

    open_set: list[tuple[float, int, Coord]] = []
    counter = 0
    g: dict[Coord, float] = {start: 0.0}
    parent: dict[Coord, Coord] = {}
    closed: set[Coord] = set()

    est = abs(goal[0] - start[0]) + abs(goal[1] - start[1])
    heapq.heappush(open_set, (est, counter, start))

    while open_set:
        _f, _c, cur = heapq.heappop(open_set)
        if cur == goal:
            path = [cur]
            while cur in parent:
                cur = parent[cur]
                path.append(cur)
            path.reverse()
            return path
        if cur in closed:
            continue
        closed.add(cur)
        cur_g = g[cur]
        for dx, dy in CARDINALS:
            nx, ny = cur[0] + dx, cur[1] + dy
            nb = (nx, ny)
            if not (0 <= nx < w and 0 <= ny < h):
                continue
            if not grid.passable[ny][nx]:
                continue
            if nb in blocked and not (allow_goal_occupied and nb == goal):
                continue
            ng = cur_g + 1.0
            if ng < g.get(nb, math.inf):
                g[nb] = ng
                parent[nb] = cur
                est_nb = ng + abs(goal[0] - nx) + abs(goal[1] - ny)
                counter += 1
                heapq.heappush(open_set, (est_nb, counter, nb))
    return None


# ---------------------------------------------------------------------------
# Vehicle lifecycle
# ---------------------------------------------------------------------------

class Phase(Enum):
    ARRIVING = auto()
    PARKED_CHARGING = auto()
    DEPARTING = auto()
    EXITED = auto()
    ABANDONED = auto()


@dataclass
class Vehicle:
    vid: int
    pos: Coord
    phase: Phase
    station_idx: int = -1
    path: list[Coord] = field(default_factory=list)
    path_i: int = 0
    charge_remaining: int = 0
    stuck_ticks: int = 0
    spawn_tick: int = 0
    exit_tick: int = 0
    reroute_cooldown: int = 0

    @property
    def blocking(self) -> bool:
        return self.phase in (Phase.ARRIVING, Phase.PARKED_CHARGING)


# ---------------------------------------------------------------------------
# Simulation
# ---------------------------------------------------------------------------

@dataclass
class SimStats:
    completed: int = 0
    abandoned: int = 0
    spawned: int = 0
    spawn_rejected: int = 0
    total_trip_ticks: int = 0
    reroute_attempts: int = 0
    reroute_failures: int = 0
    max_concurrent: int = 0
    charger_busy_ticks: int = 0
    charger_total_ticks: int = 0
    peak_queue_depth: int = 0
    peak_chargers_busy: int = 0
    peak_stations_in_use: int = 0


class Simulation:
    def __init__(
        self,
        grid: GridMap,
        stations: list[ChargerStation],
        *,
        spawn_interval: int = 4,
        charge_ticks: int = 30,
        max_stuck: int = 40,
        seed: int = 42,
    ):
        self.grid = grid
        self.stations = stations
        self.spawn_interval = spawn_interval
        self.charge_ticks = charge_ticks
        self.max_stuck = max_stuck
        self.rng = random.Random(seed)
        self.tick = 0
        self.next_vid = 0
        self.vehicles: dict[int, Vehicle] = {}
        self.blocked: set[Coord] = set()
        self.stats = SimStats()

    # -- helpers --

    def _blocking_set(self, exclude_vid: int = -1) -> set[Coord]:
        s: set[Coord] = set()
        for v in self.vehicles.values():
            if v.vid != exclude_vid and v.blocking:
                s.add(v.pos)
        return s

    def _free_stations(self) -> list[int]:
        return [i for i, s in enumerate(self.stations) if s.state == StationState.FREE]

    def _entry_clear(self, entry_pos: Coord) -> bool:
        if entry_pos in self.blocked:
            return False
        for dx, dy in CARDINALS:
            nb = (entry_pos[0] + dx, entry_pos[1] + dy)
            if 0 <= nb[0] < self.grid.width and 0 <= nb[1] < self.grid.height:
                if nb in self.blocked:
                    return False
        return True

    # -- lifecycle steps --

    def _spawn(self) -> None:
        if self.tick % self.spawn_interval != 0:
            return
        for entry_pos in self.grid.entries:
            if not self._entry_clear(entry_pos):
                self.stats.spawn_rejected += 1
                continue
            free = self._free_stations()
            if not free:
                self.stats.spawn_rejected += 1
                return
            si = self.rng.choice(free)
            station = self.stations[si]
            station.state = StationState.ASSIGNED
            vid = self.next_vid
            self.next_vid += 1
            v = Vehicle(
                vid=vid, pos=entry_pos, phase=Phase.ARRIVING,
                station_idx=si, spawn_tick=self.tick,
                charge_remaining=self.charge_ticks + self.rng.randint(-5, 10),
            )
            bl = self._blocking_set(exclude_vid=vid)
            path = astar(v.pos, station.bay, self.grid, bl)
            if path is None:
                station.state = StationState.FREE
                self.stats.spawn_rejected += 1
                continue
            v.path = path
            v.path_i = 0
            station.vehicle_id = vid
            self.vehicles[vid] = v
            self.blocked.add(v.pos)
            self.stats.spawned += 1

    def _move_vehicles(self) -> None:
        for v in list(self.vehicles.values()):
            if v.phase == Phase.PARKED_CHARGING:
                continue
            if v.phase in (Phase.EXITED, Phase.ABANDONED):
                continue
            if v.reroute_cooldown > 0:
                v.reroute_cooldown -= 1
                continue
            if v.path_i >= len(v.path) - 1:
                continue

            next_pos = v.path[v.path_i + 1]
            can_move = next_pos not in self.blocked or (
                v.phase == Phase.DEPARTING
            )
            if not can_move:
                v.stuck_ticks += 1
                if v.stuck_ticks >= 3:
                    self.stats.reroute_attempts += 1
                    goal = v.path[-1] if v.path else self.grid.exit
                    bl = self._blocking_set(exclude_vid=v.vid)
                    new_path = astar(v.pos, goal, self.grid, bl)
                    if new_path is not None:
                        v.path = new_path
                        v.path_i = 0
                        v.stuck_ticks = 0
                    else:
                        self.stats.reroute_failures += 1
                        v.reroute_cooldown = 3
                    if v.stuck_ticks > self.max_stuck:
                        self._abandon(v)
                continue

            if v.blocking:
                self.blocked.discard(v.pos)
            v.path_i += 1
            v.pos = next_pos
            if v.blocking:
                self.blocked.add(v.pos)
            v.stuck_ticks = 0

    def _check_arrivals(self) -> None:
        for v in list(self.vehicles.values()):
            if v.phase == Phase.ARRIVING and v.path_i >= len(v.path) - 1:
                si = v.station_idx
                if si >= 0:
                    self.stations[si].state = StationState.CHARGING
                v.phase = Phase.PARKED_CHARGING

    def _charge(self) -> None:
        for v in list(self.vehicles.values()):
            if v.phase != Phase.PARKED_CHARGING:
                continue
            v.charge_remaining -= 1
            if v.charge_remaining <= 0:
                self._start_departure(v)

    def _start_departure(self, v: Vehicle) -> None:
        if v.station_idx >= 0:
            st = self.stations[v.station_idx]
            st.state = StationState.FREE
            st.vehicle_id = -1
        self.blocked.discard(v.pos)
        v.phase = Phase.DEPARTING
        bl = self._blocking_set(exclude_vid=v.vid)
        path = astar(v.pos, self.grid.exit, self.grid, bl)
        if path is None:
            path = astar(v.pos, self.grid.exit, self.grid, set())
        if path is None:
            self._abandon(v)
            return
        v.path = path
        v.path_i = 0

    def _check_exits(self) -> None:
        for v in list(self.vehicles.values()):
            if v.phase == Phase.DEPARTING and v.path_i >= len(v.path) - 1:
                v.phase = Phase.EXITED
                v.exit_tick = self.tick
                self.stats.completed += 1
                self.stats.total_trip_ticks += v.exit_tick - v.spawn_tick

    def _abandon(self, v: Vehicle) -> None:
        self.blocked.discard(v.pos)
        if v.station_idx >= 0:
            st = self.stations[v.station_idx]
            if st.vehicle_id == v.vid:
                st.state = StationState.FREE
                st.vehicle_id = -1
        v.phase = Phase.ABANDONED
        self.stats.abandoned += 1

    def _cleanup(self) -> None:
        remove = [vid for vid, v in self.vehicles.items()
                  if v.phase in (Phase.EXITED, Phase.ABANDONED)]
        for vid in remove:
            del self.vehicles[vid]

    def _update_stats(self) -> None:
        active = len(self.vehicles)
        if active > self.stats.max_concurrent:
            self.stats.max_concurrent = active
        queued = sum(1 for s in self.stations if s.state == StationState.CHARGING)
        self.stats.charger_busy_ticks += queued
        self.stats.charger_total_ticks += len(self.stations)
        if queued > self.stats.peak_chargers_busy:
            self.stats.peak_chargers_busy = queued
        in_use = sum(1 for s in self.stations if s.state != StationState.FREE)
        if in_use > self.stats.peak_stations_in_use:
            self.stats.peak_stations_in_use = in_use
        waiting = sum(1 for v in self.vehicles.values() if v.phase == Phase.ARRIVING)
        if waiting > self.stats.peak_queue_depth:
            self.stats.peak_queue_depth = waiting

    # -- main loop --

    def run(self, ticks: int) -> SimStats:
        for _ in range(ticks):
            self.tick += 1
            self._spawn()
            self._move_vehicles()
            self._check_arrivals()
            self._charge()
            self._check_exits()
            self._cleanup()
            self._update_stats()
        return self.stats


# ---------------------------------------------------------------------------
# Grid diagnostics
# ---------------------------------------------------------------------------

def diagnose_grid(grid: GridMap, stations: list[ChargerStation]) -> list[str]:
    issues: list[str] = []

    for ei, entry_pos in enumerate(grid.entries):
        ex, ey = entry_pos
        label = f"Entry[{ei}] ({ex},{ey})"
        if not grid.passable[ey][ex]:
            issues.append(f"CRITICAL: {label} is not passable")
        path = astar(entry_pos, grid.exit, grid, set())
        if path is None:
            issues.append(f"CRITICAL: {label} has no path to exit")
        else:
            issues.append(f"OK: {label}->Exit path length = {len(path)}")
        adj_pass = 0
        for dx, dy in CARDINALS:
            nx, ny = ex + dx, ey + dy
            if 0 <= nx < grid.width and 0 <= ny < grid.height and grid.passable[ny][nx]:
                adj_pass += 1
        if adj_pass < 2:
            issues.append(f"WARN: {label} has only {adj_pass} passable neighbor(s) -- choke risk")

    unreachable = 0
    for i, s in enumerate(stations):
        reachable_in = any(
            astar(ep, s.bay, grid, set()) is not None for ep in grid.entries
        )
        reachable_out = astar(s.bay, grid.exit, grid, set()) is not None
        if not (reachable_in and reachable_out):
            unreachable += 1
            issues.append(f"  Station {i} bay ({s.bay[0]},{s.bay[1]}) UNREACHABLE")
    if unreachable:
        issues.append(f"WARN: {unreachable}/{len(stations)} stations unreachable")
    else:
        issues.append(f"OK: All {len(stations)} stations reachable")

    return issues


# ---------------------------------------------------------------------------
# ASCII visualisation
# ---------------------------------------------------------------------------

def render_ascii(grid: GridMap, stations: list[ChargerStation]) -> str:
    SYMBOLS = {
        "Grass": ".", "Road": "=", "Entry": "E", "Exit": "X",
        "Lot": " ", "ParkingBayNorth": "^", "ParkingBaySouth": "v",
        "Concrete": "_", "ChargerPad": "C", "TransformerPad": "T",
        "SolarPad": "S", "BatteryPad": "b", "Empty": "o",
        "Canopy": "~", "CanopyShadow": "~",
    }
    bay_set = {s.bay for s in stations}
    pad_set = {s.pad for s in stations}
    entry_set = set(grid.entries)
    lines: list[str] = []
    for ty in range(grid.height):
        row = ""
        for x in range(grid.width):
            c = grid.content[ty][x]
            if (x, ty) in entry_set:
                row += "E"
            elif (x, ty) == grid.exit:
                row += "X"
            elif (x, ty) in pad_set:
                row += "C"
            elif (x, ty) in bay_set:
                row += "B"
            else:
                row += SYMBOLS.get(c, "?")
        lines.append(f"{ty:2d} |{row}|")
    header = "   " + "".join(f"{x % 10}" for x in range(grid.width))
    lines.insert(0, header)
    return "\n".join(lines)


# ---------------------------------------------------------------------------
# Report formatting
# ---------------------------------------------------------------------------

def format_report(
    stats: SimStats, ticks: int, num_stations: int, seed: int,
) -> dict:
    avg_trip = stats.total_trip_ticks / max(stats.completed, 1)
    utilisation = stats.charger_busy_ticks / max(stats.charger_total_ticks, 1)
    return {
        "seed": seed,
        "ticks": ticks,
        "stations": num_stations,
        "spawned": stats.spawned,
        "completed": stats.completed,
        "abandoned": stats.abandoned,
        "spawn_rejected": stats.spawn_rejected,
        "throughput_per_100t": round(stats.completed / max(ticks, 1) * 100, 2),
        "avg_trip_ticks": round(avg_trip, 1),
        "reroute_attempts": stats.reroute_attempts,
        "reroute_failures": stats.reroute_failures,
        "max_concurrent": stats.max_concurrent,
        "charger_utilisation": round(utilisation, 3),
        "peak_chargers_busy": stats.peak_chargers_busy,
        "peak_stations_in_use": stats.peak_stations_in_use,
        "peak_in_transit": stats.peak_queue_depth,
    }


def print_text_report(report: dict) -> None:
    print("\n--- Simulation Results ---")
    print(f"  Seed:                 {report['seed']}")
    print(f"  Ticks:                {report['ticks']}")
    print(f"  Charger stations:     {report['stations']}")
    print(f"  Vehicles spawned:     {report['spawned']}")
    print(f"  Completed:            {report['completed']}")
    print(f"  Abandoned:            {report['abandoned']}")
    print(f"  Spawn rejected:       {report['spawn_rejected']}")
    print(f"  Throughput/100 ticks: {report['throughput_per_100t']}")
    print(f"  Avg trip (ticks):     {report['avg_trip_ticks']}")
    print(f"  Reroute attempts:     {report['reroute_attempts']}")
    print(f"  Reroute failures:     {report['reroute_failures']}")
    print(f"  Max concurrent:       {report['max_concurrent']}")
    print(f"  Charger utilisation:  {report['charger_utilisation']}")
    print(f"  Peak chargers busy:   {report['peak_chargers_busy']}")
    print(f"  Peak stations in use: {report['peak_stations_in_use']}")
    print(f"  Peak in-transit:      {report['peak_in_transit']}")


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------

def main() -> None:
    default_map = os.path.join(
        os.path.dirname(__file__), "..", "assets", "maps", "04_scooter_alley.tmx"
    )

    parser = argparse.ArgumentParser(description="Level 4 vehicle flow simulator")
    parser.add_argument("--map", default=default_map, help="Path to TMX file")
    parser.add_argument("--ticks", type=int, default=2000, help="Sim ticks to run")
    parser.add_argument("--spawn-interval", type=int, default=4,
                        help="Ticks between spawn attempts")
    parser.add_argument("--charge-ticks", type=int, default=30,
                        help="Base ticks a vehicle stays at charger")
    parser.add_argument("--max-stuck", type=int, default=40,
                        help="Ticks stuck before abandoning")
    parser.add_argument("--seed", type=int, default=42, help="RNG seed")
    parser.add_argument("--seeds", type=int, default=1,
                        help="Run N seeds and aggregate")
    parser.add_argument("--json", action="store_true", help="Output JSON")
    parser.add_argument("--ascii", action="store_true",
                        help="Print ASCII map before sim")
    parser.add_argument("--diagnose", action="store_true",
                        help="Run grid diagnostics only (no sim)")
    args = parser.parse_args()

    tmx_path = str(Path(args.map).resolve())
    log = sys.stderr.write if args.json else sys.stdout.write

    def logln(msg: str) -> None:
        log(msg + "\n")

    logln(f"Parsing {tmx_path} ...")
    grid = parse_tmx(tmx_path)
    stations = build_stations(grid)
    logln(f"Grid: {grid.width}x{grid.height}  "
          f"Entries: {grid.entries}  Exit: {grid.exit}")
    logln(f"ChargerPads: {len(grid.charger_pads)}  "
          f"ParkingBays: {len(grid.parking_bays)}  "
          f"Stations (pad+bay pairs): {len(stations)}")

    if args.ascii:
        logln("\n" + render_ascii(grid, stations))

    diag = diagnose_grid(grid, stations)
    logln("\n--- Grid Diagnostics ---")
    for line in diag:
        logln(f"  {line}")

    if args.diagnose:
        sys.exit(0)

    has_critical = any("CRITICAL" in d for d in diag)
    if has_critical:
        logln("\nCRITICAL issues found -- sim may not be meaningful.")

    reports: list[dict] = []
    for i in range(args.seeds):
        seed = args.seed + i
        fresh_stations = build_stations(grid)
        sim = Simulation(
            grid, fresh_stations,
            spawn_interval=args.spawn_interval,
            charge_ticks=args.charge_ticks,
            max_stuck=args.max_stuck,
            seed=seed,
        )
        sim.run(args.ticks)
        report = format_report(sim.stats, args.ticks, len(fresh_stations), seed)
        reports.append(report)
        if not args.json:
            print_text_report(report)

    if args.seeds > 1 and not args.json:
        avg_tp = sum(r["throughput_per_100t"] for r in reports) / len(reports)
        avg_comp = sum(r["completed"] for r in reports) / len(reports)
        avg_aband = sum(r["abandoned"] for r in reports) / len(reports)
        logln(f"\n--- Aggregate ({args.seeds} seeds) ---")
        logln(f"  Avg throughput/100t:  {avg_tp:.2f}")
        logln(f"  Avg completed:        {avg_comp:.1f}")
        logln(f"  Avg abandoned:        {avg_aband:.1f}")

    if args.json:
        print(json.dumps(reports, indent=2))


if __name__ == "__main__":
    main()
