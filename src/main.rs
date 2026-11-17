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
}

const CHASE_DURATION: f32 = 5.0;
const CHASE_SPEED_MULTIPLIER: f32 = 1.5;
const MIN_STEP_INTERVAL: f32 = 0.001;
const MAX_STEP_INTERVAL: f32 = 1.0;
const STEP_INTERVAL_CHANGE: f32 = 0.05;
const MAX_CATCH_UP_STEPS: usize = 100;

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
        }
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
            }
        }
    }

    fn steer_toward(&mut self, target: Vec2, speed: f32, delta_time: f32) {
        let direction = (target - self.entity.position).normalize_or_zero();
        let desired_velocity = direction * speed;
        let steering = (delta_time * 4.0).clamp(0.0, 1.0);
        self.entity.velocity = self.entity.velocity.lerp(desired_velocity, steering);
    }

    fn update_movement(&mut self, target_position: Option<Vec2>, delta_time: f32) {
        if !self.is_chasing() && self.entity.position.distance(self.waypoint) < self.entity.radius {
            self.waypoint = random_waypoint(self.entity.radius);
        }

        let target = target_position.unwrap_or(self.waypoint);
        let speed = if self.is_chasing() {
            self.normal_speed * CHASE_SPEED_MULTIPLIER
        } else {
            self.normal_speed
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

fn schedule_frame_tasks(engine: &mut FixedTickEngine<FifoScheduler>, hamsters: &[Hamster; 2]) {
    let tick = engine.current_tick();
    let first_task_id = tick * 4 + 1;

    for hamster_index in 0..2 {
        let task_offset = hamster_index * 2;
        let hamster_index = hamster_index as usize;
        engine.add_task(Task::new(
            first_task_id + task_offset,
            format!(
                "{} {}",
                if hamsters[hamster_index].is_chasing() {
                    "Chase"
                } else {
                    "Move"
                },
                hamsters[hamster_index].name
            ),
            10,
            tick + 1,
            DeadlineType::Hard,
            1,
            if hamsters[hamster_index].chase_target.is_some() {
                BehaviorType::Chasing
            } else {
                BehaviorType::Movement
            },
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

fn task_description(task_id: u64, hamsters: &[Hamster; 2]) -> String {
    let hamster_name = match task_id % 4 {
        1 | 2 => &hamsters[0].name,
        3 | 0 => &hamsters[1].name,
        _ => unreachable!("task IDs are assigned to one of four FIFO activities"),
    };

    match task_id % 4 {
        1 | 3 => {
            let hamster_index = if task_id % 4 == 1 { 0 } else { 1 };
            if hamsters[hamster_index].is_chasing() {
                if let Some(target_index) = hamsters[hamster_index].chase_target {
                    format!("{hamster_name}: Chasing {}", hamsters[target_index].name)
                } else {
                    format!("{hamster_name}: Fleeing")
                }
            } else {
                format!("{hamster_name}: Movement")
            }
        }
        2 | 0 => format!("{hamster_name}: Collision check"),
        _ => unreachable!("task IDs are assigned to one of four FIFO activities"),
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

fn check_hamster_collision(hamsters: &mut [Hamster; 2]) -> bool {
    let [first, second] = hamsters;
    let offset = second.entity.position - first.entity.position;
    let distance = offset.length();
    let minimum_distance = first.entity.radius + second.entity.radius;

    if distance >= minimum_distance {
        return false;
    }

    let normal = if distance > f32::EPSILON {
        offset / distance
    } else {
        vec2(1.0, 0.0)
    };
    let separation = (minimum_distance - distance) / 2.0;
    first.entity.position -= normal * separation;
    second.entity.position += normal * separation;
    std::mem::swap(&mut first.entity.velocity, &mut second.entity.velocity);
    first.collisions += 1;
    second.collisions += 1;
    true
}

fn start_chase(hamsters: &mut [Hamster; 2]) {
    let predator = rand::gen_range(0usize, 2usize);
    let prey = 1 - predator;
    hamsters[prey].chase_target = None;
    hamsters[prey].chase_active = true;
    hamsters[prey].chase_remaining = CHASE_DURATION;
    hamsters[predator].chase_target = Some(prey);
    hamsters[predator].chase_active = true;
    hamsters[predator].chase_remaining = CHASE_DURATION;
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
                }

                for task_id in &report.completed_task_ids {
                    active_task = task_description(*task_id, &hamsters);
                    let task_kind = *task_id % 4;
                    let hamster_index = match task_kind {
                        1 | 2 => 0,
                        0 | 3 => 1,
                        _ => unreachable!("task IDs are assigned to one of two hamsters"),
                    };
                    match task_kind {
                        1 | 3 => {
                            let movement_delta = if real_time_mode {
                                step_interval * 4.0
                            } else {
                                delta_time
                            };
                            let target_position = hamsters[hamster_index]
                                .chase_target
                                .map(|index| hamsters[index].entity.position);
                            let hamster = &mut hamsters[hamster_index];
                            hamster.update_movement(target_position, movement_delta);
                            hamster.entity.position += hamster.entity.velocity * movement_delta;
                            if check_collision(&mut hamster.entity) {
                                hamster.collisions += 1;
                            }
                        }
                        0 | 2 => {
                            let hamster = &mut hamsters[hamster_index];
                            if check_collision(&mut hamster.entity) {
                                hamster.collisions += 1;
                            }
                        }
                        _ => unreachable!("task IDs are assigned to movement or collision checks"),
                    }
                }
                if check_hamster_collision(&mut hamsters) {
                    start_chase(&mut hamsters);
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
            let behavior = if let Some(target_index) = hamster.chase_target {
                format!(
                    "Chasing {} ({:.1}s)",
                    hamsters[target_index].name, hamster.chase_remaining
                )
            } else if hamster.is_chasing() {
                format!("Fleeing ({:.1}s)", hamster.chase_remaining)
            } else {
                format!("Moving {}", movement_direction(hamster.entity.velocity))
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
