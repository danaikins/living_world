use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy::ui::ComputedNode;
use bevy::input::keyboard::{KeyboardInput, Key};
use bevy::ecs::prelude::ChildSpawnerCommands;
use std::collections::HashMap;

// Constants
const TILE_WIDTH: f32 = 64.0;
const TILE_HEIGHT: f32 = 32.0;
const MAP_SIZE: i32 = 20;

// ========================
// 1) CONFIG RESOURCE
// =========================
#[derive(Resource, Clone)]
struct SimulationConfig {
    // Map / tiles
    map_size: i32,
    tile_w: f32,
    tile_h: f32,

    // World / growth
    plant_spawn_chance_per_tick: f32,
    soil_exhaust_seconds_after_eat: f32,
    blood_fx_seconds: f32,

    // Movement
    base_move_seconds: f32,
    reproduction_move_seconds: f32,
    overfed_move_multiplier: f32,

    // Hunger
    hunger_starve_threshold: f32,
    sheep_hunger_burn_adult: f32,
    sheep_hunger_burn_baby: f32,
    wolf_hunger_burn_adult: f32,
    wolf_hunger_burn_baby: f32,

    // Eating rules
    eat_skip_if_hunger_below: f32, // "already full" threshold

    // Wolf berry mechanics
    wolf_berry_stun_ticks: u32,

    // Wolf fruit preference weights when very low health (hunger >= 70)
    wolf_low_health_hunger_threshold: f32,
    wolf_low_health_weight_fruit: i32,
    wolf_low_health_weight_meat: i32,

    // Species configs (keyed by species_id)
    species: HashMap<u32, SpeciesConfig>,

    // Debug UI
    debug_panel_enabled: bool,
}

#[derive(Clone)]
struct SpeciesConfig {
    name: &'static str,
    starting_count: u32,

    // Baby->Adult timing
    adult_seconds: f32,

    // Reproduction
    reproduction_chance: f32, // 0..1
    reproduction_cooldown_seconds: f32,

    // Sight
    sight_range: i32,
}

impl Default for SimulationConfig {
    fn default() -> Self {
        let mut species = HashMap::new();

        // Sheep: species_id = 0
        species.insert(
            0,
            SpeciesConfig {
                name: "Sheep",
                starting_count: 12,          // CONFIG: starting sheep
                adult_seconds: 10.0,         // CONFIG: sheep mature faster
                reproduction_chance: 0.10,   // CONFIG
                reproduction_cooldown_seconds: 30.0, // CONFIG: reduced cooldown
                sight_range: 8,              // CONFIG
            },
        );

        // Wolves: species_id = 1
        species.insert(
            1,
            SpeciesConfig {
                name: "Wolves",
                starting_count: 4,           // CONFIG: starting wolves
                adult_seconds: 20.0,         // CONFIG
                reproduction_chance: 0.10,   // CONFIG (same as sheep for now)
                reproduction_cooldown_seconds: 70.0, // CONFIG
                sight_range: 10,             // CONFIG
            },
        );

        Self {
            // Map / tiles (mirror your constants)
            map_size: 20,
            tile_w: 64.0,
            tile_h: 32.0,

            // Growth
            plant_spawn_chance_per_tick: 0.05,
            soil_exhaust_seconds_after_eat: 10.0,
            blood_fx_seconds: 30.0,

            // Movement
            base_move_seconds: 0.2,
            reproduction_move_seconds: 0.5,
            overfed_move_multiplier: 6.6,

            // Hunger
            hunger_starve_threshold: 100.0,
            sheep_hunger_burn_adult: 3.3,
            sheep_hunger_burn_baby: 1.65,
            wolf_hunger_burn_adult: 3.3 * 1.5,
            wolf_hunger_burn_baby: 1.65 * 1.5,

            // Eating
            eat_skip_if_hunger_below: 5.0,

            // Wolf berry stun
            wolf_berry_stun_ticks: 2,

            // Low-health weights for wolves
            wolf_low_health_hunger_threshold: 70.0,
            wolf_low_health_weight_fruit: 80,
            wolf_low_health_weight_meat: 50,

            // Species
            species,

            // Debug UI
            debug_panel_enabled: true,
        }
    }
}

impl SimulationConfig {
    fn s(&self, id: u32) -> &SpeciesConfig {
        self.species.get(&id).expect("Missing SpeciesConfig")
    }
    fn s_mut(&mut self, id: u32) -> &mut SpeciesConfig {
        self.species.get_mut(&id).expect("Missing SpeciesConfig")
    }
}

// --- COMPONENTS ---
// This tags an entity as being a "Tile" at a specific grid location
#[derive(Component)]
struct Tile {
    x: i32,
    y: i32,
}

// 1. Tag for the creature
#[derive(Component)]
struct Creature;

// 2. Logic Position (Where they actually are in the grid)
#[derive(Component)]
struct GridPosition {
    x: i32,
    y: i32,
}

// 3. A timer so they don't move at light speed (move once every 0.5 seconds)
#[derive(Component)]
struct MoveTimer(Timer);

// This tags the floating highlight box
#[derive(Component)]
struct MapCursor;

#[derive(Component)]
struct Water;

#[derive(Component)]
struct Plant;

#[derive(Component)]
struct Hunger(f32); // Value from 0.0 (Full) to 100.0 (Starving)

#[derive(Component)]
struct Dead;

#[derive(Component)]
struct ExhaustedSoil(Timer);

#[derive(Resource)]
struct GameStats {
    days: f32,
}

#[derive(Component)]
struct StatsText;

// Defines physical limits
#[derive(Component)]
struct CreatureStats {
    sight_range: i32, // How many tiles away they can see
    species_id: u32,  // 0 = White Squares, 1 = Red Triangles, etc.
}

// Defines logic flags
#[derive(Component)]
struct CreatureBehavior {
    scared_of_water: bool,
    altruistic: bool, // If true, won't eat if healthy + friend is nearby
}

#[derive(Component)]
struct Age {
    seconds_alive: f32,
    is_adult: bool,
}

#[derive(Component)]
struct ChartTextHealthy; // White count

#[derive(Component)]
struct ChartTextHungry;  // Yellow count

#[derive(Component)]
struct ChartTextCritical; // Red count

#[derive(Component)]
struct ChartTextAdults;

#[derive(Component)]
struct ChartTextBabies;

#[derive(Component)]
struct ReproductionCooldown(Timer);

#[derive(Component)]
struct History {
    last_x: i32,
    last_y: i32,
}

#[derive(Component)]
struct Digesting; // State 1: Immobile, waiting for hunger > 0

#[derive(Component)]
struct Overfed(Timer); // State 2: Slow movement for 5 ticks

//#[derive(Component)]
//struct WolfPart;

#[derive(Default, Resource)]
struct PopulationStats {
    // species_id -> counters
    species: HashMap<u32, SpeciesCounters>,
}

#[derive(Default, Clone, Copy)]
struct SpeciesCounters {
    born: u32,        // born via reproduction
    total_ever: u32,  // total spawned ever (initial + births)
}

#[derive(Component)]
struct SpeciesStatsSheepText;

#[derive(Component)]
struct SpeciesStatsWolfText;

#[derive(Component)]
struct BerryStun(Timer); // short immobile state after eating berries

#[derive(Component)]
struct DebugPanelRoot;

#[derive(Component)]
struct DebugPanelVisible;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum ConfigField {
    PlantSpawnChance,
    SheepStartCount,
    WolfStartCount,
    SheepAdultSeconds,
    WolfAdultSeconds,
}

#[derive(Component)]
struct Slider {
    field: ConfigField,
    min: f32,
    max: f32,
}

#[derive(Component)]
struct SliderKnob {
    field: ConfigField,
}

#[derive(Component)]
struct SliderValueText {
    field: ConfigField,
}

#[derive(Component)]
struct TextBox {
    field: ConfigField,
}

#[derive(Component)]
struct TextBoxText {
    field: ConfigField,
}

#[derive(Resource, Default)]
struct TextBoxFocus {
    active: Option<ConfigField>,
    buffer: String,
}



#[derive(Component)]
struct WorldShadow {
    phase: f32,
    drift_a: f32,
    drift_b: f32,
    base_y: f32,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Living World Simulation".into(),
                resolution: bevy::window::WindowResolution::new(1280, 720),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(SimulationConfig::default())
        .insert_resource(PopulationStats::default())
        .insert_resource(GameStats { days: 0.0 })

        // Order startup so config exists before spawn_map
        .add_systems(Startup, (setup, spawn_map, setup_chart, setup_debug_panel).chain())

        .add_systems(Update, (
            toggle_debug_panel,
            debug_panel_visibility,

            cursor_system,
            move_creatures,
            sync_creature_visuals,
            plant_growth_system,
            handle_drowning,
            reaper_system,
            handle_exhaustion,
            update_stats_ui,
            update_species_stats_ui,
            update_chart_ui,
            creature_state_update,
            creature_eating,
            predator_hunting_system,
            creature_reproduction,
            debug_slider_system,
            debug_textbox_system,
        ))

        .add_systems(Startup, spawn_world_shadow)
        .add_systems(Update, animate_world_shadow)
        .run();
}


fn setup(mut commands: Commands) {
    // 1. Initialize Game Stats Resource (Day 0)
    //commands.insert_resource(GameStats { days: 0.0 });

    // NEW: Init population stats
    //commands.insert_resource(PopulationStats::default());

    // 2. Spawn Camera
    let mut camera_transform = Transform::from_xyz(0.0, 0.0, 800.0);
    camera_transform.scale = Vec3::new(1.5, 1.5, 1.0);
    commands.spawn((Camera2d, camera_transform));

    // 3. Spawn Cursor
    commands.spawn((
        Sprite::from_color(Color::srgba(1.0, 0.0, 0.0, 0.5), Vec2::new(TILE_WIDTH, TILE_HEIGHT)),
        Transform::from_xyz(0.0, 0.0, 1.0),
        MapCursor,
    ));

    // 4. Spawn UI Text (Top-Left) - general world stats (keep yours)
    commands.spawn((
        Text::new("Stats: Loading..."),
        TextFont {
            font_size: 20.0,
            ..default()
        },
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(10.0),
            ..default()
        },
        StatsText,
    ));

    // 5. NEW: Species Stats Panel (Top-Left, below general stats)
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            top: Val::Px(160.0),
            left: Val::Px(10.0),
            padding: UiRect::all(Val::Px(10.0)),
            column_gap: Val::Px(25.0),
            flex_direction: FlexDirection::Row, // columns side-by-side
            ..default()
        })
        .insert(BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)))
        .with_children(|parent| {
            // ---- COLUMN 1: Sheep ----
            parent
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(6.0),
                    ..default()
                })
                .with_children(|col| {
                    col.spawn((
                        Text::new("Sheep"),
                        TextFont { font_size: 16.0, ..default() },
                        TextColor(Color::srgb(1.0, 1.0, 1.0)),
                    ));
                    col.spawn((
                        Text::new("Born: 0\nCurrent: 0\nTotal Ever: 0"),
                        TextFont { font_size: 14.0, ..default() },
                        SpeciesStatsSheepText,
                    ));
                });

            // ---- COLUMN 2: Wolves ----
            parent
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(6.0),
                    ..default()
                })
                .with_children(|col| {
                    col.spawn((
                        Text::new("Wolves"),
                        TextFont { font_size: 16.0, ..default() },
                        TextColor(Color::srgb(1.0, 1.0, 1.0)),
                    ));
                    col.spawn((
                        Text::new("Born: 0\nCurrent: 0\nTotal Ever: 0"),
                        TextFont { font_size: 14.0, ..default() },
                        SpeciesStatsWolfText,
                    ));
                });
        });
}

fn spawn_map(
    mut commands: Commands,
    mut pop: ResMut<PopulationStats>,
    cfg: Res<SimulationConfig>,
) {
    let map_size = cfg.map_size;
    let tile_w = cfg.tile_w;
    let tile_h = cfg.tile_h;

    // Ground
    for x in -map_size..map_size {
        for y in -map_size..map_size {
            let screen_x = (x - y) as f32 * (tile_w / 2.0);
            let screen_y = (x + y) as f32 * (tile_h / 2.0);
            commands.spawn((
                Sprite::from_color(Color::srgb(0.3, 0.5, 0.3), Vec2::new(tile_w - 2.0, tile_h - 2.0)),
                Transform::from_xyz(screen_x, screen_y, 0.0),
                Tile { x, y },
            ));
        }
    }

    // Starting sheep (born + total_ever; start as babies)
    let sheep_cfg = cfg.s(0);
    for i in 0..(sheep_cfg.starting_count as i32) {
        let entry = pop.species.entry(0).or_default();
        entry.born += 1;
        entry.total_ever += 1;

        commands.spawn((
            Sprite::from_color(Color::srgb(1.0, 1.0, 1.0), Vec2::new(20.0, 20.0)),
            Transform::from_xyz(0.0, 0.0, 2.0),
            Creature,
            GridPosition { x: i, y: i },
            MoveTimer(Timer::from_seconds(cfg.base_move_seconds, TimerMode::Repeating)),
            Hunger(0.0),
            CreatureStats { sight_range: sheep_cfg.sight_range, species_id: 0 },
            CreatureBehavior { scared_of_water: true, altruistic: true },
            Age { seconds_alive: 0.0, is_adult: false },
            History { last_x: i, last_y: i },
        ));
    }

    // Starting wolves (born + total_ever; start as babies)
    // Spread 4 wolves in a simple pattern
    let wolf_cfg = cfg.s(1);
    let wolf_coords = vec![(-6, -6), (-4, -6), (4, -6), (6, -6)];
    for idx in 0..(wolf_cfg.starting_count.min(wolf_coords.len() as u32) as usize) {
        let (wx, wy) = wolf_coords[idx];

        let entry = pop.species.entry(1).or_default();
        entry.born += 1;
        entry.total_ever += 1;

        commands.spawn((
            Sprite::from_color(Color::srgb(0.4, 0.2, 0.1), Vec2::new(22.0, 22.0)),
            Transform::from_xyz(0.0, 0.0, 2.0),
            Creature,
            GridPosition { x: wx, y: wy },
            MoveTimer(Timer::from_seconds(cfg.base_move_seconds, TimerMode::Repeating)),
            Hunger(0.0),
            CreatureStats { sight_range: wolf_cfg.sight_range, species_id: 1 },
            CreatureBehavior { scared_of_water: true, altruistic: false },
            Age { seconds_alive: 0.0, is_adult: false },
            History { last_x: wx, last_y: wy },
        ));
    }
}

fn spawn_world_shadow(mut commands: Commands, cfg: Res<SimulationConfig>) {
    let map = cfg.map_size as f32;
    let half_w = cfg.tile_w * map;
    let half_h = cfg.tile_h * map;

    // True diamond bounds of the tile carpet
    let diamond_width  = half_w * 2.0;
    let diamond_height = half_h * 2.0;

    // Shadow slightly larger than the platform
    let scale = 1.5;
    let w = diamond_width * scale;
    let h = diamond_height * scale;

    // Push down so it peeks out under the platform
    let base_y = -half_h * 0.9;
    let base_z = -0.01;

    commands.spawn((
        Sprite::from_color(Color::srgba(0.05, 0.05, 0.08, 0.45), Vec2::new(w, h)),
        Transform::from_xyz(0.0, base_y, base_z)
            .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_4)), // 45° for diamond
        WorldShadow {
            phase: rand::random::<f32>() * std::f32::consts::TAU,
            drift_a: 0.06 + rand::random::<f32>() * 0.03,
            drift_b: 0.03 + rand::random::<f32>() * 0.02,
            base_y,
        },
    ));
}

fn animate_world_shadow(time: Res<Time>, mut q: Query<(&mut Transform, &mut Sprite, &mut WorldShadow)>) {
    for (mut tr, mut spr, mut sh) in q.iter_mut() {
        let dt = time.delta().as_secs_f32();
        sh.phase += dt;

        // Slow deliberate "hover" motion (not jittery)
        let y_float = (sh.phase * sh.drift_a).sin() * 18.0 + (sh.phase * sh.drift_b).sin() * 10.0;
        tr.translation.y = sh.base_y + y_float;

        // Keep it hovering around its original offset by nudging relative to current baseline
        // (Better: store base_y if you want; this works fine visually.)
        tr.translation.y += y_float * dt;

        // Color pulse: black <-> deep blue, with red hints
        let pulse = (time.elapsed_secs() * 0.35).sin() * 0.5 + 0.5;
        let a = 0.25 + pulse * 0.20; // lower max alpha
        let r = 0.02 + pulse * 0.12;
        let g = 0.01 + pulse * 0.03;
        let b = 0.06 + pulse * 0.28;

        spr.color = Color::srgba(r, g, b, a);
    }
}


// --- LOGIC SYSTEMS ---

// This function figures out where the mouse is in the Isometric World
fn cursor_system(
    mut commands: Commands,
    q_window: Query<&Window, With<PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform)>,
    mut q_cursor: Query<&mut Transform, With<MapCursor>>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    mut q_tiles: Query<(Entity, &Tile, &mut Sprite)>,
    // NEW: We need to see plants and soil markers to delete them
    q_plants: Query<(Entity, &GridPosition), With<Plant>>,
    q_exhausted: Query<(Entity, &GridPosition), With<ExhaustedSoil>>,
) {
    let (camera, camera_transform) = q_camera.single().expect("Camera not found!");
    let window = q_window.single().expect("Window not found!");
    let mut cursor_transform = q_cursor.single_mut().expect("Cursor not found!");

    if let Some(screen_pos) = window.cursor_position() {
        if let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, screen_pos) {

            let half_w = TILE_WIDTH / 2.0;
            let half_h = TILE_HEIGHT / 2.0;
            let grid_x = (world_pos.y / half_h + world_pos.x / half_w) / 2.0;
            let grid_y = (world_pos.y / half_h - world_pos.x / half_w) / 2.0;
            let snapped_x = grid_x.round();
            let snapped_y = grid_y.round();

            cursor_transform.translation.x = (snapped_x - snapped_y) * half_w;
            cursor_transform.translation.y = (snapped_x + snapped_y) * half_h;

            // --- LEFT CLICK: Create Water & Destroy Nature ---
            if mouse_input.just_pressed(MouseButton::Left) {
                // 1. Turn Tile Blue
                for (entity, tile, mut sprite) in q_tiles.iter_mut() {
                    if tile.x == snapped_x as i32 && tile.y == snapped_y as i32 {
                        sprite.color = Color::srgb(0.2, 0.2, 0.8);
                        commands.entity(entity).insert(Water);
                    }
                }

                // 2. Kill Plants on this spot
                for (entity, pos) in q_plants.iter() {
                    if pos.x == snapped_x as i32 && pos.y == snapped_y as i32 {
                        commands.entity(entity).insert(Dead);
                    }
                }

                // 3. Remove Exhausted Soil markers on this spot
                for (entity, pos) in q_exhausted.iter() {
                    if pos.x == snapped_x as i32 && pos.y == snapped_y as i32 {
                        commands.entity(entity).insert(Dead);
                    }
                }
            }

            // --- RIGHT CLICK: Remove Water (Restore Land) ---
            if mouse_input.pressed(MouseButton::Right) {
                for (entity, tile, mut sprite) in q_tiles.iter_mut() {
                    if tile.x == snapped_x as i32 && tile.y == snapped_y as i32 {
                        sprite.color = Color::srgb(0.3, 0.5, 0.3);
                        commands.entity(entity).remove::<Water>();
                    }
                }
            }
        }
    }
}

fn move_creatures(
    mut commands: Commands,
    time: Res<Time>,
    cfg: Res<SimulationConfig>,
    mut param_set: ParamSet<(
        Query<(Entity, &GridPosition, &CreatureStats, &Age), (With<Creature>, Without<Dead>)>,
        Query<(
            Entity,
            &mut GridPosition,
            &mut MoveTimer,
            &CreatureBehavior,
            &CreatureStats,
            Option<&ReproductionCooldown>,
            &mut History,
            Option<&Digesting>,
            Option<&Overfed>,
            Option<&mut BerryStun>,
            &Hunger,
            &Age,
        ), (With<Creature>, Without<Dead>)>,
        Query<&GridPosition, With<Plant>>,
        Query<&GridPosition, With<Water>>,
    )>,
) {
    struct CreatureSnapshot {
        entity: Entity,
        x: i32,
        y: i32,
        species: u32,
        is_adult: bool,
    }

    let creature_targets: Vec<CreatureSnapshot> = param_set
        .p0()
        .iter()
        .map(|(e, pos, stats, age)| CreatureSnapshot {
            entity: e,
            x: pos.x,
            y: pos.y,
            species: stats.species_id,
            is_adult: age.is_adult,
        })
        .collect();

    let plant_positions: Vec<(i32, i32)> = param_set.p2().iter().map(|p| (p.x, p.y)).collect();
    let water_tiles: Vec<(i32, i32)> = param_set.p3().iter().map(|p| (p.x, p.y)).collect();

    for (
        my_entity,
        mut my_pos,
        mut timer,
        behavior,
        my_stats,
        cooldown,
        mut history,
        digesting,
        overfed,
        berry_stun,
        my_hunger,
        my_age,
    ) in param_set.p1().iter_mut()
    {
        // --- BERRY STUN: immobile until timer completes ---
        if let Some(mut stun) = berry_stun {
            stun.0.tick(time.delta());
            if !stun.0.just_finished() {
                continue;
            }
            commands.entity(my_entity).remove::<BerryStun>();
        }

        // If digesting, no movement
        if digesting.is_some() {
            continue;
        }

        // --- Movement timer (Repeating) ---
        let mut move_seconds = cfg.base_move_seconds;
        if cooldown.is_some() {
            move_seconds = cfg.reproduction_move_seconds;
        }
        if overfed.is_some() {
            move_seconds = cfg.base_move_seconds * cfg.overfed_move_multiplier;
        }

        timer.0.set_duration(std::time::Duration::from_secs_f32(move_seconds));
        timer.0.tick(time.delta());

        // ✅ THIS is the important change
        if !timer.0.just_finished() {
            continue;
        }

        let old_x = my_pos.x;
        let old_y = my_pos.y;

        // === TARGET SELECTION (unchanged from your current logic) ===
        let mut target_pos: Option<(i32, i32)> = None;
        let mut target_type: i32 = 0;      // 1=fruit, 2=mate, 3=prey, 4=predator
        let mut target_weight: i32 = 20;

        let is_sheep = my_stats.species_id == 0;
        let is_wolf = my_stats.species_id == 1;

        let hunger_level = my_hunger.0;
        let is_full = hunger_level <= 10.0;
        let can_breed = my_age.is_adult && cooldown.is_none() && overfed.is_none();

        if is_sheep {
            if is_full && can_breed {
                let mut best_dist = 9999;
                for other in &creature_targets {
                    if my_entity == other.entity || other.species != 0 { continue; }
                    let dist = (my_pos.x - other.x).abs() + (my_pos.y - other.y).abs();
                    if dist > 1 && dist < my_stats.sight_range && dist < best_dist {
                        best_dist = dist;
                        target_pos = Some((other.x, other.y));
                        target_type = 2;
                        target_weight = 20;
                    }
                }
            }

            if target_pos.is_none() && hunger_level > 30.0 {
                let mut best_dist = 9999;
                for &(px, py) in &plant_positions {
                    let dist = (my_pos.x - px).abs() + (my_pos.y - py).abs();
                    if dist > 0 && dist < my_stats.sight_range && dist < best_dist {
                        best_dist = dist;
                        target_pos = Some((px, py));
                        target_type = 1;
                        target_weight = 20;
                    }
                }
            }
        }

        if is_wolf {
            if can_breed && hunger_level <= 50.0 {
                let mut best_dist = 9999;
                for other in &creature_targets {
                    if my_entity == other.entity || other.species != 1 { continue; }
                    if !other.is_adult { continue; }
                    let dist = (my_pos.x - other.x).abs() + (my_pos.y - other.y).abs();
                    if dist > 1 && dist < my_stats.sight_range && dist < best_dist {
                        best_dist = dist;
                        target_pos = Some((other.x, other.y));
                        target_type = 2;
                        target_weight = 60;
                    }
                }
            }
        }

        let mut best_prey: Option<(i32, i32, i32)> = None;
        let mut best_predator: Option<(i32, i32, i32)> = None;

        for other in &creature_targets {
            if my_entity == other.entity { continue; }
            let dist = (my_pos.x - other.x).abs() + (my_pos.y - other.y).abs();
            if dist >= my_stats.sight_range { continue; }

            if is_wolf && other.species == 0 {
                if my_age.is_adult && !(target_type == 2 && hunger_level <= 50.0) {
                    if best_prey.map(|(_,_,d)| dist < d).unwrap_or(true) {
                        best_prey = Some((other.x, other.y, dist));
                    }
                }
            } else if is_sheep && other.species == 1 {
                if other.is_adult {
                    if best_predator.map(|(_,_,d)| dist < d).unwrap_or(true) {
                        best_predator = Some((other.x, other.y, dist));
                    }
                }
            }
        }

        if let Some((px, py, _)) = best_predator {
            target_pos = Some((px, py));
            target_type = 4;
            target_weight = 20;
        }

        if target_type != 2 {
            if let Some((sx, sy, _)) = best_prey {
                target_pos = Some((sx, sy));
                target_type = 3;
                target_weight = 20;
            }
        }

        if is_wolf {
            let can_eat_fruit = !my_age.is_adult || hunger_level <= 30.0 || hunger_level >= 70.0;
            if hunger_level >= 70.0 && target_type == 3 {
                target_weight = 50;
            }
            if can_eat_fruit && target_type != 2 && target_type != 3 {
                let mut best_dist = 9999;
                for &(px, py) in &plant_positions {
                    let dist = (my_pos.x - px).abs() + (my_pos.y - py).abs();
                    if dist > 0 && dist < my_stats.sight_range && dist < best_dist {
                        best_dist = dist;
                        target_pos = Some((px, py));
                        target_type = 1;
                        target_weight = if hunger_level >= 70.0 { 80 } else { 20 };
                    }
                }
            }
        }

        // === MOVE EVALUATION ===
        let moves = [(0, 1), (0, -1), (-1, 0), (1, 0)];
        let mut best_move = (0, 0);
        let mut best_score = -9999_i32;

        for (dx, dy) in moves {
            let nx = my_pos.x + dx;
            let ny = my_pos.y + dy;

            // ✅ Use config map size, not MAP_SIZE
            if nx < -cfg.map_size || nx >= cfg.map_size || ny < -cfg.map_size || ny >= cfg.map_size {
                continue;
            }

            let mut score = rand::random::<i32>() % 20;

            if behavior.scared_of_water && water_tiles.contains(&(nx, ny)) {
                score -= 1000;
            }

            if nx == history.last_x && ny == history.last_y {
                score -= 30;
            }

            if let Some((tx, ty)) = target_pos {
                let dist_now = (my_pos.x - tx).abs() + (my_pos.y - ty).abs();
                let dist_after = (nx - tx).abs() + (ny - ty).abs();
                let delta = dist_after - dist_now;

                match target_type {
                    1 | 2 | 3 => score -= delta * target_weight,
                    4 => score += delta * target_weight,
                    _ => {}
                }
            }

            if score > best_score {
                best_score = score;
                best_move = (dx, dy);
            }
        }

        my_pos.x += best_move.0;
        my_pos.y += best_move.1;

        history.last_x = old_x;
        history.last_y = old_y;
    }
}

fn sync_creature_visuals(
    time: Res<Time>, // We need Time to calculate animation speed
    mut query: Query<(&mut Transform, &GridPosition), With<Creature>>
) {
    for (mut transform, pos) in query.iter_mut() {
        // 1. Calculate the TARGET position (Where they logically are)
        let target_x = (pos.x - pos.y) as f32 * (TILE_WIDTH / 2.0);
        let target_y = (pos.x + pos.y) as f32 * (TILE_HEIGHT / 2.0);

        // We define the target vector.
        // We keep Z at 2.0 so they stay above the ground.
        let target = Vec3::new(target_x, target_y, 2.0);

        // 2. Interpolate (Lerp) towards the target
        // "15.0 * dt" controls the speed.
        // Higher = Snappier, Lower = Floaty/Driftier.
        // 15.0 is a good balance for top-down movement.
        let interpolation_speed = 15.0 * time.delta().as_secs_f32();

        transform.translation = transform.translation.lerp(target, interpolation_speed);
    }
}

fn handle_drowning(
    mut commands: Commands,
    // FIX: Add Without<Dead> so we don't try to kill ghosts
    q_creatures: Query<(Entity, &GridPosition), (With<Creature>, Without<Dead>)>,
    q_water: Query<&Tile, With<Water>>,
) {
    for (creature_entity, creature_pos) in q_creatures.iter() {
        for water_tile in q_water.iter() {
            if creature_pos.x == water_tile.x && creature_pos.y == water_tile.y {
                commands.entity(creature_entity).insert(Dead);
                println!("Drowned!");
            }
        }
    }
}

fn plant_growth_system(
    mut commands: Commands,
    cfg: Res<SimulationConfig>,
    q_tiles: Query<(&Tile, &Sprite), Without<Water>>,
    q_plants: Query<&GridPosition, With<Plant>>,
    q_exhausted: Query<&GridPosition, With<ExhaustedSoil>>,
) {
    if rand::random::<f32>() < cfg.plant_spawn_chance_per_tick {
        let map_size = cfg.map_size;
        let tile_w = cfg.tile_w;
        let tile_h = cfg.tile_h;

        let x = (rand::random::<i32>().abs() % (map_size * 2)) - map_size;
        let y = (rand::random::<i32>().abs() % (map_size * 2)) - map_size;

        let mut valid_ground = false;
        for (tile, _sprite) in q_tiles.iter() {
            if tile.x == x && tile.y == y {
                valid_ground = true;
                break;
            }
        }

        let mut occupied = false;
        // Check Plants
        for plant_pos in q_plants.iter() {
            if plant_pos.x == x && plant_pos.y == y {
                occupied = true;
                break;
            }
        }
        // NEW: Check Exhausted Soil
        for exhausted_pos in q_exhausted.iter() {
            if exhausted_pos.x == x && exhausted_pos.y == y {
                occupied = true;
                break;
            }
        }

        if valid_ground && !occupied {
            let screen_x = (x - y) as f32 * (tile_w / 2.0);
            let screen_y = (x + y) as f32 * (tile_h / 2.0);

            commands.spawn((
                Sprite::from_color(Color::srgb(0.2, 0.8, 0.2), Vec2::new(15.0, 15.0)),
                Transform::from_xyz(screen_x, screen_y, 0.5),
                Plant,
                GridPosition { x, y },
            ));
        }
    }
}

// SYSTEM 1: Updates internal state (Hunger, Age, Visuals, Timers)
fn creature_state_update(
    mut commands: Commands,
    time: Res<Time>,
    cfg: Res<SimulationConfig>,
    // The Query includes Option<&Digesting>
    mut q_creatures: Query<(Entity, &mut Hunger, &mut Sprite, &mut Age, Option<&mut ReproductionCooldown>, &CreatureStats, Option<&Digesting>, Option<&mut Overfed>), (With<Creature>, Without<Dead>)>,
) {
    let dt = time.delta().as_secs_f32();
    let current_time = time.elapsed_secs();

    // MAKE SURE 'digesting' IS IN THIS LIST ↓
    for (entity, mut hunger, mut sprite, mut age, mut cooldown_opt, stats, digesting, mut overfed_opt) in q_creatures.iter_mut() {

        // 1. Growth & Size
        age.seconds_alive += dt;
        let adult_seconds = cfg.s(stats.species_id).adult_seconds;
        if !age.is_adult && age.seconds_alive > adult_seconds {
            age.is_adult = true;
        }

        let base_size = if stats.species_id == 1 { 22.0 } else { 20.0 };
        let target_scale = if age.is_adult { base_size } else { base_size / 2.0 };
        sprite.custom_size = Some(Vec2::new(target_scale, target_scale));
/*
        let burn_rate = if age.is_adult { 3.3 } else { 1.65 };
        let final_burn = if stats.species_id == 1 { burn_rate * 1.5 } else { burn_rate };
        hunger.0 += final_burn * dt;

 */

        // Burn per species + age
        let burn = match (stats.species_id, age.is_adult) {
            (0, true) => cfg.sheep_hunger_burn_adult,
            (0, false) => cfg.sheep_hunger_burn_baby,
            (1, true) => cfg.wolf_hunger_burn_adult,
            (1, false) => cfg.wolf_hunger_burn_baby,
            _ => 3.0,
        };
        hunger.0 += burn * dt;

        // 2. DIGESTION LOGIC
        if digesting.is_some() {
            // Visual: Dark while digesting
            sprite.color = Color::srgb(0.2, 0.1, 0.05);

            // Burn off the "Overheal" (Waiting for hunger to reach 0.0)
            if hunger.0 >= 0.0 {
                commands.entity(entity).remove::<Digesting>();
                // Enter Overfed state (Slow movement)
                commands.entity(entity).insert(Overfed(Timer::from_seconds(5.0, TimerMode::Once)));
            }
        }
        else if let Some(ref mut overfed_timer) = overfed_opt {
            // Visual: Greenish tint
            sprite.color = Color::srgb(0.4, 0.3, 0.1);

            overfed_timer.0.tick(time.delta());
            if overfed_timer.0.is_finished() {
                commands.entity(entity).remove::<Overfed>();
            }
        }
        else {
            // Standard Colors
            if cooldown_opt.is_some() {
                let pulse = (current_time * 5.0).sin().abs();
                sprite.color = Color::srgb(0.5 + 0.5 * pulse, 0.0, 1.0 - 0.5 * pulse);
            } else {
                if stats.species_id == 0 {
                    // Sheep
                    if hunger.0 > 90.0 { sprite.color = Color::srgb(1.0, 0.0, 0.0); }
                    else if hunger.0 > 50.0 { sprite.color = Color::srgb(1.0, 1.0, 0.0); }
                    else { sprite.color = Color::srgb(1.0, 1.0, 1.0); }
                } else {
                    // Wolf
                    if hunger.0 > 90.0 { sprite.color = Color::srgb(1.0, 0.0, 0.0); }
                    else if hunger.0 > 50.0 { sprite.color = Color::srgb(0.8, 0.4, 0.0); }
                    else { sprite.color = Color::srgb(0.4, 0.2, 0.1); }
                }
            }
        }

        // 3. Cooldown
        if let Some(ref mut timer) = cooldown_opt {
            timer.0.tick(time.delta());
            if timer.0.is_finished() { commands.entity(entity).remove::<ReproductionCooldown>(); }
        }

        // 4. Starvation
        if hunger.0 >= 100.0 {
            commands.entity(entity).insert(Dead);

            // say what creature starved in a println
            if stats.species_id == 0 {
                println!("A sheep has starved to death!");
            } else {
                println!("A wolf has starved to death!");
            }
        }
    }
}

// SYSTEM 2: Handling Eating (Interactions with Plants)
fn creature_eating(
    mut commands: Commands,
    cfg: Res<SimulationConfig>,
    mut q_creatures: Query<(Entity, &GridPosition, &mut Hunger, &CreatureStats, &CreatureBehavior, &Age, Option<&Digesting>), (With<Creature>, Without<Dead>)>,
    q_plants: Query<(Entity, &GridPosition), (With<Plant>, Without<Dead>)>,
    q_all_creatures: Query<(Entity, &GridPosition, &CreatureStats), (With<Creature>, Without<Dead>)>,
) {
    for (plant_entity, plant_pos) in q_plants.iter() {
        for (my_entity, my_pos, mut my_hunger, my_stats, my_behavior, my_age, digesting) in q_creatures.iter_mut() {
            if digesting.is_some() { continue; }

            if my_pos.x != plant_pos.x || my_pos.y != plant_pos.y {
                continue;
            }

            let is_sheep = my_stats.species_id == 0;
            let is_wolf = my_stats.species_id == 1;

            // Sheep can always eat plants (existing behavior)
            // Wolves can eat plants only if:
            // - baby wolf OR hunger <= 30 OR very low health (hunger >= 70)
            let wolf_can_eat_plant = !my_age.is_adult || my_hunger.0 <= 30.0 || my_hunger.0 >= 70.0;

            if is_wolf && !wolf_can_eat_plant {
                continue;
            }

            if my_pos.x == plant_pos.x && my_pos.y == plant_pos.y {
                // Full check (keep it: no point eating if already essentially full)
                if my_hunger.0 < cfg.eat_skip_if_hunger_below { continue; }

                // Altruism only applies to sheep (wolves ignore altruism)
                if is_sheep {
                    let mut should_eat = true;
                    if my_behavior.altruistic && my_hunger.0 < 20.0 {
                        for (other_entity, other_pos, other_stats) in q_all_creatures.iter() {
                            if my_entity == other_entity { continue; }
                            let dist = (my_pos.x - other_pos.x).abs() + (my_pos.y - other_pos.y).abs();
                            if other_stats.species_id == my_stats.species_id && dist <= my_stats.sight_range {
                                should_eat = false;
                                break;
                            }
                        }
                    }
                    if !should_eat { continue; }
                }

                // Eat plant
                my_hunger.0 = 0.0;
                commands.entity(plant_entity).insert(Dead);

                // If wolf: apply 2-tick berry stun
                if my_stats.species_id == 1 {
                    let stun_seconds = cfg.base_move_seconds * (cfg.wolf_berry_stun_ticks as f32);
                    commands.entity(my_entity).insert(BerryStun(Timer::from_seconds(stun_seconds, TimerMode::Once)));
                }

                // Spawn Exhausted Soil (existing)
                let tile_w = cfg.tile_w;
                let tile_h = cfg.tile_h;
                let screen_x = (my_pos.x - my_pos.y) as f32 * (tile_w / 2.0);
                let screen_y = (my_pos.x + my_pos.y) as f32 * (tile_h / 2.0);

                commands.spawn((
                    Sprite::from_color(Color::srgb(0.5, 0.25, 0.0), Vec2::new(10.0, 40.0)),
                    Transform::from_xyz(screen_x, screen_y, 0.1).with_rotation(Quat::from_rotation_z(0.785)),
                    ExhaustedSoil(Timer::from_seconds(cfg.soil_exhaust_seconds_after_eat, TimerMode::Once)),
                    GridPosition { x: my_pos.x, y: my_pos.y },
                ));

                break; // plant eaten
            }
        }
    }
}

// SYSTEM 3: Handling Reproduction (Interactions with other Creatures)
// We use 'iter_combinations' to check every unique pair of creatures safely
fn creature_reproduction(
    mut commands: Commands,
    cfg: Res<SimulationConfig>,
    mut pop: ResMut<PopulationStats>,
    q_creatures: Query<(Entity, &GridPosition, &Age, &CreatureStats, &CreatureBehavior, Option<&ReproductionCooldown>, Option<&Digesting>, Option<&Overfed>), (With<Creature>, Without<Dead>)>,
) {
    for [(entity_a, pos_a, age_a, stats_a, behavior_a, cooldown_a, digest_a, fed_a),
    (entity_b, pos_b, age_b, stats_b, _,          cooldown_b, digest_b, fed_b)] in q_creatures.iter_combinations()
    {
        if !age_a.is_adult || !age_b.is_adult { continue; }
        if cooldown_a.is_some() || cooldown_b.is_some() { continue; }
        if digest_a.is_some() || fed_a.is_some() { continue; }
        if digest_b.is_some() || fed_b.is_some() { continue; }
        if stats_a.species_id != stats_b.species_id { continue; }

        let dist = (pos_a.x - pos_b.x).abs() + (pos_a.y - pos_b.y).abs();
        if dist > 1 { continue; }

        let sid = stats_a.species_id;
        let sc = cfg.s(sid);

        if rand::random::<f32>() < sc.reproduction_chance {
            // stats bump
            let entry = pop.species.entry(sid).or_default();
            entry.born += 1;
            entry.total_ever += 1;

            // spawn baby (unchanged except timing knobs if you want)
            let baby_x = pos_a.x;
            let baby_y = pos_a.y;

            let tile_w = cfg.tile_w;
            let tile_h = cfg.tile_h;
            let screen_x = (baby_x - baby_y) as f32 * (tile_w / 2.0);
            let screen_y = (baby_x + baby_y) as f32 * (tile_h / 2.0);

            commands.spawn((
                Sprite::from_color(Color::srgb(1.0, 1.0, 1.0), Vec2::new(10.0, 10.0)),
                Transform::from_xyz(screen_x, screen_y, 2.0),
                Creature,
                GridPosition { x: baby_x, y: baby_y },
                MoveTimer(Timer::from_seconds(cfg.base_move_seconds, TimerMode::Repeating)),
                Hunger(0.0),
                CreatureStats { sight_range: sc.sight_range, species_id: sid },
                CreatureBehavior { scared_of_water: behavior_a.scared_of_water, altruistic: behavior_a.altruistic },
                Age { seconds_alive: 0.0, is_adult: false },
                History { last_x: baby_x, last_y: baby_y },
            ));

            // CONFIG: per-species cooldown
            let cd = sc.reproduction_cooldown_seconds;
            commands.entity(entity_a).insert(ReproductionCooldown(Timer::from_seconds(cd, TimerMode::Once)));
            commands.entity(entity_b).insert(ReproductionCooldown(Timer::from_seconds(cd, TimerMode::Once)));
        }
    }
}

fn reaper_system(
    mut commands: Commands,
    q_dead: Query<Entity, With<Dead>>,
) {
    for entity in q_dead.iter() {
        // Despawn safely. If it's already gone, this won't crash
        // because we are iterating existing entities.
        commands.entity(entity).despawn();
    }
}

fn handle_exhaustion(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut ExhaustedSoil)>,
) {
    for (entity, mut exhausted) in query.iter_mut() {
        // Tick the timer
        exhausted.0.tick(time.delta());

        // If time is up, remove the Brown X
        if exhausted.0.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

fn update_stats_ui(
    time: Res<Time>,
    mut stats: ResMut<GameStats>,
    q_creatures: Query<&Creature, Without<Dead>>,
    q_plants: Query<&Plant, Without<Dead>>,
    q_exhausted: Query<&ExhaustedSoil>,
    mut q_text: Query<&mut Text, With<StatsText>>,
) {
    // 1. Update Days
    let dt = time.delta().as_secs_f32();
    stats.days += dt / 10.0;

    // 2. Calculate FPS (Frames Per Second)
    // Avoid division by zero
    let fps = if dt > 0.0 { 1.0 / dt } else { 0.0 };

    // 3. Count Entities
    let creature_count = q_creatures.iter().count();
    let plant_count = q_plants.iter().count();
    let exhausted_count = q_exhausted.iter().count();

    // 4. Update Text
    for mut text in q_text.iter_mut() {
        **text = format!(
            "FPS: {:.0}\nDays: {:.1}\nCreatures: {}\nPlants: {}\nExhausted Soil: {}",
            fps, stats.days, creature_count, plant_count, exhausted_count
        );
    }
}

fn update_species_stats_ui(
    pop: Res<PopulationStats>,
    q_creatures: Query<&CreatureStats, (With<Creature>, Without<Dead>)>,

    mut text_params: ParamSet<(
        Query<&mut Text, With<SpeciesStatsSheepText>>,
        Query<&mut Text, With<SpeciesStatsWolfText>>,
    )>,
) {
    let mut sheep_current: u32 = 0;
    let mut wolf_current: u32 = 0;

    for stats in q_creatures.iter() {
        match stats.species_id {
            0 => sheep_current += 1,
            1 => wolf_current += 1,
            _ => {}
        }
    }

    let sheep_counters = pop.species.get(&0).copied().unwrap_or_default();
    let wolf_counters = pop.species.get(&1).copied().unwrap_or_default();

    // Sheep column text
    for mut t in text_params.p0().iter_mut() {
        **t = format!(
            "Born: {}\nCurrent: {}\nTotal Ever: {}",
            sheep_counters.born, sheep_current, sheep_counters.total_ever
        );
    }

    // Wolf column text
    for mut t in text_params.p1().iter_mut() {
        **t = format!(
            "Born: {}\nCurrent: {}\nTotal Ever: {}",
            wolf_counters.born, wolf_current, wolf_counters.total_ever
        );
    }
}


fn setup_chart(mut commands: Commands) {
    // Container Node (Top Right)
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            right: Val::Px(10.0),
            width: Val::Px(150.0),
            padding: UiRect::all(Val::Px(10.0)),
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .insert(BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)))
        .with_children(|parent| {
            // Header 1: Health
            parent.spawn((
                Text::new("Health Status"),
                TextFont { font_size: 16.0, ..default() },
                TextColor(Color::srgb(1.0, 1.0, 1.0)),
            ));

            // Row 1: Healthy (White)
            parent.spawn(Node { margin: UiRect::top(Val::Px(5.0)), ..default() }).with_children(|row| {
                row.spawn((Node { width: Val::Px(10.0), height: Val::Px(10.0), ..default() }, BackgroundColor(Color::srgb(1.0, 1.0, 1.0))));
                row.spawn((Text::new(" Healthy: 0"), TextFont { font_size: 14.0, ..default() }, ChartTextHealthy));
            });

            // Row 2: Hungry (Yellow)
            parent.spawn(Node { margin: UiRect::top(Val::Px(5.0)), ..default() }).with_children(|row| {
                row.spawn((Node { width: Val::Px(10.0), height: Val::Px(10.0), ..default() }, BackgroundColor(Color::srgb(1.0, 1.0, 0.0))));
                row.spawn((Text::new(" Hungry: 0"), TextFont { font_size: 14.0, ..default() }, ChartTextHungry));
            });

            // Row 3: Critical (Red)
            parent.spawn(Node { margin: UiRect::top(Val::Px(5.0)), ..default() }).with_children(|row| {
                row.spawn((Node { width: Val::Px(10.0), height: Val::Px(10.0), ..default() }, BackgroundColor(Color::srgb(1.0, 0.0, 0.0))));
                row.spawn((Text::new(" Critical: 0"), TextFont { font_size: 14.0, ..default() }, ChartTextCritical));
            });

            // --- SPACER ---
            parent.spawn(Node { height: Val::Px(15.0), ..default() });

            // Header 2: Demographics
            parent.spawn((
                Text::new("Demographics"),
                TextFont { font_size: 16.0, ..default() },
                TextColor(Color::srgb(1.0, 1.0, 1.0)),
            ));

            // Row 4: Adults (Gray Box)
            parent.spawn(Node { margin: UiRect::top(Val::Px(5.0)), ..default() }).with_children(|row| {
                row.spawn((Node { width: Val::Px(10.0), height: Val::Px(10.0), ..default() }, BackgroundColor(Color::srgb(0.7, 0.7, 0.7))));
                row.spawn((Text::new(" Adults: 0"), TextFont { font_size: 14.0, ..default() }, ChartTextAdults));
            });

            // Row 5: Babies (Small White Box)
            parent.spawn(Node { margin: UiRect::top(Val::Px(5.0)), ..default() }).with_children(|row| {
                row.spawn((Node { width: Val::Px(6.0), height: Val::Px(6.0), margin: UiRect::all(Val::Px(2.0)), ..default() }, BackgroundColor(Color::srgb(1.0, 1.0, 1.0))));
                row.spawn((Text::new(" Babies: 0"), TextFont { font_size: 14.0, ..default() }, ChartTextBabies));
            });
        });
}

fn update_chart_ui(
    q_creatures: Query<(&Hunger, &Age), (With<Creature>, Without<Dead>)>,

    // FIX: ParamSet lets us borrow &mut Text multiple times safely
    mut text_params: ParamSet<(
        Query<&mut Text, With<ChartTextHealthy>>,
        Query<&mut Text, With<ChartTextHungry>>,
        Query<&mut Text, With<ChartTextCritical>>,
        Query<&mut Text, With<ChartTextAdults>>,
        Query<&mut Text, With<ChartTextBabies>>,
    )>,
) {
    let mut healthy = 0;
    let mut hungry = 0;
    let mut critical = 0;
    let mut adults = 0;
    let mut babies = 0;

    for (hunger, age) in q_creatures.iter() {
        if hunger.0 > 90.0 {
            critical += 1;
        } else if hunger.0 > 50.0 {
            hungry += 1;
        } else {
            healthy += 1;
        }

        if age.is_adult {
            adults += 1;
        } else {
            babies += 1;
        }
    }

    // Access p0, p1, p2... matching the order in the ParamSet above

    // 1. Healthy
    for mut text in text_params.p0().iter_mut() {
        **text = format!(" Healthy: {}", healthy);
    }

    // 2. Hungry
    for mut text in text_params.p1().iter_mut() {
        **text = format!(" Hungry: {}", hungry);
    }

    // 3. Critical
    for mut text in text_params.p2().iter_mut() {
        **text = format!(" Critical: {}", critical);
    }

    // 4. Adults
    for mut text in text_params.p3().iter_mut() {
        **text = format!(" Adults: {}", adults);
    }

    // 5. Babies
    for mut text in text_params.p4().iter_mut() {
        **text = format!(" Babies: {}", babies);
    }
}

fn predator_hunting_system(
    mut commands: Commands,
    mut q_wolves: Query<(Entity, &GridPosition, &mut Hunger, &CreatureStats, &Age), (With<Creature>, Without<Dead>)>,
    q_sheep: Query<(Entity, &GridPosition, &CreatureStats), (With<Creature>, Without<Dead>)>,
) {
    for (wolf_entity, wolf_pos, mut wolf_hunger, wolf_stats, wolf_age) in q_wolves.iter_mut() {
        if wolf_stats.species_id != 1 { continue; }
        if !wolf_age.is_adult { continue; } // <-- BABIES CAN'T ATTACK

        for (sheep_entity, sheep_pos, sheep_stats) in q_sheep.iter() {
            if sheep_stats.species_id != 0 { continue; }

            if wolf_pos.x == sheep_pos.x && wolf_pos.y == sheep_pos.y {
                // Gorge + digest (existing)
                wolf_hunger.0 = -5.0;
                commands.entity(wolf_entity).insert(Digesting);
                commands.entity(sheep_entity).insert(Dead);

                // Blood FX (existing)
                let screen_x = (wolf_pos.x - wolf_pos.y) as f32 * (TILE_WIDTH / 2.0);
                let screen_y = (wolf_pos.x + wolf_pos.y) as f32 * (TILE_HEIGHT / 2.0);
                commands.spawn((
                    Sprite::from_color(Color::srgb(0.8, 0.0, 0.0), Vec2::new(10.0, 40.0)),
                    Transform::from_xyz(screen_x, screen_y, 0.1).with_rotation(Quat::from_rotation_z(0.785)),
                    ExhaustedSoil(Timer::from_seconds(30.0, TimerMode::Once)),
                    GridPosition { x: wolf_pos.x, y: wolf_pos.y },
                ));

                println!("Wolf is gorging!");
                break;
            }
        }
    }
}

fn setup_debug_panel(mut commands: Commands) {
    commands.insert_resource(TextBoxFocus::default());

    commands
        .spawn((
            DebugPanelRoot,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(10.0),
                bottom: Val::Px(10.0),
                width: Val::Px(380.0),
                padding: UiRect::all(Val::Px(10.0)),
                row_gap: Val::Px(10.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
        ))
        .with_children(|p| {
            p.spawn((
                Text::new("Debug Controls (F1)"),
                TextFont { font_size: 18.0, ..default() },
                TextColor(Color::srgb(1.0, 1.0, 1.0)),
            ));

            // --- Row: Plant spawn chance slider ---
            debug_slider_row(
                p,
                "Plant Spawn Chance",
                ConfigField::PlantSpawnChance,
                0.0,
                0.25,
            );

            // --- Row: Sheep start count textbox ---
            debug_textbox_row(p, "Sheep Start Count", ConfigField::SheepStartCount);

            // --- Row: Wolf start count textbox ---
            debug_textbox_row(p, "Wolf Start Count", ConfigField::WolfStartCount);

            // --- Row: Sheep adult seconds slider ---
            debug_slider_row(
                p,
                "Sheep Adult Seconds",
                ConfigField::SheepAdultSeconds,
                1.0,
                60.0,
            );

            // --- Row: Wolf adult seconds slider ---
            debug_slider_row(
                p,
                "Wolf Adult Seconds",
                ConfigField::WolfAdultSeconds,
                1.0,
                60.0,
            );
        });
}

fn debug_slider_row(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    field: ConfigField,
    min: f32,
    max: f32,
) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                Text::new(label),
                TextFont { font_size: 14.0, ..default() },
                TextColor(Color::srgb(1.0, 1.0, 1.0)),
            ));

            row.spawn((
                Node {
                    height: Val::Px(24.0),
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(10.0),
                    align_items: AlignItems::Center,
                    ..default()
                },
            ))
                .with_children(|line| {
                    // Track
                    line.spawn((
                        Slider { field, min, max },
                        Node {
                            width: Val::Px(220.0),
                            height: Val::Px(10.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.2, 0.2, 0.2)),
                        Interaction::default(),
                    ))
                        .with_children(|track| {
                            // Knob
                            track.spawn((
                                SliderKnob { field },
                                Node {
                                    position_type: PositionType::Absolute,
                                    left: Val::Px(0.0),
                                    top: Val::Px(-4.0),
                                    width: Val::Px(12.0),
                                    height: Val::Px(18.0),
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.8, 0.8, 0.8)),
                            ));
                        });

                    // Value text
                    line.spawn((
                        SliderValueText { field },
                        Text::new("0.00"),
                        TextFont { font_size: 14.0, ..default() },
                        TextColor(Color::srgb(1.0, 1.0, 1.0)),
                    ));
                });
        });
}

fn debug_textbox_row(parent: &mut ChildSpawnerCommands, label: &str, field: ConfigField) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                Text::new(label),
                TextFont { font_size: 14.0, ..default() },
                TextColor(Color::srgb(1.0, 1.0, 1.0)),
            ));

            row.spawn((
                TextBox { field },
                Node {
                    width: Val::Px(140.0),
                    height: Val::Px(26.0),
                    padding: UiRect::horizontal(Val::Px(6.0)),
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Color::srgb(0.15, 0.15, 0.15)),
                Interaction::default(),
            ))
                .with_children(|tb| {
                    tb.spawn((
                        TextBoxText { field },
                        Text::new(""),
                        TextFont { font_size: 14.0, ..default() },
                        TextColor(Color::srgb(1.0, 1.0, 1.0)),
                    ));
                });
        });
}

fn toggle_debug_panel(keys: Res<ButtonInput<KeyCode>>, mut cfg: ResMut<SimulationConfig>) {
    if keys.just_pressed(KeyCode::F1) {
        cfg.debug_panel_enabled = !cfg.debug_panel_enabled;
    }
}

fn debug_panel_visibility(
    cfg: Res<SimulationConfig>,
    mut q: Query<&mut Visibility, With<DebugPanelRoot>>,
) {
    // In your Bevy build, single_mut() returns Result<Mut<_>, QuerySingleError>
    let Ok(mut v) = q.single_mut() else { return; };

    *v = if cfg.debug_panel_enabled {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
}

// ---- Slider behavior: click+drag on track ----
fn val_to_px(v: Val) -> Option<f32> {
    match v {
        Val::Px(px) => Some(px),
        _ => None, // Percent/Vw/Vh/Auto etc. not handled here
    }
}

fn debug_slider_system(
    q_window: Query<&Window, With<PrimaryWindow>>,
    mut cfg: ResMut<SimulationConfig>,
    mouse: Res<ButtonInput<MouseButton>>,

    mut params: ParamSet<(
        Query<(
            &GlobalTransform,
            &ComputedNode,
            &Node,
            &Slider,
            &Interaction,
            &Children,
        )>,
        Query<(&mut Node, &SliderKnob)>,
        Query<(&mut Text, &SliderValueText)>,
    )>,
) {
    if !cfg.debug_panel_enabled {
        return;
    }

    let Ok(window) = q_window.single() else { return; };
    let Some(cursor) = window.cursor_position() else { return; };

    // Update value texts
    for (mut t, tag) in params.p2().iter_mut() {
        let val = get_field_f32(&cfg, tag.field);
        **t = match tag.field {
            ConfigField::PlantSpawnChance => format!("{:.3}", val),
            ConfigField::SheepAdultSeconds | ConfigField::WolfAdultSeconds => format!("{:.1}", val),
            _ => format!("{:.2}", val),
        };
    }

    let track_width_px = |node: &Node| -> f32 {
        match node.width {
            Val::Px(px) => px,
            _ => 220.0,
        }
    };

    // -------- Pass 1: snapshot all knob updates we want to apply --------
    // (child_entity, slider_field, left_px)
    let mut knob_updates: Vec<(Entity, ConfigField, f32)> = Vec::new();

    // If not dragging, we sync ALL knobs to cfg
    if !mouse.pressed(MouseButton::Left) {
        for (_gt, _computed, node, slider, _interaction, children) in params.p0().iter() {
            let width_px = track_width_px(node).max(1.0);
            let val = get_field_f32(&cfg, slider.field);
            let t = ((val - slider.min) / (slider.max - slider.min)).clamp(0.0, 1.0);
            let left_px = t * (width_px - 12.0);

            for child in children.iter() {
                knob_updates.push((child, slider.field, left_px));
            }
        }
    } else {
        // Dragging: only update pressed track(s)
        for (gt, _computed, node, slider, interaction, children) in params.p0().iter() {
            if *interaction != Interaction::Pressed {
                continue;
            }

            let width_px = track_width_px(node).max(1.0);
            let center = gt.translation().truncate();
            let min_x = center.x - (width_px * 0.5);
            let max_x = center.x + (width_px * 0.5);

            let t = ((cursor.x - min_x) / (max_x - min_x)).clamp(0.0, 1.0);
            let new_val = slider.min + t * (slider.max - slider.min);
            set_field_f32(&mut cfg, slider.field, new_val);

            let left_px = t * (width_px - 12.0);
            for child in children.iter() {
                knob_updates.push((child, slider.field, left_px));
            }
        }
    }

    // -------- Pass 2: apply knob updates (now we can mutably borrow p1 safely) --------
    {
        let mut q_knob = params.p1();
        for (child_entity, field, left_px) in knob_updates {
            if let Ok((mut knob_node, knob)) = q_knob.get_mut(child_entity) {
                if knob.field == field {
                    knob_node.left = Val::Px(left_px);
                }
            }
        }
    }
}

// ---- Textbox behavior: click focus + type + Enter commit ----
fn debug_textbox_system(
    mut cfg: ResMut<SimulationConfig>,
    mut focus: ResMut<TextBoxFocus>,
    keys: Res<ButtonInput<KeyCode>>,
    mut key_evr: MessageReader<KeyboardInput>,
    mut q_tb: Query<(&TextBox, &Interaction, &Children)>,
    mut q_text: Query<(&mut Text, &TextBoxText)>,
) {
    if !cfg.debug_panel_enabled { return; }

    // handle clicks to set focus
    for (tb, interaction, children) in q_tb.iter_mut() {
        if *interaction == Interaction::Pressed {
            focus.active = Some(tb.field);
            focus.buffer.clear();

            // seed buffer with current value
            match tb.field {
                ConfigField::SheepStartCount => focus.buffer = cfg.s(0).starting_count.to_string(),
                ConfigField::WolfStartCount => focus.buffer = cfg.s(1).starting_count.to_string(),
                _ => {}
            }

            // update visible text immediately
            for child in children.iter() {
                if let Ok((mut t, tag)) = q_text.get_mut(child) {
                    if tag.field == tb.field {
                        **t = focus.buffer.clone();
                    }
                }
            }
        }
    }

    // If no active textbox, still keep display updated from cfg
    if focus.active.is_none() {
        for (mut t, tag) in q_text.iter_mut() {
            **t = match tag.field {
                ConfigField::SheepStartCount => cfg.s(0).starting_count.to_string(),
                ConfigField::WolfStartCount => cfg.s(1).starting_count.to_string(),
                _ => "".to_string(),
            };
        }
        return;
    }

    let active = focus.active.unwrap();

    // typing
    for ev in key_evr.read() {
        if !ev.state.is_pressed() {
            continue;
        }

        if let Key::Character(ref s) = ev.logical_key {
            for c in s.chars() {
                if c.is_ascii_digit() {
                    focus.buffer.push(c);
                }
            }
        }
    }


    // backspace
    if keys.just_pressed(KeyCode::Backspace) {
        focus.buffer.pop();
    }

    // cancel
    if keys.just_pressed(KeyCode::Escape) {
        focus.active = None;
        focus.buffer.clear();
        return;
    }

    // commit
    if keys.just_pressed(KeyCode::Enter) {
        if let Ok(v) = focus.buffer.parse::<u32>() {
            match active {
                ConfigField::SheepStartCount => cfg.s_mut(0).starting_count = v.clamp(0, 200),
                ConfigField::WolfStartCount => cfg.s_mut(1).starting_count = v.clamp(0, 200),
                _ => {}
            }
        }
        focus.active = None;
        focus.buffer.clear();
        return;
    }

    // update visible text for active box
    for (mut t, tag) in q_text.iter_mut() {
        if tag.field == active {
            **t = focus.buffer.clone();
        }
    }
}

// =========================
// 5) FIELD GET/SET HELPERS (for sliders)
// =========================
fn get_field_f32(cfg: &SimulationConfig, field: ConfigField) -> f32 {
    match field {
        ConfigField::PlantSpawnChance => cfg.plant_spawn_chance_per_tick,
        ConfigField::SheepAdultSeconds => cfg.s(0).adult_seconds,
        ConfigField::WolfAdultSeconds => cfg.s(1).adult_seconds,
        _ => 0.0,
    }
}

fn set_field_f32(cfg: &mut SimulationConfig, field: ConfigField, val: f32) {
    match field {
        ConfigField::PlantSpawnChance => cfg.plant_spawn_chance_per_tick = val.clamp(0.0, 1.0),
        ConfigField::SheepAdultSeconds => cfg.s_mut(0).adult_seconds = val.clamp(1.0, 600.0),
        ConfigField::WolfAdultSeconds => cfg.s_mut(1).adult_seconds = val.clamp(1.0, 600.0),
        _ => {}
    }
}
