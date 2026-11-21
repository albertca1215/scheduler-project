use scheduler_project::scheduler::{
    BehaviorType, DeadlineType, FifoScheduler, FixedTickEngine, PriorityScheduler,
    RoundRobinScheduler, Scheduler, Task,
};
use std::collections::BTreeMap;

const TICK_BUDGET: u32 = 4;
const ROUND_ROBIN_QUANTUM: u32 = 2;

fn encounter_tasks() -> Vec<Task> {
    vec![
        Task::new(
            1,
            "Move Meep",
            10,
            8,
            DeadlineType::Hard,
            6,
            BehaviorType::Movement,
        ),
        Task::new(
            2,
            "Check Meep collision",
            1,
            3,
            DeadlineType::Hard,
            2,
            BehaviorType::CollisionCheck,
        ),
        Task::new(
            3,
            "Chase Beep",
            4,
            8,
            DeadlineType::Soft,
            4,
            BehaviorType::Chasing,
        ),
        Task::new(
            4,
            "Flee Peep",
            5,
            8,
            DeadlineType::Soft,
            4,
            BehaviorType::Fleeing,
        ),
        Task::new(
            5,
            "Pounce Beep",
            3,
            6,
            DeadlineType::EventDriven,
            3,
            BehaviorType::Pouncing,
        ),
        Task::new(
            6,
            "Knockback Peep",
            2,
            6,
            DeadlineType::Hard,
            3,
            BehaviorType::Knockback,
        ),
    ]
}

fn task_names(tasks: &[Task]) -> BTreeMap<u64, &str> {
    tasks
        .iter()
        .map(|task| (task.id, task.name.as_str()))
        .collect()
}

fn describe_ids(ids: &[u64], names: &BTreeMap<u64, &str>) -> String {
    if ids.is_empty() {
        return "none".to_string();
    }

    ids.iter()
        .map(|id| names.get(id).copied().unwrap_or("unknown task"))
        .collect::<Vec<_>>()
        .join(" -> ")
}

fn run_experiment<S: Scheduler>(label: &str, scheduler: S, tasks: &[Task]) {
    let names = task_names(tasks);
    let mut engine = FixedTickEngine::new(scheduler, TICK_BUDGET);

    for task in tasks {
        engine.add_task(task.clone());
    }

    println!("\n=== {label} ===");
    println!(
        "tick budget: {TICK_BUDGET} work units{}",
        if label == "Round Robin" {
            format!(", quantum: {ROUND_ROBIN_QUANTUM}")
        } else {
            String::new()
        }
    );
    println!("Tick trace:");

    while engine.pending_tasks() > 0 {
        let report = engine.run_tick();
        println!(
            "  t{:>2}: work {}/{} | selected: {} | completed: {} | queued: {}",
            report.tick,
            report.work_units,
            TICK_BUDGET,
            describe_ids(&report.selected_task_ids, &names),
            describe_ids(&report.completed_task_ids, &names),
            engine.pending_tasks(),
        );
    }

    println!("Task metrics:");
    for task in tasks {
        let metrics = &engine.task_metrics()[&task.id];
        println!(
            "  {:<20} response {:>2?} | waiting {:>2?} | turnaround {:>2?} | missed {}",
            task.name,
            metrics.response_time(),
            metrics.waiting_time(),
            metrics.turnaround_time(),
            metrics.deadline_missed,
        );
    }

    let aggregate = engine.aggregate_metrics();
    println!(
        "Aggregate: {} ticks | throughput {:.2} tasks/tick | CPU utilization {:.0}%",
        aggregate.elapsed_ticks,
        aggregate.throughput(),
        aggregate.cpu_utilization() * 100.0,
    );
}

fn main() {
    let tasks = encounter_tasks();

    println!("Scheduler comparison: deterministic hamster encounter");
    println!("The same ready-task queue is run through every policy.");
    println!("Lower numeric priority values represent higher precedence.\n");
    println!("Workload:");
    for task in &tasks {
        println!(
            "  [{}] {:<20} priority {:>2} | deadline {:>2} | cost {}",
            task.id, task.name, task.priority, task.deadline, task.simulated_cost
        );
    }

    run_experiment("FIFO", FifoScheduler::new(), &tasks);
    run_experiment("Priority", PriorityScheduler::new(), &tasks);
    run_experiment(
        "Round Robin",
        RoundRobinScheduler::new(ROUND_ROBIN_QUANTUM),
        &tasks,
    );
}
