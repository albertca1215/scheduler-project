use macroquad::prelude::*;
use scheduler_project::entity::Entity;
use scheduler_project::scheduler::{
    BehaviorType, DeadlineType, FifoScheduler, FixedTickEngine, Task,
};

const MOVEMENT_TASK_ID: u64 = 1;
const COLLISION_TASK_ID: u64 = 2;
const FIRST_COLOR_TASK_ID: u64 = 1_000_000;

fn add_frame_tasks(engine: &mut FixedTickEngine<FifoScheduler>) {
    let tick = engine.current_tick();

    engine.add_task(Task::new(
        MOVEMENT_TASK_ID + tick * 2,
        "Move entity",
        10,
        tick + 1,
        DeadlineType::Hard,
        1,
        BehaviorType::Movement,
    ));
    engine.add_task(Task::new(
        COLLISION_TASK_ID + tick * 2,
        "Check entity collision",
        9,
        tick + 1,
        DeadlineType::Hard,
        1,
        BehaviorType::CollisionCheck,
    ));
}

fn random_color() -> Color {
    Color::new(
        rand::gen_range(0.0, 1.0),
        rand::gen_range(0.0, 1.0),
        rand::gen_range(0.0, 1.0),
        1.0,
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

#[macroquad::main("Task-Scheduled Entity")]
async fn main() {
    let texture = load_texture("examples/hamster.png")
        .await
        .expect("Failed to load hamster image");
    texture.set_filter(FilterMode::Nearest);

    let radius = 100.0;
    let mut entity = Entity::new(
        vec2(screen_width() / 2.0, screen_height() / 2.0),
        vec2(180.0, 140.0),
        radius,
        WHITE,
    );
    let mut engine = FixedTickEngine::new(FifoScheduler::new(), 2);
    let mut next_color_task_id = FIRST_COLOR_TASK_ID;
    let mut collision_count = 0;

    loop {
        let delta_time = get_frame_time();
        add_frame_tasks(&mut engine);
        let report = engine.run_tick();

        for task_id in &report.completed_task_ids {
            match task_id {
                &id if id >= FIRST_COLOR_TASK_ID => {
                    entity.color = random_color();
                }
                &id if id % 2 == 1 => {
                    entity.position += entity.velocity * delta_time;
                }
                &id if id % 2 == 0 => {
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

                    if bounced {
                        collision_count += 1;
                        engine.add_task(Task::new(
                            next_color_task_id,
                            "Change entity color",
                            8,
                            engine.current_tick() + 1,
                            DeadlineType::EventDriven,
                            1,
                            BehaviorType::CollisionColorChange,
                        ));
                        next_color_task_id += 1;
                    }
                }
                _ => {}
            }
        }

        clear_background(BLACK);
        let texture_scale = (entity.radius * 2.0) / texture.width().max(texture.height());
        let draw_size = vec2(
            texture.width() * texture_scale,
            texture.height() * texture_scale,
        );
        draw_texture_ex(
            &texture,
            entity.position.x - draw_size.x / 2.0,
            entity.position.y - draw_size.y / 2.0,
            entity.color,
            DrawTextureParams {
                dest_size: Some(draw_size),
                ..Default::default()
            },
        );
        draw_text(
            &format!(
                "Tick: {}  |  Completed this tick: {}  |  Total: {}",
                report.tick,
                report.completed_task_ids.len(),
                engine.aggregate_metrics().completed_tasks,
            ),
            20.0,
            30.0,
            24.0,
            WHITE,
        );
        draw_text("Behavior: Moving", 20.0, 60.0, 24.0, WHITE);
        draw_text(
            &format!(
                "Direction: {}  |  Velocity: ({:.0}, {:.0})  |  Collisions: {}",
                movement_direction(entity.velocity),
                entity.velocity.x,
                entity.velocity.y,
                collision_count,
            ),
            20.0,
            90.0,
            24.0,
            WHITE,
        );

        next_frame().await;
    }
}
