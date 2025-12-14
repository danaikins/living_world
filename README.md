# Living World Simulation

A small isometric world simulation built with Rust and Bevy. Creatures move, age, eat plants, reproduce, drown in water, and the UI displays stats and charts.

## Features
- Isometric tile map rendered with Bevy sprites.
- Creatures with hunger, age, reproduction cooldowns, and simple behaviors.
- Plants that grow randomly and can be eaten, leaving exhausted soil.
- Water placement via mouse to flood tiles and drown creatures.
- UI showing FPS, days, counts, and a small chart for health/demographics.

## Requirements
- Windows (development verified on Windows)
- Rust (stable)
- Cargo
- Bevy (brought in by Cargo dependencies)

## Quick start (Windows)
1. Install Rust (if needed): https://rustup.rs/
2. Clone the repo and enter the project directory.
3. Run the app:
```
    cargo run --release
```
Or run from RustRover / your IDE with the project's Cargo configuration.

## Controls
- Move mouse to move the highlighted tile cursor.
- Left mouse button: place water on the hovered tile (turns tile blue), kills plants and removes exhausted soil.
- Right mouse button: remove water from the hovered tile (restore land).

## Tweakable constants
Edit `src/main.rs` to adjust:
- `TILE_WIDTH`, `TILE_HEIGHT` — tile sizing.
- `MAP_SIZE` — map radius / extents.
- Timers and rates for movement, plant growth, reproduction, hunger, etc.

## Project layout
- `src/main.rs` — main game logic and systems (spawning, input, movement, UI, game rules).
- Assets are generated via code (no external assets required).

## Systems overview
- `cursor_system` — convert mouse to grid, place/remove water.
- `spawn_map` & `setup` — initial world, camera, UI.
- `move_creatures`, `sync_creature_visuals` — AI movement and visual interpolation.
- `plant_growth_system` — random plant spawning.
- `creature_state_update`, `creature_eating`, `creature_reproduction` — life logic.
- `handle_drowning`, `handle_exhaustion`, `reaper_system` — cleanup and status effects.
- `update_stats_ui`, `update_chart_ui` — UI updates.

## Debugging / Development tips
- Use `cargo run` with debug symbols while iterating.
- Increase logging (e.g., `println!`) in systems for runtime inspection.
- Adjust spawn counts and timer durations in `spawn_map` and component initializers for faster testing.

## License
MIT — modify and use freely.
