use scheduler_project::scheduler::{
    BehaviorType, DeadlineType, FixedTickEngine, SchedulerPolicy, Task,
};
use std::io::{self, Write};

const DEFAULT_TICK_BUDGET: u32 = 4;
const DEFAULT_RR_QUANTUM: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PolicyChoice {
    Fifo,
    Priority,
    RoundRobin,
}

impl PolicyChoice {
    fn name(self) -> &'static str {
        match self {
            Self::Fifo => "FIFO",
            Self::Priority => "Priority",
            Self::RoundRobin => "Round Robin",
        }
    }

    fn scheduler(self, rr_quantum: u32) -> SchedulerPolicy {
        match self {
            Self::Fifo => SchedulerPolicy::fifo(),
            Self::Priority => SchedulerPolicy::priority(),
            Self::RoundRobin => SchedulerPolicy::round_robin(rr_quantum),
        }
    }
}

fn encounter_tasks() -> Vec<Task> {
    vec![
        Task::new(
            1,
            "Meep collision check",
            1,
            2,
            DeadlineType::Hard,
            1,
            BehaviorType::CollisionCheck,
        ),
        Task::new(
            2,
            "Meep movement",
            10,
            8,
            DeadlineType::Soft,
            1,
            BehaviorType::Movement,
        ),
        Task::new(
            3,
            "Beep collision check",
            1,
            2,
            DeadlineType::Hard,
            1,
            BehaviorType::CollisionCheck,
        ),
        Task::new(
            4,
            "Beep chase",
            4,
            7,
            DeadlineType::Soft,
            2,
            BehaviorType::Chasing,
        ),
        Task::new(
            5,
            "Peep collision check",
            1,
            2,
            DeadlineType::Hard,
            1,
            BehaviorType::CollisionCheck,
        ),
        Task::new(
            6,
            "Peep flee",
            5,
            7,
            DeadlineType::Soft,
            2,
            BehaviorType::Fleeing,
        ),
        Task::new(
            7,
            "Beep pounce",
            3,
            6,
            DeadlineType::Hard,
            3,
            BehaviorType::Pouncing,
        ),
        Task::new(
            8,
            "Peep knockback",
            2,
            6,
            DeadlineType::Hard,
            3,
            BehaviorType::Knockback,
        ),
    ]
}

fn task_names(report_ids: &[u64]) -> String {
    if report_ids.is_empty() {
        "none".to_string()
    } else {
        report_ids
            .iter()
            .map(|task_id| format!("Task {task_id}"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn print_workload() {
    println!("Shared task workload:");
    for task in encounter_tasks() {
        println!(
            "  Task {}: priority {:2}, cost {}, deadline {}",
            task.id, task.priority, task.simulated_cost, task.deadline
        );
    }
}

fn run_demo(policy: PolicyChoice, tick_budget: u32, rr_quantum: u32) {
    let tasks = encounter_tasks();
    let task_count = tasks.len() as u64;
    let mut engine = FixedTickEngine::new(policy.scheduler(rr_quantum), tick_budget);

    for task in tasks {
        engine.add_task(task);
    }

    println!(
        "\n=== {} | budget {}{} ===",
        policy.name(),
        tick_budget,
        if policy == PolicyChoice::RoundRobin {
            format!(" | RR quantum {}", rr_quantum)
        } else {
            String::new()
        }
    );

    print_workload();
    println!("Tick execution:");

    while engine.aggregate_metrics().completed_tasks < task_count {
        let report = engine.run_tick();
        let incomplete = report
            .selected_task_ids
            .iter()
            .filter(|task_id| !report.completed_task_ids.contains(task_id))
            .copied()
            .collect::<Vec<_>>();

        println!(
            "  t{:2} | started: {:47} | completed: {:47} | incomplete: {:47} | queued: {}",
            report.tick,
            task_names(&report.selected_task_ids),
            task_names(&report.completed_task_ids),
            task_names(&incomplete),
            engine.pending_tasks()
        );
    }

    println!("\nTask metrics:");
    for (task_id, metrics) in engine.task_metrics() {
        println!(
            "  Task {:2}: response {:?}, waiting {:?}, turnaround {:?}, deadline missed {}",
            task_id,
            metrics.response_time(),
            metrics.waiting_time(),
            metrics.turnaround_time(),
            metrics.deadline_missed
        );
    }

    let aggregate = engine.aggregate_metrics();
    println!(
        "Aggregate: throughput {:.2} tasks/tick, CPU utilization {:.0}%\n",
        aggregate.throughput(),
        aggregate.cpu_utilization() * 100.0
    );
}

fn usage() {
    println!("Usage:");
    println!("  cargo run --example scheduler_demo");
    println!("  cargo run --example scheduler_demo -- fifo <budget>");
    println!("  cargo run --example scheduler_demo -- priority <budget>");
    println!("  cargo run --example scheduler_demo -- rr <budget> <quantum>");
    println!("  cargo run --example scheduler_demo -- interactive");
}

fn parse_positive(label: &str, value: &str) -> Result<u32, String> {
    let parsed = value
        .parse::<u32>()
        .map_err(|_| format!("{label} must be a positive integer"))?;
    if parsed == 0 {
        Err(format!("{label} must be greater than zero"))
    } else {
        Ok(parsed)
    }
}

fn read_input(prompt: &str) -> io::Result<String> {
    print!("{prompt}");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

fn run_interactive() {
    println!("Scheduler demo");
    println!("1 = FIFO");
    println!("2 = Priority");
    println!("3 = Round Robin");

    let policy = loop {
        match read_input("Select policy (1-3): ") {
            Ok(input) => match input.as_str() {
                "1" => break PolicyChoice::Fifo,
                "2" => break PolicyChoice::Priority,
                "3" => break PolicyChoice::RoundRobin,
                _ => println!("Please enter 1, 2, or 3."),
            },
            Err(error) => {
                println!("Could not read input: {error}");
                return;
            }
        }
    };

    let tick_budget = loop {
        match read_input(&format!("Tick budget (default {DEFAULT_TICK_BUDGET}): ")) {
            Ok(input) if input.is_empty() => break DEFAULT_TICK_BUDGET,
            Ok(input) => match parse_positive("Tick budget", &input) {
                Ok(value) => break value,
                Err(error) => println!("{error}."),
            },
            Err(error) => {
                println!("Could not read input: {error}");
                return;
            }
        }
    };

    let rr_quantum = if policy == PolicyChoice::RoundRobin {
        loop {
            match read_input(&format!("RR quantum (default {DEFAULT_RR_QUANTUM}): ")) {
                Ok(input) if input.is_empty() => break DEFAULT_RR_QUANTUM,
                Ok(input) => match parse_positive("RR quantum", &input) {
                    Ok(value) => break value,
                    Err(error) => println!("{error}."),
                },
                Err(error) => {
                    println!("Could not read input: {error}");
                    return;
                }
            }
        }
    } else {
        DEFAULT_RR_QUANTUM
    };

    run_demo(policy, tick_budget, rr_quantum);
}

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();

    if args.is_empty() {
        println!("Running the default three-policy comparison.");
        println!("Use --interactive for text-based selection.");
        run_demo(PolicyChoice::Fifo, DEFAULT_TICK_BUDGET, DEFAULT_RR_QUANTUM);
        run_demo(
            PolicyChoice::Priority,
            DEFAULT_TICK_BUDGET,
            DEFAULT_RR_QUANTUM,
        );
        run_demo(
            PolicyChoice::RoundRobin,
            DEFAULT_TICK_BUDGET,
            DEFAULT_RR_QUANTUM,
        );
        return;
    }

    if args.len() == 1
        && (args[0].eq_ignore_ascii_case("--interactive")
            || args[0].eq_ignore_ascii_case("interactive"))
    {
        run_interactive();
        return;
    }

    if args.len() == 1 && (args[0] == "-h" || args[0] == "--help") {
        usage();
        return;
    }

    let policy = match args[0].to_ascii_lowercase().as_str() {
        "fifo" => PolicyChoice::Fifo,
        "priority" => PolicyChoice::Priority,
        "rr" | "round-robin" | "round_robin" => PolicyChoice::RoundRobin,
        _ => {
            eprintln!("Unknown policy: {}", args[0]);
            usage();
            std::process::exit(2);
        }
    };

    let tick_budget = match args.get(1) {
        Some(value) => match parse_positive("Tick budget", value) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("{error}");
                usage();
                std::process::exit(2);
            }
        },
        None => {
            eprintln!("Missing tick budget");
            usage();
            std::process::exit(2);
        }
    };

    let rr_quantum = if policy == PolicyChoice::RoundRobin {
        match args.get(2) {
            Some(value) => match parse_positive("RR quantum", value) {
                Ok(value) => value,
                Err(error) => {
                    eprintln!("{error}");
                    usage();
                    std::process::exit(2);
                }
            },
            None => DEFAULT_RR_QUANTUM,
        }
    } else {
        if args.len() > 2 {
            eprintln!("Unexpected arguments for {}", policy.name());
            usage();
            std::process::exit(2);
        }
        DEFAULT_RR_QUANTUM
    };

    if policy == PolicyChoice::RoundRobin && args.len() > 3 {
        eprintln!("Too many arguments");
        usage();
        std::process::exit(2);
    }

    run_demo(policy, tick_budget, rr_quantum);
}

#[cfg(test)]
mod tests {
    use super::{encounter_tasks, task_names};
    use scheduler_project::scheduler::{FixedTickEngine, SchedulerPolicy};

    #[test]
    fn task_labels_use_ids_in_selection_order() {
        assert_eq!(task_names(&[8, 4, 7]), "Task 8, Task 4, Task 7");
        assert_eq!(task_names(&[]), "none");
    }

    #[test]
    fn round_robin_multiple_selections_record_distinct_service_ticks() {
        let mut engine = FixedTickEngine::new(SchedulerPolicy::round_robin(1), 8);
        for task in encounter_tasks() {
            engine.add_task(task);
        }

        engine.run_tick();
        let report = engine.run_tick();
        assert_eq!(report.selected_task_ids, vec![4, 6, 7, 8, 7, 8]);
        assert_eq!(report.completed_task_ids, vec![4, 6, 7, 8]);
        for task_id in [7, 8] {
            let metrics = &engine.task_metrics()[&task_id];
            assert_eq!(metrics.service_ticks, 2);
            assert_eq!(metrics.waiting_time(), Some(0));
            assert_eq!(metrics.turnaround_time(), Some(2));
            assert_eq!(metrics.response_time(), Some(0));
        }
    }
}
