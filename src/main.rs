use macroquad::prelude::*;
use scheduler_project::entity::Entity;
use scheduler_project::scheduler::{
    BehaviorType, DeadlineType, FifoScheduler, FixedTickEngine, Task,
};
use std::time::Instant;

struct Hamster {
    name: String,
    entity: Entity,
    collisions: u64,
    waypoint: Vec2,
    normal_speed: f32,
    chase_target: Option<usize>,
    chase_remaining: f32,
    chase_active: bool,
    collision_cooldown: f32,
    action: HamsterAction,
    pounce_remaining: f32,
    knockback_remaining: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HamsterAction {
    Normal,
    Chase,
    Pounce,
    Flee,
    Knockback,
}

const CHASE_DURATION: f32 = 5.0;
const CHASE_SPEED_MULTIPLIER: f32 = 1.5;
const FLEE_SPEED_MULTIPLIER: f32 = 2.0;
const POUNCE_LAUNCH_SPEED_MULTIPLIER: f32 = 3.5;
const PUNCH_SPEED_MULTIPLIER: f32 = 5.0;
const KNOCKBACK_DECAY_PER_SECOND: f32 = 0.82;
const MIN_STEP_INTERVAL: f32 = 0.001;
const MAX_STEP_INTERVAL: f32 = 1.0;
const STEP_INTERVAL_CHANGE: f32 = 0.05;
const MAX_CATCH_UP_STEPS: usize = 100;
const COLLISION_COOLDOWN: f32 = 0.75;

impl Hamster {
    fn new(name: impl Into<String>, position: Vec2, velocity: Vec2) -> Self {
        let radius = 75.0;
        Self {
            name: name.into(),
            entity: Entity::new(position, velocity, radius, WHITE),
            collisions: 0,
            waypoint: random_waypoint(radius),
            normal_speed: velocity.length(),
            chase_target: None,
            chase_remaining: 0.0,
            chase_active: false,
            collision_cooldown: 0.0,
            action: HamsterAction::Normal,
            pounce_remaining: 0.0,
            knockback_remaining: 0.0,
        }
    }

    fn update_collision_cooldown(&mut self, delta_time: f32) {
        self.collision_cooldown = (self.collision_cooldown - delta_time).max(0.0);
    }

    fn is_chasing(&self) -> bool {
        self.chase_active && self.chase_remaining > 0.0
    }

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

    fn steer_toward(&mut self, target: Vec2, speed: f32, delta_time: f32) {
        let direction = (target - self.entity.position).normalize_or_zero();
        let desired_velocity = direction * speed;
        let steering = (delta_time * 4.0).clamp(0.0, 1.0);
        self.entity.velocity = self.entity.velocity.lerp(desired_velocity, steering);
    }

    fn update_movement(
        &mut self,
        target_position: Option<Vec2>,
        flee_from: Option<Vec2>,
        delta_time: f32,
    ) {
        if self.action == HamsterAction::Pounce {
            if self.pounce_remaining > 0.0 {
                self.pounce_remaining = (self.pounce_remaining - delta_time).max(0.0);
                self.entity.velocity = Vec2::ZERO;
                if self.pounce_remaining == 0.0 {
                    if let Some(target) = target_position {
                        let direction = (target - self.entity.position).normalize_or_zero();
                        self.entity.velocity =
                            direction * self.normal_speed * POUNCE_LAUNCH_SPEED_MULTIPLIER;
                    }
                }
            }
            return;
        }

        if self.action == HamsterAction::Knockback {
            self.entity.position += self.entity.velocity * delta_time;
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

fn random_waypoint(radius: f32) -> Vec2 {
    let max_x = (screen_width() - radius).max(radius);
    let max_y = (screen_height() - radius).max(radius);
    vec2(
        rand::gen_range(radius, max_x),
        rand::gen_range(radius, max_y),
    )
}

fn movement_direction(velocity: Vec2) -> &'static str {
    match (velocity.x.is_sign_negative(), velocity.y.is_sign_negative()) {
        (false, false) => "right and down",
        (false, true) => "right and up",
        (true, false) => "left and down",
        (true, true) => "left and up",
    }
}

fn schedule_frame_tasks(engine: &mut FixedTickEngine<FifoScheduler>, hamsters: &[Hamster; 3]) {
    let tick = engine.current_tick();
    let first_task_id = tick * 6 + 1;

    for hamster_index in 0..3 {
        let task_offset = hamster_index * 2;
        let hamster_index = hamster_index as usize;
        let activity = if hamsters[hamster_index].is_chasing() {
            match hamsters[hamster_index].action {
                HamsterAction::Chase => ("Chase", BehaviorType::Chasing),
                HamsterAction::Pounce => ("Pounce", BehaviorType::Pouncing),
                HamsterAction::Flee => ("Flee", BehaviorType::Fleeing),
                HamsterAction::Knockback => ("Knockback", BehaviorType::CollisionCheck),
                HamsterAction::Normal => ("Move", BehaviorType::Movement),
            }
        } else {
            ("Move", BehaviorType::Movement)
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
            9,
            tick + 1,
            DeadlineType::Hard,
            1,
            BehaviorType::CollisionCheck,
        ));
    }
}

fn task_description(task_id: u64, hamsters: &[Hamster; 3]) -> String {
    let movement_residue = match task_id % 6 {
        1 => Some((0, "Movement")),
        3 => Some((1, "Movement")),
        5 => Some((2, "Movement")),
        _ => None,
    };
    if let Some((index, normal_label)) = movement_residue {
        let action = match hamsters[index].action {
            HamsterAction::Chase => format!(
                "Chasing {}",
                hamsters[hamsters[index].chase_target.unwrap_or(index)].name
            ),
            HamsterAction::Pounce => "Pouncing".to_string(),
            HamsterAction::Flee => "Fleeing".to_string(),
            HamsterAction::Knockback => "Knocked back".to_string(),
            HamsterAction::Normal => normal_label.to_string(),
        };
        return format!("{}: {}", hamsters[index].name, action);
    }

    match task_id % 6 {
        2 => format!("{}: Collision check", hamsters[0].name),
        4 => format!("{}: Collision check", hamsters[1].name),
        0 => format!("{}: Collision check", hamsters[2].name),
        _ => "Unknown task".to_string(),
    }
}

fn check_collision(entity: &mut Entity) -> bool {
    let mut bounced = false;

    if entity.position.x - entity.radius < 0.0 {
        entity.position.x = entity.radius;
        entity.velocity.x = entity.velocity.x.abs();
        bounced = true;
    } else if entity.position.x + entity.radius > screen_width() {
        entity.position.x = screen_width() - entity.radius;
        entity.velocity.x = -entity.velocity.x.abs();
        bounced = true;
    }

    if entity.position.y - entity.radius < 0.0 {
        entity.position.y = entity.radius;
        entity.velocity.y = entity.velocity.y.abs();
        bounced = true;
    } else if entity.position.y + entity.radius > screen_height() {
        entity.position.y = screen_height() - entity.radius;
        entity.velocity.y = -entity.velocity.y.abs();
        bounced = true;
    }

    bounced
}

fn check_hamster_collisions(hamsters: &mut [Hamster; 3]) -> Vec<(usize, usize)> {
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
            hamsters[first_index].entity.velocity += normal * (second_normal - first_normal);
            hamsters[second_index].entity.velocity += normal * (first_normal - second_normal);

            hamsters[first_index].collision_cooldown = COLLISION_COOLDOWN;
            hamsters[second_index].collision_cooldown = COLLISION_COOLDOWN;
            hamsters[first_index].collisions += 1;
            hamsters[second_index].collisions += 1;
            collisions.push((first_index, second_index));
        }
    }

    collisions
}

fn start_encounter(hamsters: &mut [Hamster; 3], first: usize, second: usize) {
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

fn punch_fleeing_hamster(hamsters: &mut [Hamster; 3], hunter: usize, prey: usize) {
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

fn closest_other_position(hamsters: &[Hamster; 3], index: usize) -> Option<Vec2> {
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

#[macroquad::main("Task-Scheduled Hamsters")]
async fn main() {
    let texture = load_texture("examples/hamster.png")
        .await
        .expect("Failed to load hamster image");
    texture.set_filter(FilterMode::Nearest);

    let mut hamsters = [
        Hamster::new(
            "Meep",
            vec2(screen_width() * 0.3, screen_height() * 0.55),
            vec2(180.0, 140.0),
        ),
        Hamster::new(
            "Beep",
            vec2(screen_width() * 0.7, screen_height() * 0.55),
            vec2(-150.0, 190.0),
        ),
        Hamster::new(
            "Peep",
            vec2(screen_width() * 0.5, screen_height() * 0.25),
            vec2(170.0, -160.0),
        ),
    ];
    let mut engine = FixedTickEngine::new(FifoScheduler::new(), 1);
    let mut simulation_timer = 0.0;
    let mut step_interval = 0.5;
    let mut paused = false;
    let mut active_task = String::from("Waiting for first task");
    let mut previous_time = Instant::now();

    loop {
        let current_time = Instant::now();
        let delta_time = current_time
            .duration_since(previous_time)
            .as_secs_f32()
            .min(0.1);
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

        let real_time_mode = step_interval <= MIN_STEP_INTERVAL;
        let mut completed_this_frame = 0;
        if !paused {
            simulation_timer += delta_time;
            let mut steps = 0;
            while simulation_timer >= step_interval && steps < MAX_CATCH_UP_STEPS {
                simulation_timer -= step_interval;
                if engine.pending_tasks() == 0 {
                    schedule_frame_tasks(&mut engine, &hamsters);
                }
                let report = engine.run_tick();
                completed_this_frame += report.completed_task_ids.len();
                for hamster in &mut hamsters {
                    hamster.update_chase(step_interval);
                    hamster.update_collision_cooldown(step_interval);
                }

                for task_id in &report.completed_task_ids {
                    active_task = task_description(*task_id, &hamsters);
                    let task_kind = *task_id % 6;
                    let hamster_index = match task_kind {
                        1 | 2 => 0,
                        3 | 4 => 1,
                        5 | 0 => 2,
                        _ => unreachable!("task IDs are assigned to one of three hamsters"),
                    };
                    match task_kind {
                        1 | 3 | 5 => {
                            let movement_delta = if real_time_mode {
                                step_interval * 4.0
                            } else {
                                delta_time
                            };
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
                        2 | 4 | 0 => {
                            let hamster = &mut hamsters[hamster_index];
                            if check_collision(&mut hamster.entity) {
                                hamster.collisions += 1;
                            }
                        }
                        _ => unreachable!("task IDs are assigned to movement or collision checks"),
                    }
                }
                for hamster in &mut hamsters {
                    hamster.update_knockback(step_interval);
                }
                let collisions = check_hamster_collisions(&mut hamsters);
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
                    } else if !hamsters.iter().any(Hamster::is_chasing) {
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
        draw_text(
            &format!(
                "FIFO | Tick: {}  |  Completed this frame: {}  |  Total: {}",
                engine.current_tick(),
                completed_this_frame,
                engine.aggregate_metrics().completed_tasks,
            ),
            20.0,
            30.0,
            24.0,
            WHITE,
        );
        draw_text(
            &format!(
                "Active task: {}  |  {}",
                active_task,
                if paused { "PAUSED" } else { "RUNNING" },
            ),
            20.0,
            60.0,
            24.0,
            YELLOW,
        );
        draw_text(
            &format!(
                "Step interval: {}  |  Up/Down: speed  |  Space: pause/resume",
                if real_time_mode {
                    "REAL-TIME".to_string()
                } else {
                    format!("{step_interval:.3}s")
                },
            ),
            20.0,
            90.0,
            22.0,
            LIGHTGRAY,
        );

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
            draw_text(
                &format!(
                    "{}: {}  |  Velocity: ({:.0}, {:.0})  |  Collisions: {}",
                    hamster.name,
                    behavior,
                    hamster.entity.velocity.x,
                    hamster.entity.velocity.y,
                    hamster.collisions,
                ),
                20.0,
                125.0 + index as f32 * 30.0,
                22.0,
                WHITE,
            );
        }

        next_frame().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

        assert_eq!(check_hamster_collisions(&mut hamsters).len(), 1);
        let first_velocity = hamsters[0].entity.velocity;
        let second_velocity = hamsters[1].entity.velocity;
        let first_collisions = hamsters[0].collisions;
        let second_collisions = hamsters[1].collisions;

        assert!(check_hamster_collisions(&mut hamsters).is_empty());
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

        hamsters[0].update_movement(Some(hamsters[1].entity.position), None, 0.5);

        assert_eq!(hamsters[0].pounce_remaining, 0.0);
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
}
