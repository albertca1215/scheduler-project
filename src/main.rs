// LLM Disclosure: Parts of this application, including comments and dynamic behavior
// state logic, were generated or modified with assistance from a large language model.

use macroquad::prelude::*;
use scheduler_project::entity::Entity;
use scheduler_project::scheduler::{
    BehaviorType, DeadlineType, FixedTickEngine, SchedulerPolicy, Task,
};
use std::collections::VecDeque;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SchedulingKind {
    Fifo,
    Priority,
    RoundRobin,
}

#[derive(Clone)]
struct Hamster {
    name: String,                // Display name and task label.
    entity: Entity,              // Position, velocity, radius, and rendered color.
    collisions: u64,             // Total wall and hamster collisions recorded.
    waypoint: Vec2,              // Random destination used during normal movement.
    normal_speed: f32,           // Speed restored after chase or knockback ends.
    chase_target: Option<usize>, // Target hamster during Chase/Pounce.
    chase_remaining: f32,        // Seconds remaining in the active encounter.
    chase_active: bool,          // Whether an encounter timer is currently running.
    collision_cooldown: f32,     // Short period preventing repeated overlap contacts.
    action: HamsterAction,       // Current behavior selected by the state machine.
    pounce_remaining: f32,       // Delay before Pounce launches.
    knockback_remaining: f32,    // Time remaining before normal behavior returns.
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HamsterAction {
    Normal,
    Chase,
    Pounce,
    Flee,
    Knockback,
}

const CHASE_DURATION: f32 = 5.0; // Encounter timer for hunter and prey.
const CHASE_SPEED_MULTIPLIER: f32 = 1.5; // Chase velocity scale.
const FLEE_SPEED_MULTIPLIER: f32 = 2.0; // Prey velocity scale.
const POUNCE_LAUNCH_SPEED_MULTIPLIER: f32 = 3.5; // Pounce launch velocity scale.
const PUNCH_SPEED_MULTIPLIER: f32 = 6.0; // Initial punch velocity scale.
const KNOCKBACK_DECAY_PER_SECOND: f32 = 0.82; // Exponential knockback decay.
const POOL_BALL_RESTITUTION: f32 = 0.92; // Pool-ball restitution factor.
const MIN_STEP_INTERVAL: f32 = 0.025;
const MAX_STEP_INTERVAL: f32 = 1.0;
const STEP_INTERVAL_CHANGE: f32 = 0.05;
const MAX_CATCH_UP_STEPS: usize = 100;
const COLLISION_COOLDOWN: f32 = 0.75;
const FIXED_SIMULATION_STEP: f32 = 1.0 / 60.0;
const MAX_SCHEDULER_LOG_ENTRIES: usize = 8;
const HUD_HEIGHT: f32 = 240.0;

impl Hamster {
    // Creates one entity with a 75-pixel radius and a random initial waypoint.
    fn new(name: impl Into<String>, position: Vec2, velocity: Vec2) -> Self {
        let radius = 75.0;
        let boosted_velocity = velocity * 1.15;
        Self {
            name: name.into(),
            entity: Entity::new(position, boosted_velocity, radius, WHITE),
            collisions: 0,
            waypoint: random_waypoint(radius),
            normal_speed: boosted_velocity.length(),
            chase_target: None,
            chase_remaining: 0.0,
            chase_active: false,
            collision_cooldown: 0.0,
            action: HamsterAction::Normal,
            pounce_remaining: 0.0,
            knockback_remaining: 0.0,
        }
    }

    // Prevents one physical overlap from counting repeatedly on later steps.
    fn update_collision_cooldown(&mut self, delta_time: f32) {
        self.collision_cooldown = (self.collision_cooldown - delta_time).max(0.0);
    }

    // Returns true only while an encounter timer is active.
    fn is_chasing(&self) -> bool {
        self.chase_active && self.chase_remaining > 0.0
    }

    // Counts down Chase/Flee/Pounce encounter time and resets expired behavior.
    fn update_chase(&mut self, delta_time: f32) {
        if self.chase_remaining > 0.0 {
            self.chase_remaining = (self.chase_remaining - delta_time).max(0.0);
            if self.chase_remaining == 0.0 {
                self.chase_target = None;
                self.chase_active = false;
                self.action = HamsterAction::Normal;
                self.pounce_remaining = 0.0;
            }
        }
    }

    // Counts Pounce setup independently of FIFO task execution, then launches once.
    fn update_pounce(&mut self, target: Option<Vec2>, delta_time: f32) {
        if self.action != HamsterAction::Pounce || self.pounce_remaining <= 0.0 {
            return;
        }

        self.pounce_remaining = (self.pounce_remaining - delta_time).max(0.0);
        self.entity.velocity = Vec2::ZERO;
        if self.pounce_remaining == 0.0 {
            if let Some(target) = target {
                let direction = (target - self.entity.position).normalize_or_zero();
                self.entity.velocity =
                    direction * self.normal_speed * POUNCE_LAUNCH_SPEED_MULTIPLIER;
            }
        }
    }

    // Applies exponential knockback decay and restores normal movement at rest.
    fn update_knockback(&mut self, delta_time: f32) {
        if self.action != HamsterAction::Knockback {
            return;
        }
        self.knockback_remaining = (self.knockback_remaining - delta_time).max(0.0);
        self.entity.velocity *= KNOCKBACK_DECAY_PER_SECOND.powf(delta_time);

        if self.entity.velocity.length() <= self.normal_speed || self.knockback_remaining == 0.0 {
            self.entity.velocity = self.entity.velocity.normalize_or_zero() * self.normal_speed;
            self.action = HamsterAction::Normal;
            self.knockback_remaining = 0.0;
        }
    }

    // Smoothly lerps velocity toward a target at the selected behavior speed.
    fn steer_toward(&mut self, target: Vec2, speed: f32, delta_time: f32) {
        let direction = (target - self.entity.position).normalize_or_zero();
        let desired_velocity = direction * speed;
        let steering = (delta_time * 4.0).clamp(0.0, 1.0);
        self.entity.velocity = self.entity.velocity.lerp(desired_velocity, steering);
    }

    // Dispatches movement for Normal, Chase, Pounce, Flee, and Knockback states.
    fn update_movement(
        &mut self,
        target_position: Option<Vec2>,
        flee_from: Option<Vec2>,
        delta_time: f32,
    ) {
        if self.action == HamsterAction::Pounce {
            if self.pounce_remaining > 0.0 {
                self.entity.velocity = Vec2::ZERO;
            }
            return;
        }

        if self.action == HamsterAction::Knockback {
            return;
        }

        if matches!(self.action, HamsterAction::Normal | HamsterAction::Flee)
            && self.entity.position.distance(self.waypoint) < self.entity.radius
        {
            self.waypoint = random_waypoint(self.entity.radius);
        }

        let (target, speed) = match self.action {
            HamsterAction::Chase => (
                target_position.unwrap_or(self.waypoint),
                self.normal_speed * CHASE_SPEED_MULTIPLIER,
            ),
            HamsterAction::Flee => {
                let away = flee_from
                    .map(|closest| self.entity.position - closest)
                    .filter(|direction| direction.length_squared() > f32::EPSILON)
                    .unwrap_or(vec2(1.0, 0.0))
                    .normalize();
                (
                    self.entity.position + away * 500.0,
                    self.normal_speed * FLEE_SPEED_MULTIPLIER,
                )
            }
            HamsterAction::Normal => (self.waypoint, self.normal_speed),
            HamsterAction::Pounce | HamsterAction::Knockback => (self.entity.position, 0.0),
        };
        self.steer_toward(target, speed, delta_time);
    }
}

// Chooses a reachable point inside the window boundaries for normal steering.
fn random_waypoint(radius: f32) -> Vec2 {
    let max_x = (screen_width() - radius).max(radius);
    let min_y = HUD_HEIGHT + radius;
    let max_y = (screen_height() - radius).max(min_y);
    vec2(
        rand::gen_range(radius, max_x),
        rand::gen_range(min_y, max_y),
    )
}

fn sandbox_position(horizontal: f32, vertical: f32) -> Vec2 {
    let radius = 75.0;
    let top = HUD_HEIGHT + radius;
    let bottom = (screen_height() - radius).max(top);
    vec2(
        screen_width() * horizontal,
        top + (bottom - top) * vertical,
    )
}

// Converts a velocity vector into a readable direction label.
fn movement_direction(velocity: Vec2) -> &'static str {
    match (velocity.x.is_sign_negative(), velocity.y.is_sign_negative()) {
        (false, false) => "right and down",
        (false, true) => "right and up",
        (true, false) => "left and down",
        (true, true) => "left and up",
    }
}

// Adds movement and collision-check work to the active scheduler queue.
fn schedule_frame_tasks(
    engine: &mut FixedTickEngine<SchedulerPolicy>,
    hamsters: &[Hamster],
) {
    let tick = engine.current_tick();
    let tasks_per_cycle = hamsters.len() as u64 * 2;
    let first_task_id = tick * tasks_per_cycle + 1;

    for hamster_index in 0..hamsters.len() {
        let task_offset = (hamster_index * 2) as u64;
        let hamster_index = hamster_index as usize;
        let activity = match hamsters[hamster_index].action {
            HamsterAction::Chase => ("Chase", BehaviorType::Chasing),
            HamsterAction::Pounce => ("Pounce", BehaviorType::Pouncing),
            HamsterAction::Flee => ("Flee", BehaviorType::Fleeing),
            HamsterAction::Knockback => ("Knockback", BehaviorType::Knockback),
            HamsterAction::Normal => ("Move", BehaviorType::Movement),
        };
        engine.add_task(Task::new(
            first_task_id + task_offset,
            format!("{} {}", activity.0, hamsters[hamster_index].name),
            10,
            tick + 1,
            DeadlineType::Hard,
            1,
            activity.1,
        ));
        engine.add_task(Task::new(
            first_task_id + task_offset + 1,
            format!("Check {} collision", hamsters[hamster_index].name),
            1,
            tick + 1,
            DeadlineType::Hard,
            1,
            BehaviorType::CollisionCheck,
        ));
    }
}

// Maps a scheduled task ID to its human-readable FIFO/HUD label.
fn task_description(task_id: u64, hamsters: &[Hamster]) -> String {
    let cycle = hamsters.len() as u64 * 2;
    let residue = task_id % cycle;
    if residue % 2 == 1 {
        let index = ((residue - 1) / 2) as usize;
        if let Some(hamster) = hamsters.get(index) {
            let action = match hamsters[index].action {
                HamsterAction::Chase => format!(
                    "Chasing {}",
                    hamsters[hamsters[index].chase_target.unwrap_or(index)].name
                ),
                HamsterAction::Pounce => "Pouncing".to_string(),
                HamsterAction::Flee => "Fleeing".to_string(),
                HamsterAction::Knockback => "Knocked back".to_string(),
                HamsterAction::Normal => "Movement".to_string(),
            };
            return format!("{}: {}", hamster.name, action);
        }
    }

    if residue == 0 {
        let index = hamsters.len().saturating_sub(1);
        return format!("{}: Collision check", hamsters[index].name);
    }
    if residue % 2 == 0 {
        let index = (residue / 2 - 1) as usize;
        if let Some(hamster) = hamsters.get(index) {
            return format!("{}: Collision check", hamster.name);
        }
    }
    "Unknown task".to_string()
}

fn append_scheduler_log(
    scheduler_log: &mut VecDeque<String>,
    report: &scheduler_project::scheduler::TickReport,
    pending_tasks: usize,
    hamsters: &[Hamster],
) {
    let selected = report
        .selected_task_ids
        .iter()
        .map(|task_id| task_description(*task_id, hamsters))
        .collect::<Vec<_>>()
        .join(", ");
    let completed = report
        .completed_task_ids
        .iter()
        .map(|task_id| task_description(*task_id, hamsters))
        .collect::<Vec<_>>()
        .join(", ");
    let completed = if completed.is_empty() {
        "none"
    } else {
        &completed
    };

    scheduler_log.push_back(format!(
        "t{} | selected: {} | completed: {} | queued: {}",
        report.tick,
        if selected.is_empty() {
            "none"
        } else {
            &selected
        },
        completed,
        pending_tasks,
    ));
    while scheduler_log.len() > MAX_SCHEDULER_LOG_ENTRIES {
        scheduler_log.pop_front();
    }
}

fn draw_wrapped_text(
    text: &str,
    x: f32,
    mut y: f32,
    max_width: f32,
    font_size: f32,
    line_height: f32,
    color: Color,
) -> f32 {
    let mut line = String::new();
    for word in text.split_whitespace() {
        let candidate = if line.is_empty() {
            word.to_string()
        } else {
            format!("{line} {word}")
        };
        if !line.is_empty()
            && measure_text(&candidate, None, font_size as u16, 1.0).width > max_width
        {
            draw_text(&line, x, y, font_size, color);
            y += line_height;
            line = word.to_string();
        } else {
            line = candidate;
        }
    }
    if !line.is_empty() {
        draw_text(&line, x, y, font_size, color);
        y += line_height;
    }
    y
}

// Resolves a wall collision and applies restitution to the reflected velocity.
fn check_collision(entity: &mut Entity) -> bool {
    let mut bounced = false;

    if entity.position.x - entity.radius < 0.0 {
        entity.position.x = entity.radius;
        entity.velocity.x = entity.velocity.x.abs() * POOL_BALL_RESTITUTION;
        bounced = true;
    } else if entity.position.x + entity.radius > screen_width() {
        entity.position.x = screen_width() - entity.radius;
        entity.velocity.x = -entity.velocity.x.abs() * POOL_BALL_RESTITUTION;
        bounced = true;
    }

    if entity.position.y - entity.radius < HUD_HEIGHT {
        entity.position.y = HUD_HEIGHT + entity.radius;
        entity.velocity.y = entity.velocity.y.abs() * POOL_BALL_RESTITUTION;
        bounced = true;
    } else if entity.position.y + entity.radius > screen_height() {
        entity.position.y = screen_height() - entity.radius;
        entity.velocity.y = -entity.velocity.y.abs() * POOL_BALL_RESTITUTION;
        bounced = true;
    }

    bounced
}

// Finds hamster overlaps, separates their centers, and resolves pool-ball impulses.
fn check_collisions(hamsters: &mut [Hamster]) -> Vec<(usize, usize)> {
    let mut collisions = Vec::new();

    for first_index in 0..hamsters.len() {
        for second_index in (first_index + 1)..hamsters.len() {
            let offset =
                hamsters[second_index].entity.position - hamsters[first_index].entity.position;
            let distance = offset.length();
            let minimum_distance =
                hamsters[first_index].entity.radius + hamsters[second_index].entity.radius;

            if distance >= minimum_distance {
                continue;
            }
            if hamsters[first_index].collision_cooldown > 0.0
                || hamsters[second_index].collision_cooldown > 0.0
            {
                continue;
            }

            let normal = if distance > f32::EPSILON {
                offset / distance
            } else {
                vec2(1.0, 0.0)
            };
            let separation = (minimum_distance - distance) / 2.0;
            hamsters[first_index].entity.position -= normal * separation;
            hamsters[second_index].entity.position += normal * separation;

            let first_normal = hamsters[first_index].entity.velocity.dot(normal);
            let second_normal = hamsters[second_index].entity.velocity.dot(normal);
            let relative_normal_speed = second_normal - first_normal;

            let first_is_knocked = hamsters[first_index].action == HamsterAction::Knockback;
            let second_is_knocked = hamsters[second_index].action == HamsterAction::Knockback;

            if first_is_knocked || second_is_knocked {
                transfer_knockback_by_contact_angle(hamsters, first_index, second_index, normal);
            }

            if relative_normal_speed < 0.0 {
                let impulse = -(1.0 + POOL_BALL_RESTITUTION) * relative_normal_speed / 2.0;
                hamsters[first_index].entity.velocity -= normal * impulse;
                hamsters[second_index].entity.velocity += normal * impulse;
            }

            hamsters[first_index].collision_cooldown = COLLISION_COOLDOWN;
            hamsters[second_index].collision_cooldown = COLLISION_COOLDOWN;
            hamsters[first_index].collisions += 1;
            hamsters[second_index].collisions += 1;
            collisions.push((first_index, second_index));
        }
    }

    collisions
}

// Transfers knockback from a cue hamster to a struck hamster along the contact normal.
fn transfer_knockback_by_contact_angle(
    hamsters: &mut [Hamster],
    first_index: usize,
    second_index: usize,
    normal: Vec2,
) {
    let (cue_index, struck_index) = if hamsters[first_index].action == HamsterAction::Knockback {
        (first_index, second_index)
    } else if hamsters[second_index].action == HamsterAction::Knockback {
        (second_index, first_index)
    } else {
        return;
    };

    if hamsters[struck_index].action == HamsterAction::Knockback
        || hamsters[struck_index].knockback_remaining > 0.0
    {
        return;
    }

    let cue_direction = if cue_index == first_index {
        normal
    } else {
        -normal
    };
    let cue_velocity = hamsters[cue_index].entity.velocity;
    let cue_normal_speed = cue_velocity.dot(cue_direction);

    if cue_normal_speed <= 0.0 {
        return;
    }

    let tangent = vec2(-cue_direction.y, cue_direction.x);
    let tangent_speed = cue_velocity.dot(tangent);

    let transferred_speed = cue_normal_speed * POOL_BALL_RESTITUTION;
    hamsters[struck_index].entity.velocity =
        cue_direction * transferred_speed + tangent * tangent_speed * 0.5;
    hamsters[struck_index].action = HamsterAction::Knockback;
    hamsters[struck_index].knockback_remaining = 1.5;
    hamsters[struck_index].chase_target = None;
    hamsters[struck_index].chase_active = false;
    hamsters[struck_index].chase_remaining = 0.0;
    hamsters[struck_index].pounce_remaining = 0.0;

    hamsters[cue_index].entity.velocity *= 1.0 - POOL_BALL_RESTITUTION;
}

// Randomly assigns a prey and either Chase or Pounce behavior after a collision.
fn start_encounter(hamsters: &mut [Hamster], first: usize, second: usize) {
    if hamsters.iter().any(Hamster::is_chasing) {
        return;
    }

    let prey = if rand::gen_range(0, 2) == 0 {
        first
    } else {
        second
    };
    let predator = if prey == first { second } else { first };
    hamsters[prey].action = HamsterAction::Flee;
    hamsters[prey].chase_target = None;
    hamsters[prey].chase_active = true;
    hamsters[prey].chase_remaining = CHASE_DURATION;

    if rand::gen_range(0, 2) == 0 {
        hamsters[predator].action = HamsterAction::Chase;
    } else {
        hamsters[predator].action = HamsterAction::Pounce;
        hamsters[predator].pounce_remaining = rand::gen_range(0.5, 1.0);
    }
    hamsters[predator].chase_target = Some(prey);
    hamsters[predator].chase_active = true;
    hamsters[predator].chase_remaining = CHASE_DURATION;
}

// Sends a fleeing hamster linearly away from a Chase/Pounce hunter.
fn punch_fleeing_hamster(hamsters: &mut [Hamster], hunter: usize, prey: usize) {
    if hamsters[prey].action != HamsterAction::Flee {
        return;
    }
    match hamsters[hunter].action {
        HamsterAction::Chase => {}
        HamsterAction::Pounce if hamsters[hunter].pounce_remaining == 0.0 => {}
        _ => return,
    }

    let direction =
        (hamsters[prey].entity.position - hamsters[hunter].entity.position).normalize_or_zero();
    hamsters[prey].entity.velocity =
        direction * hamsters[prey].normal_speed * PUNCH_SPEED_MULTIPLIER;
    hamsters[prey].action = HamsterAction::Knockback;
    hamsters[prey].chase_target = None;
    hamsters[prey].chase_active = false;
    hamsters[prey].chase_remaining = 0.0;
    hamsters[prey].knockback_remaining = 2.0;

    hamsters[hunter].action = HamsterAction::Normal;
    hamsters[hunter].chase_target = None;
    hamsters[hunter].chase_active = false;
    hamsters[hunter].chase_remaining = 0.0;
    hamsters[hunter].pounce_remaining = 0.0;
}

// Finds the position of the nearest other hamster for flee steering.
fn closest_other_position(hamsters: &[Hamster], index: usize) -> Option<Vec2> {
    hamsters
        .iter()
        .enumerate()
        .filter(|(other_index, _)| *other_index != index)
        .min_by(|(_, first), (_, second)| {
            let first_distance = first
                .entity
                .position
                .distance(hamsters[index].entity.position);
            let second_distance = second
                .entity
                .position
                .distance(hamsters[index].entity.position);
            first_distance.total_cmp(&second_distance)
        })
        .map(|(_, hamster)| hamster.entity.position)
}

// Calculates the rendered square size and draws one hamster texture plus label.
fn draw_hamster(texture: &Texture2D, hamster: &Hamster) {
    let entity = &hamster.entity;
    let texture_scale = (entity.radius * 2.0) / texture.width().max(texture.height());
    let draw_size = vec2(
        texture.width() * texture_scale,
        texture.height() * texture_scale,
    );

    draw_texture_ex(
        texture,
        entity.position.x - draw_size.x / 2.0,
        entity.position.y - draw_size.y / 2.0,
        entity.color,
        DrawTextureParams {
            dest_size: Some(draw_size),
            ..Default::default()
        },
    );
    draw_text(
        &hamster.name,
        entity.position.x - 55.0,
        entity.position.y + entity.radius + 25.0,
        22.0,
        WHITE,
    );
}

fn new_named_hamster(name: &str) -> Hamster {
    let index = match name {
        "Meep" => 0,
        "Beep" => 1,
        "Peep" => 2,
        _ => 0,
    };
    let horizontal = 0.25 + index as f32 * 0.25;
    let vertical = if index % 2 == 0 { 0.55 } else { 0.35 };
    let velocity = match name {
        "Beep" => vec2(-150.0, 190.0),
        "Peep" => vec2(170.0, -160.0),
        _ => vec2(180.0, 140.0),
    };
    Hamster::new(
        name,
        sandbox_position(horizontal, vertical),
        velocity,
    )
}

fn has_hamster(hamsters: &[Hamster], name: &str) -> bool {
    hamsters.iter().any(|hamster| hamster.name == name)
}

fn next_available_hamster_name(hamsters: &[Hamster]) -> Option<&'static str> {
    ["Beep", "Peep"]
        .into_iter()
        .find(|name| !has_hamster(hamsters, name))
}

fn add_next_hamster(hamsters: &mut Vec<Hamster>) {
    if let Some(name) = next_available_hamster_name(hamsters) {
        hamsters.push(new_named_hamster(name));
    }
}

fn remove_last_hamster(hamsters: &mut Vec<Hamster>) {
    if hamsters.len() > 1 {
        hamsters.pop();
    }
}

#[macroquad::main("Task-Scheduled Hamsters")]
async fn main() {
    // Loads the shared texture once; all entities scale and render independently.
    let texture = load_texture("examples/hamster.png")
        .await
        .expect("Failed to load hamster image");
    texture.set_filter(FilterMode::Nearest);

    let mut hamsters = vec![Hamster::new(
        "Meep",
        sandbox_position(0.3, 0.55),
        vec2(180.0, 140.0),
    )];
    let mut scheduling_kind = SchedulingKind::Fifo;
    let mut engine = FixedTickEngine::new(SchedulerPolicy::fifo(), 1);
    let mut simulation_timer = 0.0;
    let mut step_interval = 0.5;
    let mut paused = false;
    let mut active_task = String::from("Waiting for first task");
    let mut scheduler_log = VecDeque::new();
    let mut previous_time = Instant::now();

    loop {
        // Monotonic elapsed time controls the scheduler accumulator independently of FPS.
        let current_time = Instant::now();
        let delta_time = current_time
            .duration_since(previous_time)
            .as_secs_f32()
            .min(0.25);
        previous_time = current_time;
        if is_key_pressed(KeyCode::Space) {
            paused = !paused;
        }
        if is_key_pressed(KeyCode::Up) {
            step_interval = if step_interval <= MIN_STEP_INTERVAL {
                MIN_STEP_INTERVAL
            } else {
                (step_interval - STEP_INTERVAL_CHANGE).max(MIN_STEP_INTERVAL)
            };
        }
        if is_key_pressed(KeyCode::Down) {
            step_interval = if step_interval <= MIN_STEP_INTERVAL {
                STEP_INTERVAL_CHANGE
            } else {
                (step_interval + STEP_INTERVAL_CHANGE).min(MAX_STEP_INTERVAL)
            };
        }
        if is_key_pressed(KeyCode::A) {
            add_next_hamster(&mut hamsters);
        }
        if is_key_pressed(KeyCode::R) {
            remove_last_hamster(&mut hamsters);
        }
        let requested_scheduling_kind = if is_key_pressed(KeyCode::F1) {
            Some(SchedulingKind::Fifo)
        } else if is_key_pressed(KeyCode::F2) {
            Some(SchedulingKind::Priority)
        } else if is_key_pressed(KeyCode::F3) {
            Some(SchedulingKind::RoundRobin)
        } else {
            None
        };
        if let Some(requested) = requested_scheduling_kind {
            if requested != scheduling_kind {
                scheduling_kind = requested;
                engine = match scheduling_kind {
                    SchedulingKind::Fifo => FixedTickEngine::new(SchedulerPolicy::fifo(), 1),
                    SchedulingKind::Priority => {
                        FixedTickEngine::new(SchedulerPolicy::priority(), 1)
                    }
                    SchedulingKind::RoundRobin => {
                        FixedTickEngine::new(SchedulerPolicy::round_robin(10), 1)
                    }
                };
                hamsters = vec![Hamster::new(
                    "Meep",
                    sandbox_position(0.3, 0.55),
                    vec2(180.0, 140.0),
                )];
                simulation_timer = 0.0;
                step_interval = 0.5;
                paused = false;
                active_task = String::from("Waiting for first task");
                scheduler_log.clear();
                previous_time = Instant::now();
            }
        }
        let mut completed_this_frame = 0;
        if !paused {
            simulation_timer += delta_time;
            let mut steps = 0;
            let scheduler_interval = step_interval;
            while simulation_timer >= scheduler_interval && steps < MAX_CATCH_UP_STEPS {
                // The presentation interval chooses how often tasks become visible.
                // Movement, timers, and physics remain on the fixed 60 Hz quantum.
                simulation_timer -= scheduler_interval;
                if engine.pending_tasks() == 0 {
                    schedule_frame_tasks(&mut engine, &hamsters);
                }
                let report = engine.run_tick();
                completed_this_frame += report.completed_task_ids.len();
                append_scheduler_log(
                    &mut scheduler_log,
                    &report,
                    engine.pending_tasks(),
                    &hamsters,
                );
                for hamster in &mut hamsters {
                    hamster.update_chase(FIXED_SIMULATION_STEP);
                    hamster.update_collision_cooldown(FIXED_SIMULATION_STEP);
                }
                for index in 0..hamsters.len() {
                    let target = hamsters[index]
                        .chase_target
                        .and_then(|target_index| hamsters.get(target_index))
                        .map(|target| target.entity.position);
                    hamsters[index].update_pounce(target, FIXED_SIMULATION_STEP);
                }

                for task_id in &report.completed_task_ids {
                    active_task = task_description(*task_id, &hamsters);
                    let cycle = hamsters.len() as u64 * 2;
                    let task_kind = *task_id % cycle;
                    let is_movement = task_kind % 2 == 1;
                    let hamster_index = if is_movement {
                        ((task_kind - 1) / 2) as usize
                    } else if task_kind == 0 {
                        hamsters.len() - 1
                    } else {
                        (task_kind / 2 - 1) as usize
                    };
                    match task_kind % 2 {
                        1 => {
                            // One movement task advances the entity by the selected frame slice.
                            let movement_delta = FIXED_SIMULATION_STEP;
                            let target_position = hamsters[hamster_index]
                                .chase_target
                                .map(|index| hamsters[index].entity.position);
                            let flee_from = closest_other_position(&hamsters, hamster_index);
                            let hamster = &mut hamsters[hamster_index];
                            hamster.update_movement(
                                target_position,
                                if hamster.action == HamsterAction::Flee {
                                    flee_from
                                } else {
                                    None
                                },
                                movement_delta,
                            );
                            hamster.entity.position += hamster.entity.velocity * movement_delta;
                            if check_collision(&mut hamster.entity) {
                                if hamster.action == HamsterAction::Knockback {
                                    hamster.entity.velocity *= 0.82;
                                }
                                hamster.collisions += 1;
                            }
                        }
                        0 => {
                            let hamster = &mut hamsters[hamster_index];
                            if check_collision(&mut hamster.entity) {
                                hamster.collisions += 1;
                            }
                        }
                        _ => unreachable!("task IDs are assigned to movement or collision checks"),
                    }
                }
                for hamster in &mut hamsters {
                    hamster.update_knockback(FIXED_SIMULATION_STEP);
                }
                let collisions = check_collisions(&mut hamsters);
                for (first, second) in collisions {
                    let first_hunter = matches!(
                        hamsters[first].action,
                        HamsterAction::Chase | HamsterAction::Pounce
                    );
                    let second_hunter = matches!(
                        hamsters[second].action,
                        HamsterAction::Chase | HamsterAction::Pounce
                    );
                    if first_hunter && hamsters[second].action == HamsterAction::Flee {
                        punch_fleeing_hamster(&mut hamsters, first, second);
                    } else if second_hunter && hamsters[first].action == HamsterAction::Flee {
                        punch_fleeing_hamster(&mut hamsters, second, first);
                    } else if !hamsters.iter().any(|hamster| {
                        hamster.action == HamsterAction::Knockback || hamster.is_chasing()
                    }) {
                        start_encounter(&mut hamsters, first, second);
                    }
                }
                steps += 1;
            }
            if steps == MAX_CATCH_UP_STEPS {
                simulation_timer = 0.0;
            }
        }

        clear_background(BLACK);
        draw_rectangle(
            0.0,
            0.0,
            screen_width(),
            HUD_HEIGHT,
            Color::new(0.015, 0.015, 0.02, 1.0),
        );
        let panel_padding = 12.0;
        let split_x = screen_width() * 0.5;
        let left_width = split_x - panel_padding * 2.0;
        let right_panel_x = split_x + panel_padding;
        let right_width = screen_width() - right_panel_x - panel_padding;
        draw_line(
            0.0,
            HUD_HEIGHT,
            screen_width(),
            HUD_HEIGHT,
            2.0,
            DARKGRAY,
        );
        let scheduling_name = match scheduling_kind {
            SchedulingKind::Fifo => "FIFO",
            SchedulingKind::Priority => "Priority",
            SchedulingKind::RoundRobin => "Round Robin",
        };
        let mut left_y = draw_wrapped_text(
            &format!(
                "{} | Tick: {} | Completed this frame: {} | Total: {}",
                scheduling_name,
                engine.current_tick(),
                completed_this_frame,
                engine.aggregate_metrics().completed_tasks,
            ),
            20.0,
            30.0,
            left_width - 16.0,
            20.0,
            22.0,
            WHITE,
        );
        left_y = draw_wrapped_text(
            &format!("Active task: {}", active_task),
            20.0,
            left_y + 2.0,
            left_width - 16.0,
            20.0,
            22.0,
            YELLOW,
        );
        left_y = draw_wrapped_text(
            &format!("Step interval: {:.3}s", step_interval),
            20.0,
            left_y + 2.0,
            left_width - 16.0,
            18.0,
            20.0,
            LIGHTGRAY,
        );
        left_y = draw_wrapped_text(
            "Up/Down: speed | Space: pause/resume | A: add | R: remove | F1: FIFO | F2: Priority | F3: RR",
            20.0,
            left_y + 2.0,
            left_width - 16.0,
            18.0,
            20.0,
            LIGHTGRAY,
        );
        let log_x = right_panel_x + 8.0;
        draw_text("Scheduler log:", log_x, 32.0, 20.0, GOLD);
        let mut log_y = 54.0;
        for entry in &scheduler_log {
            if log_y >= HUD_HEIGHT - 8.0 {
                break;
            }
            log_y = draw_wrapped_text(
                entry,
                log_x,
                log_y,
                right_width - 16.0,
                12.0,
                15.0,
                LIGHTGRAY,
            );
        }

        let mut status_y = left_y + 4.0;
        for (index, hamster) in hamsters.iter().enumerate() {
            draw_hamster(&texture, hamster);
            let behavior = match hamster.action {
                HamsterAction::Chase => {
                    let target_index = hamster.chase_target.unwrap_or(index);
                    format!(
                        "Chasing {} ({:.1}s)",
                        hamsters[target_index].name, hamster.chase_remaining
                    )
                }
                HamsterAction::Pounce => format!("Pouncing {:.1}s", hamster.pounce_remaining),
                HamsterAction::Flee => format!("Fleeing ({:.1}s)", hamster.chase_remaining),
                HamsterAction::Knockback => {
                    format!("Knocked back ({:.1}s)", hamster.knockback_remaining)
                }
                HamsterAction::Normal => {
                    format!("Moving {}", movement_direction(hamster.entity.velocity))
                }
            };
            status_y = draw_wrapped_text(
                &format!(
                    "{}: {}  |  Velocity: ({:.0}, {:.0})  |  Collisions: {}",
                    hamster.name,
                    behavior,
                    hamster.entity.velocity.x,
                    hamster.entity.velocity.y,
                    hamster.collisions,
                ),
                20.0,
                status_y,
                left_width - 16.0,
                18.0,
                20.0,
                WHITE,
            );
        }

        next_frame().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roster_starts_with_meep_and_add_remove_keeps_meep() {
        let meep = hamsters()[0].clone();
        let mut roster = vec![meep];
        assert_eq!(next_available_hamster_name(&roster), Some("Beep"));

        roster.push(hamsters()[1].clone());
        assert_eq!(next_available_hamster_name(&roster), Some("Peep"));

        roster.push(hamsters()[2].clone());
        assert_eq!(next_available_hamster_name(&roster), None);

        remove_last_hamster(&mut roster);
        assert_eq!(roster.len(), 2);
        remove_last_hamster(&mut roster);
        assert_eq!(roster.len(), 1);
        remove_last_hamster(&mut roster);
        assert_eq!(roster.len(), 1);
    }

    fn hamsters() -> [Hamster; 3] {
        [
            Hamster {
                name: "Meep".to_string(),
                entity: Entity::new(vec2(100.0, 100.0), vec2(180.0, 0.0), 75.0, WHITE),
                collisions: 0,
                waypoint: vec2(500.0, 500.0),
                normal_speed: 180.0,
                chase_target: None,
                chase_remaining: 0.0,
                chase_active: false,
                collision_cooldown: 0.0,
                action: HamsterAction::Normal,
                pounce_remaining: 0.0,
                knockback_remaining: 0.0,
            },
            Hamster {
                name: "Beep".to_string(),
                entity: Entity::new(vec2(300.0, 100.0), vec2(-150.0, 0.0), 75.0, WHITE),
                collisions: 0,
                waypoint: vec2(500.0, 500.0),
                normal_speed: 150.0,
                chase_target: None,
                chase_remaining: 0.0,
                chase_active: false,
                collision_cooldown: 0.0,
                action: HamsterAction::Normal,
                pounce_remaining: 0.0,
                knockback_remaining: 0.0,
            },
            Hamster {
                name: "Peep".to_string(),
                entity: Entity::new(vec2(500.0, 100.0), vec2(160.0, 0.0), 75.0, WHITE),
                collisions: 0,
                waypoint: vec2(500.0, 500.0),
                normal_speed: 160.0,
                chase_target: None,
                chase_remaining: 0.0,
                chase_active: false,
                collision_cooldown: 0.0,
                action: HamsterAction::Normal,
                pounce_remaining: 0.0,
                knockback_remaining: 0.0,
            },
        ]
    }

    #[test]
    fn collision_assigns_one_predator_and_one_prey() {
        let mut hamsters = hamsters();
        start_encounter(&mut hamsters, 0, 1);

        let predator_count = hamsters
            .iter()
            .filter(|hamster| {
                matches!(hamster.action, HamsterAction::Chase | HamsterAction::Pounce)
            })
            .count();
        let prey_count = hamsters
            .iter()
            .filter(|hamster| hamster.action == HamsterAction::Flee)
            .count();

        assert_eq!(predator_count, 1);
        assert_eq!(prey_count, 1);
    }

    #[test]
    fn active_chase_is_not_restarted_by_another_collision() {
        let mut hamsters = hamsters();
        start_encounter(&mut hamsters, 0, 1);

        if let Some(predator) = hamsters
            .iter_mut()
            .find(|hamster| matches!(hamster.action, HamsterAction::Chase | HamsterAction::Pounce))
        {
            predator.chase_remaining = 1.25;
        }
        start_encounter(&mut hamsters, 0, 2);

        assert!(
            hamsters
                .iter()
                .find(|hamster| matches!(
                    hamster.action,
                    HamsterAction::Chase | HamsterAction::Pounce
                ))
                .is_some_and(|predator| predator.chase_remaining == 1.25)
        );
    }

    #[test]
    fn chase_expires_after_five_seconds() {
        let mut hamsters = hamsters();
        start_encounter(&mut hamsters, 0, 1);

        for hamster in &mut hamsters {
            hamster.update_chase(CHASE_DURATION);
        }

        assert!(hamsters.iter().all(|hamster| !hamster.is_chasing()));
        assert!(
            hamsters
                .iter()
                .all(|hamster| hamster.chase_target.is_none())
        );
    }

    #[test]
    fn prey_moves_twice_as_fast_as_normal() {
        let mut hamsters = hamsters();
        start_encounter(&mut hamsters, 0, 1);
        let predator_index = hamsters
            .iter()
            .position(|hamster| {
                matches!(hamster.action, HamsterAction::Chase | HamsterAction::Pounce)
            })
            .expect("a predator should be assigned");
        let prey_index = 1 - predator_index;
        let normal_speed = hamsters[prey_index].normal_speed;

        hamsters[prey_index].update_movement(None, Some(vec2(500.0, 100.0)), 0.01);

        assert!(
            (hamsters[prey_index].entity.velocity.length() - normal_speed * FLEE_SPEED_MULTIPLIER)
                < 1.0
        );
        assert!(
            (normal_speed * FLEE_SPEED_MULTIPLIER - normal_speed * CHASE_SPEED_MULTIPLIER) > 0.0
        );
    }

    #[test]
    fn repeated_overlap_is_blocked_during_collision_cooldown() {
        let mut hamsters = hamsters();
        hamsters[1].entity.position = hamsters[0].entity.position
            + vec2(
                hamsters[0].entity.radius + hamsters[1].entity.radius - 10.0,
                0.0,
            );

        assert_eq!(check_collisions(&mut hamsters).len(), 1);
        let first_velocity = hamsters[0].entity.velocity;
        let second_velocity = hamsters[1].entity.velocity;
        let first_collisions = hamsters[0].collisions;
        let second_collisions = hamsters[1].collisions;

        assert!(check_collisions(&mut hamsters).is_empty());
        assert_eq!(hamsters[0].entity.velocity, first_velocity);
        assert_eq!(hamsters[1].entity.velocity, second_velocity);
        assert_eq!(hamsters[0].collisions, first_collisions);
        assert_eq!(hamsters[1].collisions, second_collisions);
    }

    #[test]
    fn pounce_waits_then_launches_at_target() {
        let mut hamsters = hamsters();
        hamsters[0].action = HamsterAction::Pounce;
        hamsters[0].chase_target = Some(1);
        hamsters[0].pounce_remaining = 0.5;

        hamsters[0].update_pounce(Some(hamsters[1].entity.position), 0.5);
        hamsters[0].update_movement(Some(hamsters[1].entity.position), None, 0.5);

        assert_eq!(hamsters[0].pounce_remaining, 0.0);
        assert_eq!(hamsters[0].action, HamsterAction::Pounce);
        assert!(hamsters[0].entity.velocity.length() > hamsters[0].normal_speed);
    }

    #[test]
    fn punch_sends_prey_into_knockback() {
        let mut hamsters = hamsters();
        hamsters[0].action = HamsterAction::Chase;
        hamsters[0].chase_target = Some(1);
        hamsters[1].action = HamsterAction::Flee;

        punch_fleeing_hamster(&mut hamsters, 0, 1);

        assert_eq!(hamsters[1].action, HamsterAction::Knockback);
        assert!(hamsters[1].knockback_remaining > 0.0);
        assert!(
            hamsters[1].entity.velocity.length()
                >= hamsters[1].normal_speed * PUNCH_SPEED_MULTIPLIER - 1.0
        );
    }

    #[test]
    fn knocked_back_hamster_transfers_velocity_to_struck_hamster() {
        let mut hamsters = hamsters();
        hamsters[0].action = HamsterAction::Knockback;
        hamsters[0].entity.velocity = vec2(400.0, 0.0);
        hamsters[1].entity.position = vec2(240.0, 100.0);

        let _ = check_collisions(&mut hamsters);

        assert_eq!(hamsters[1].action, HamsterAction::Knockback);
        assert!(hamsters[1].entity.velocity.x > 0.0);
        assert!(hamsters[1].knockback_remaining > 0.0);
    }

    #[test]
    fn glancing_impact_keeps_a_tangent_component() {
        let mut hamsters = hamsters();
        hamsters[0].action = HamsterAction::Knockback;
        hamsters[0].entity.velocity = vec2(400.0, 100.0);
        hamsters[1].entity.position = vec2(240.0, 145.0);

        let _ = check_collisions(&mut hamsters);

        assert_eq!(hamsters[1].action, HamsterAction::Knockback);
        assert!(hamsters[1].entity.velocity.y != 0.0);
    }

    #[test]
    fn knocked_back_hamster_overrides_pounce_state() {
        let mut hamsters = hamsters();
        hamsters[0].action = HamsterAction::Knockback;
        hamsters[0].entity.velocity = vec2(400.0, 0.0);
        hamsters[1].action = HamsterAction::Pounce;
        hamsters[1].pounce_remaining = 0.75;
        hamsters[1].entity.position = vec2(240.0, 100.0);

        let collisions = check_collisions(&mut hamsters);

        assert_eq!(collisions.len(), 1);
        assert_eq!(hamsters[1].action, HamsterAction::Knockback);
        assert_eq!(hamsters[1].pounce_remaining, 0.0);
        assert!(hamsters[1].knockback_remaining > 0.0);
    }
}
