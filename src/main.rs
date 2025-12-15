use bevy::prelude::*;
use bevy::window::PrimaryWindow; // Needed to get mouse coordinates
use std::collections::HashMap;

// Constants
const TILE_WIDTH: f32 = 64.0;
const TILE_HEIGHT: f32 = 32.0;
const MAP_SIZE: i32 = 20;

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
        // A1) Guaranteed resource init (fixes the panic)
        .insert_resource(PopulationStats::default())
        .insert_resource(GameStats { days: 0.0 }) // optional: also move GameStats here if you want

        // B) Chain Startup systems so they're ordered
        .add_systems(Startup, (setup, spawn_map, setup_chart).chain())

        .add_systems(Update, (
            cursor_system,
            move_creatures,
            sync_creature_visuals,
            plant_growth_system,
            handle_drowning,
            reaper_system,
            handle_exhaustion,
            update_stats_ui,
            update_species_stats_ui, // <-- make sure this is added
            update_chart_ui,
            creature_state_update,
            creature_eating,
            predator_hunting_system,
            creature_reproduction,
        ))
        .run();
}

fn setup(mut commands: Commands) {
    // 1. Initialize Game Stats Resource (Day 0)
    //commands.insert_resource(GameStats { days: 0.0 });

    // NEW: Init population stats
    //commands.insert_resource(PopulationStats::default());

    // 2. Spawn Camera
    let mut camera_transform = Transform::from_xyz(0.0, 0.0, 1000.0);
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

fn spawn_map(mut commands: Commands, mut stats: ResMut<PopulationStats>) {
    // 1. Spawn Ground
    for x in -MAP_SIZE..MAP_SIZE {
        for y in -MAP_SIZE..MAP_SIZE {
            let screen_x = (x - y) as f32 * (TILE_WIDTH / 2.0);
            let screen_y = (x + y) as f32 * (TILE_HEIGHT / 2.0);
            commands.spawn((
                Sprite::from_color(
                    Color::srgb(0.3, 0.5, 0.3),
                    Vec2::new(TILE_WIDTH - 2.0, TILE_HEIGHT - 2.0),
                ),
                Transform::from_xyz(screen_x, screen_y, 0.0),
                Tile { x, y },
            ));
        }
    }

    // Spawn Sheep (start as babies; count as born + total ever)
    for i in 0..8 {
        let species_id = 0_u32;
        let entry = stats.species.entry(species_id).or_default();
        entry.born += 1;
        entry.total_ever += 1;

        commands.spawn((
            Sprite::from_color(Color::srgb(1.0, 1.0, 1.0), Vec2::new(20.0, 20.0)),
            Transform::from_xyz(0.0, 0.0, 2.0),
            Creature,
            GridPosition { x: i, y: i },
            MoveTimer(Timer::from_seconds(0.2, TimerMode::Repeating)),
            Hunger(0.0),
            CreatureStats { sight_range: 8, species_id },
            CreatureBehavior { scared_of_water: true, altruistic: true },
            Age { seconds_alive: 0.0, is_adult: false }, // <-- BABY
            History { last_x: i, last_y: i },
        ));
    }

    // Spawn Wolves (start as babies; count as born + total ever)
    let wolf_coords = vec![(-5, -5), (5, -5)];
    for (wx, wy) in wolf_coords {
        let species_id = 1_u32;
        let entry = stats.species.entry(species_id).or_default();
        entry.born += 1;
        entry.total_ever += 1;

        commands.spawn((
            Sprite::from_color(Color::srgb(0.4, 0.2, 0.1), Vec2::new(22.0, 22.0)),
            Transform::from_xyz(0.0, 0.0, 2.0),
            Creature,
            GridPosition { x: wx, y: wy },
            MoveTimer(Timer::from_seconds(0.2, TimerMode::Repeating)),
            Hunger(0.0),
            CreatureStats { sight_range: 10, species_id },
            CreatureBehavior { scared_of_water: true, altruistic: false },
            Age { seconds_alive: 0.0, is_adult: false }, // <-- BABY
            History { last_x: wx, last_y: wy },
        ));
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
            &Age
        ), (With<Creature>, Without<Dead>)>,
        Query<&GridPosition, With<Plant>>,
        Query<&GridPosition, With<Water>>,
    )>,
) {
    // 1) Creature snapshot now includes adulthood
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

    // 2) Plant + water snapshots
    let plant_positions: Vec<(i32, i32)> = param_set.p2().iter().map(|p| (p.x, p.y)).collect();
    let water_tiles: Vec<(i32, i32)> = param_set.p3().iter().map(|p| (p.x, p.y)).collect();

    // 3) Mutate creatures
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
        // --- BERRY STUN LOGIC ---
        if let Some(mut stun) = berry_stun {
            stun.0.tick(time.delta());

            if !stun.0.is_finished() {
                // Immobilized for berry digestion
                continue;
            } else {
                // Stun over
                commands.entity(my_entity).remove::<BerryStun>();
            }
        }

        // Timer logic
        let base_duration = 0.2;
        let mut target_duration = if cooldown.is_some() { 0.5 } else { base_duration };
        if overfed.is_some() {
            target_duration = base_duration * 6.6;
        }
        timer.0.set_duration(std::time::Duration::from_secs_f32(target_duration));
        timer.0.tick(time.delta());

        if digesting.is_some() || !timer.0.is_finished() {
            continue;
        }

        let old_x = my_pos.x;
        let old_y = my_pos.y;

        // === TARGET SELECTION ===
        let mut target_pos: Option<(i32, i32)> = None;

        // target_type:
        // 1 = fruit (plant), 2 = mate, 3 = prey (sheep), 4 = predator (wolf)
        let mut target_type: i32 = 0;
        let mut target_weight: i32 = 20; // multiplier for distance delta scoring

        let is_sheep = my_stats.species_id == 0;
        let is_wolf = my_stats.species_id == 1;

        let hunger_level = my_hunger.0;
        let is_full = hunger_level <= 10.0;

        let can_breed = my_age.is_adult && cooldown.is_none() && digesting.is_none() && overfed.is_none();

        // ---------- SHEEP LOGIC ----------
        if is_sheep {
            // Seek mate if full and ready
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

            // Seek food if hungry
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

        // ---------- PREDATOR / PREY OVERRIDE WITH BABY RULES ----------
        //
        // Babies can't attack:
        // - Wolf babies do not target sheep as prey.
        //
        // Not scared of other babies:
        // - Sheep do NOT treat wolf babies as predators (no flee target).
        let mut threat_dist = 9999;

        // Track best prey target for wolves
        let mut best_prey: Option<(i32, i32, i32)> = None; // (x, y, dist)

        // Track best predator threat for sheep
        let mut best_predator: Option<(i32, i32, i32)> = None;

        for other in &creature_targets {
            if my_entity == other.entity { continue; }
            let dist = (my_pos.x - other.x).abs() + (my_pos.y - other.y).abs();
            if dist >= my_stats.sight_range { continue; }

            // Wolves hunting sheep (only if THIS wolf is adult; prey can be adult or baby)
            if is_wolf && other.species == 0 {
                if my_age.is_adult {
                    if best_prey.map(|(_,_,d)| dist < d).unwrap_or(true) {
                        best_prey = Some((other.x, other.y, dist));
                    }
                }
            }
            // Sheep fearing wolves (only if the wolf is adult; sheep are not scared of wolf babies)
            else if is_sheep && other.species == 1 {
                if other.is_adult {
                    if best_predator.map(|(_,_,d)| dist < d).unwrap_or(true) {
                        best_predator = Some((other.x, other.y, dist));
                    }
                }
            }

            // "aren't scared of other babies" also implies: if both are babies, never treat as threat/target
            // (handled implicitly above since sheep only fear adult wolves, and wolf babies don't hunt).
        }

        // Apply best predator for sheep
        if let Some((px, py, dist)) = best_predator {
            threat_dist = dist;
            target_pos = Some((px, py));
            target_type = 4;     // flee
            target_weight = 20;
        }

        // Apply best prey for wolves (only if no flee target set)
        if target_type == 0 || target_type == 1 || target_type == 2 {
            if let Some((sx, sy, dist)) = best_prey {
                threat_dist = dist;
                target_pos = Some((sx, sy));
                target_type = 3;     // hunt
                target_weight = 20;  // default hunt weight (can be adjusted below)
            }
        }

        // ---------- WOLF FRUIT FALLBACK + LOW-HEALTH WEIGHTS ----------
        //
        // - Baby wolves can eat fruit.
        // - Adult wolves can eat fruit if hunger <= 30.0.
        // - If wolf is <= 30% health (hunger >= 70.0), prefer fruit more strongly:
        //      fruit weight = 80%  => 80
        //      meat  weight = 50%  => 50
        if is_wolf {
            let can_eat_fruit = !my_age.is_adult || hunger_level <= 30.0 || hunger_level >= 70.0;

            // If wolf is low health, change weights even if meat exists.
            if hunger_level >= 70.0 {
                // If we already have a meat target, reduce its pull (50)
                if target_type == 3 {
                    target_weight = 50;
                }
            }

            // If no meat target found, seek fruit (if allowed)
            if can_eat_fruit && (target_type != 3) {
                let mut best_dist = 9999;
                for &(px, py) in &plant_positions {
                    let dist = (my_pos.x - px).abs() + (my_pos.y - py).abs();
                    if dist > 0 && dist < my_stats.sight_range && dist < best_dist {
                        best_dist = dist;
                        target_pos = Some((px, py));
                        target_type = 1; // fruit
                        // Low health = strong fruit pull
                        target_weight = if hunger_level >= 70.0 { 80 } else { 20 };
                    }
                }
            }
        }

        // === MOVE EVALUATION ===
        let moves = [(0,1), (0,-1), (-1,0), (1,0)];
        let mut best_move = (0, 0);
        let mut best_score = -9999_i32;

        for (dx, dy) in moves {
            let nx = my_pos.x + dx;
            let ny = my_pos.y + dy;

            if nx < -MAP_SIZE || nx >= MAP_SIZE || ny < -MAP_SIZE || ny >= MAP_SIZE {
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
                    // toward fruit/mate/prey
                    1 | 2 | 3 => score -= delta * target_weight,
                    // away from predator
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
    q_tiles: Query<(&Tile, &Sprite), Without<Water>>,
    q_plants: Query<&GridPosition, With<Plant>>,
    // NEW: Check for exhausted soil
    q_exhausted: Query<&GridPosition, With<ExhaustedSoil>>,
) {
    if rand::random::<f32>() < 0.05 {
        let x = (rand::random::<i32>().abs() % (MAP_SIZE * 2)) - MAP_SIZE;
        let y = (rand::random::<i32>().abs() % (MAP_SIZE * 2)) - MAP_SIZE;

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
            let screen_x = (x - y) as f32 * (TILE_WIDTH / 2.0);
            let screen_y = (x + y) as f32 * (TILE_HEIGHT / 2.0);

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
    // The Query includes Option<&Digesting>
    mut q_creatures: Query<(Entity, &mut Hunger, &mut Sprite, &mut Age, Option<&mut ReproductionCooldown>, &CreatureStats, Option<&Digesting>, Option<&mut Overfed>), (With<Creature>, Without<Dead>)>,
) {
    let dt = time.delta().as_secs_f32();
    let current_time = time.elapsed_secs();

    // MAKE SURE 'digesting' IS IN THIS LIST ↓
    for (entity, mut hunger, mut sprite, mut age, mut cooldown_opt, stats, digesting, mut overfed_opt) in q_creatures.iter_mut() {

        // 1. Growth & Size
        age.seconds_alive += dt;
        if !age.is_adult && age.seconds_alive > 20.0 { age.is_adult = true; }

        let base_size = if stats.species_id == 1 { 22.0 } else { 20.0 };
        let target_scale = if age.is_adult { base_size } else { base_size / 2.0 };
        sprite.custom_size = Some(Vec2::new(target_scale, target_scale));

        let burn_rate = if age.is_adult { 3.3 } else { 1.65 };
        let final_burn = if stats.species_id == 1 { burn_rate * 1.5 } else { burn_rate };
        hunger.0 += final_burn * dt;

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
    mut q_creatures: Query<(Entity, &GridPosition, &mut Hunger, &CreatureStats, &CreatureBehavior, &Age, Option<&Digesting>), (With<Creature>, Without<Dead>)>,
    q_plants: Query<(Entity, &GridPosition), (With<Plant>, Without<Dead>)>,
    q_all_creatures: Query<(Entity, &GridPosition, &CreatureStats), (With<Creature>, Without<Dead>)>,
) {
    for (plant_entity, plant_pos) in q_plants.iter() {
        for (my_entity, my_pos, mut my_hunger, my_stats, my_behavior, my_age, digesting) in q_creatures.iter_mut() {
            if digesting.is_some() { continue; }

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
                if my_hunger.0 < 5.0 { continue; }

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
                    // 2 ticks = 2 * base MoveTimer duration (0.2s each)
                    commands.entity(my_entity).insert(BerryStun(
                        Timer::from_seconds(0.4, TimerMode::Once),
                    ));
                }

                // Spawn Exhausted Soil (existing)
                let screen_x = (my_pos.x - my_pos.y) as f32 * (TILE_WIDTH / 2.0);
                let screen_y = (my_pos.x + my_pos.y) as f32 * (TILE_HEIGHT / 2.0);
                commands.spawn((
                    Sprite::from_color(Color::srgb(0.5, 0.25, 0.0), Vec2::new(10.0, 40.0)),
                    Transform::from_xyz(screen_x, screen_y, 0.1).with_rotation(Quat::from_rotation_z(0.785)),
                    ExhaustedSoil(Timer::from_seconds(10.0, TimerMode::Once)),
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
    mut pop: ResMut<PopulationStats>,
    q_creatures: Query<
        (
            Entity,
            &GridPosition,
            &Age,
            &CreatureStats,
            &CreatureBehavior,
            Option<&ReproductionCooldown>,
            Option<&Digesting>,
            Option<&Overfed>,
        ),
        (With<Creature>, Without<Dead>),
    >,
) {
    for [
    (entity_a, pos_a, age_a, stats_a, behavior_a, cooldown_a, digest_a, fed_a),
    (entity_b, pos_b, age_b, stats_b, _, cooldown_b, digest_b, fed_b),
    ] in q_creatures.iter_combinations()
    {
        if !age_a.is_adult || !age_b.is_adult { continue; }
        if cooldown_a.is_some() || cooldown_b.is_some() { continue; }
        if digest_a.is_some() || fed_a.is_some() { continue; }
        if digest_b.is_some() || fed_b.is_some() { continue; }
        if stats_a.species_id != stats_b.species_id { continue; }

        let dist = (pos_a.x - pos_b.x).abs() + (pos_a.y - pos_b.y).abs();
        if dist > 1 { continue; }

        if rand::random::<f32>() < 0.10 {
            let baby_x = pos_a.x;
            let baby_y = pos_a.y;

            let screen_x = (baby_x - baby_y) as f32 * (TILE_WIDTH / 2.0);
            let screen_y = (baby_x + baby_y) as f32 * (TILE_HEIGHT / 2.0);

            let species_id = stats_a.species_id;

            // NEW: stats bump
            let entry = pop.species.entry(species_id).or_default();
            entry.born += 1;
            entry.total_ever += 1;

            commands.spawn((
                Sprite::from_color(Color::srgb(1.0, 1.0, 1.0), Vec2::new(10.0, 10.0)),
                Transform::from_xyz(screen_x, screen_y, 2.0),
                Creature,
                GridPosition { x: baby_x, y: baby_y },
                MoveTimer(Timer::from_seconds(0.2, TimerMode::Repeating)),
                Hunger(0.0),
                CreatureStats { sight_range: stats_a.sight_range, species_id },
                CreatureBehavior { scared_of_water: behavior_a.scared_of_water, altruistic: behavior_a.altruistic },
                Age { seconds_alive: 0.0, is_adult: false },
                History { last_x: baby_x, last_y: baby_y },
            ));

            let cooldown_seconds = if species_id == 0 {
                20.0 // Sheep reproduce faster
            } else {
                40.0 // Wolves reproduce slower
            };

            commands.entity(entity_a).insert(ReproductionCooldown(
                Timer::from_seconds(cooldown_seconds, TimerMode::Once),
            ));
            commands.entity(entity_b).insert(ReproductionCooldown(
                Timer::from_seconds(cooldown_seconds, TimerMode::Once),
            ));

            if species_id == 0 {
                println!("A new sheep is born!");
            } else {
                println!("A new wolf is born!");
            }
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