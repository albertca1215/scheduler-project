use scheduler_project::scheduler::{
    BehaviorType, DeadlineType, FifoScheduler, FixedTickEngine, Task,
};

fn main() {
    let tasks = [
        Task::new(
            1,
            "Wall collision",
            10,
            2,
            DeadlineType::Hard,
            1,
            BehaviorType::CollisionCheck,
        ),
        Task::new(
            2,
            "Move hamster",
            5,
            5,
            DeadlineType::Soft,
            2,
            BehaviorType::Movement,
        ),
        Task::new(
            3,
            "Change color",
            1,
            10,
            DeadlineType::EventDriven,
            1,
            BehaviorType::CollisionColorChange,
        ),
    ];

    let scheduler = FifoScheduler::new();
    let mut engine = FixedTickEngine::new(scheduler, 2);
    let task_count = tasks.len() as u64;

    println!("Scheduler demo: FIFO (tick budget: 2 work units)");
    println!("Adding tasks:");
    for task in tasks {
        println!(
            "  [{}] {} (priority {}, deadline tick {}, cost {})",
            task.id, task.name, task.priority, task.deadline, task.simulated_cost
        );
        engine.add_task(task);
    }

    println!("\nTick execution:");
    while engine.aggregate_metrics().completed_tasks < task_count {
        let report = engine.run_tick();
        println!(
            "  Tick {}: {} work, {} idle{}",
            report.tick,
            report.work_units,
            report.idle_units,
            if report.completed_task_ids.is_empty() {
                String::new()
            } else {
                format!("; completed {:?}", report.completed_task_ids)
            }
        );
    }

    println!("\nTask metrics:");
    for (task_id, metrics) in engine.task_metrics() {
        println!(
            "  Task {}: response {:?}, waiting {:?}, turnaround {:?}, deadline missed {}",
            task_id,
            metrics.response_time(),
            metrics.waiting_time(),
            metrics.turnaround_time(),
            metrics.deadline_missed
        );
    }

    let aggregate = engine.aggregate_metrics();
    println!(
        "\nAggregate: throughput {:.2} tasks/tick, CPU utilization {:.0}%",
        aggregate.throughput(),
        aggregate.cpu_utilization() * 100.0
    );
}
