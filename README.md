# Kilowatt Tycoon

**Run a profitable charging network in a world where everything breaks.** 

A 2D top-down tycoon game where you operate an EV charging network. Manage chargers, dispatch technicians, and manage your budget wisely.

Built with [Bevy](https://bevyengine.org/) 0.17+ in Rust.

## Quick Start

### Rust

Start by installing the `Rust` toolchain locally if you don't already have it.

One way is to use the [standalone installers](https://forge.rust-lang.org/infra/other-installation-methods.html#standalone-installers)

**Requirements:** Rust 1.85+ (2024 edition)

### Native

```bash
# Run the game
cargo run

# Run in release mode (faster)
cargo run --release

# Run tests
cargo test

# Updated/generate assets
python3 tools/build_assets.py
```

### WASM

Test the game locally in the browser:

```bash
trunk serve
```

### Level editing

Grab a copy of [Tiled](https://github.com/mapeditor/tiled/releases/tag/v1.11.2) and install it locally.

Open the tiled project in the root of the `/asset` directory, or create a new one as you see fit!

## Project Structure

```
src/
├── components/     # ECS components (Charger, Driver, Ticket, etc.)
├── systems/        # Game logic (charging, movement, power dispatch)
├── resources/      # Global state (GameClock, MultiSiteManager)
├── ui/             # User interface
└── states/         # Game state machine

assets/             # Sprites, icons, tiles (SVG + PNG)
spec/               # Design documentation
```

## Documentation

| Document | Description |
|----------|-------------|
| [GDD_COMPLETE.md](spec/GDD_COMPLETE.md) | Full game design document |
| [ARCHITECTURE.md](spec/ARCHITECTURE.md) | Technical architecture, plugins, systems |
| [SPEC_GRID_POWER.md](spec/SPEC_GRID_POWER.md) | Electrical simulation and power economics |
| [SPEC_DEMAND_CHARGES.md](spec/SPEC_DEMAND_CHARGES.md) | Demand charge mechanics and UX |
| [SPEC_OPERATIONS.md](spec/SPEC_OPERATIONS.md) | Remote actions, tickets, technicians |
| [SPEC_SYSTEMS.md](spec/SPEC_SYSTEMS.md) | Emotions, traffic, pathfinding |
| [SPEC_SANDBOX_MVP.md](spec/SPEC_SANDBOX_MVP.md) | MVP session design and sandbox parameters |
| [ECOSYSTEM.md](spec/ECOSYSTEM.md) | Site types and world systems |
| [STYLE_GUIDE.md](spec/STYLE_GUIDE.md) | Art style guide |

## Contributing

Anyone and everyone is welcome to contribute! Please review the [CONTRIBUTING.md](./CONTRIBUTING.md) document for more details. The best way to get started is to find an open issue, and then start hacking on implementing it. Letting other folks know that you are working on it, and sharing progress is a great approach. Open pull requests early and often, and please use GitHub's draft pull request feature.

## License

GNU General Public License version 3 (GPL-3.0-only)
