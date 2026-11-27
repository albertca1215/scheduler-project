use std::collections::{BTreeMap, HashSet, VecDeque};

/// The timing guarantee associated with a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeadlineType {
    /// Missing this deadline can cause the simulation to become visibly incorrect.
    Hard,
    /// Missing this deadline is acceptable, but the task should still run eventually.
    Soft,
    /// The task is released by an event rather than by a periodic timer.
    EventDriven,
}

/// The behavior represented by a task in the simulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BehaviorType {
    Movement,
    Chasing,
    Fleeing,
    Pouncing,
    Knockback,
    CollisionCheck,
    PeriodicColorChange,
    CollisionColorChange,
}

/// A unit of work that can be placed in a scheduler queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    pub id: u64,
    pub name: String,
    pub priority: u32,
    pub deadline: u64,
    pub deadline_type: DeadlineType,
    pub simulated_cost: u32,
    pub remaining_cost: u32,
    pub behavior_type: BehaviorType,
}

impl Task {
    #[must_use]
    pub fn new(
        id: u64,
        name: impl Into<String>,
        priority: u32,
        deadline: u64,
        deadline_type: DeadlineType,
        simulated_cost: u32,
        behavior_type: BehaviorType,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            priority,
            deadline,
            deadline_type,
            simulated_cost,
            remaining_cost: simulated_cost,
            behavior_type,
        }
    }

    #[must_use]
    pub fn is_overdue(&self, current_tick: u64) -> bool {
        current_tick > self.deadline
    }
}

/// Timing data collected while a task moves through the scheduler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskMetrics {
    pub task_id: u64,
    pub release_tick: u64,
    pub first_run_tick: Option<u64>,
    pub completion_tick: Option<u64>,
    pub service_ticks: u64,
    pub deadline_missed: bool,
}

impl TaskMetrics {
    #[must_use]
    pub fn new(task_id: u64, release_tick: u64) -> Self {
        Self {
            task_id,
            release_tick,
            first_run_tick: None,
            completion_tick: None,
            service_ticks: 0,
            deadline_missed: false,
        }
    }

    pub fn record_start(&mut self, tick: u64) {
        if self.first_run_tick.is_none() {
            self.first_run_tick = Some(tick);
        }
    }

    /// Records one simulation tick in which the task received service.
    pub fn record_service(&mut self, service_ticks: u64) {
        self.service_ticks += service_ticks;
    }

    pub fn record_completion(&mut self, tick: u64) {
        self.completion_tick = Some(tick);
    }

    pub fn record_deadline_miss(&mut self) {
        self.deadline_missed = true;
    }

    #[must_use]
    pub fn waiting_time(&self) -> Option<u64> {
        self.completion_tick
            .map(|completion| completion - self.release_tick - self.service_ticks)
    }

    #[must_use]
    pub fn turnaround_time(&self) -> Option<u64> {
        self.completion_tick
            .map(|completion| completion - self.release_tick)
    }

    #[must_use]
    pub fn response_time(&self) -> Option<u64> {
        self.first_run_tick
            .map(|first_run| first_run - self.release_tick)
    }
}

/// Aggregate measurements for one scheduler experiment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchedulerMetrics {
    pub elapsed_ticks: u64,
    pub busy_ticks: u64,
    pub completed_tasks: u64,
    pub work_units: u64,
    pub budget_units: u64,
}

impl SchedulerMetrics {
    #[must_use]
    pub fn new() -> Self {
        Self {
            elapsed_ticks: 0,
            busy_ticks: 0,
            completed_tasks: 0,
            work_units: 0,
            budget_units: 0,
        }
    }

    #[must_use]
    pub fn throughput(&self) -> f64 {
        if self.elapsed_ticks == 0 {
            0.0
        } else {
            self.completed_tasks as f64 / self.elapsed_ticks as f64
        }
    }

    #[must_use]
    pub fn cpu_utilization(&self) -> f64 {
        if self.elapsed_ticks == 0 {
            0.0
        } else {
            self.busy_ticks as f64 / self.elapsed_ticks as f64
        }
    }

    #[must_use]
    pub fn budget_utilization(&self) -> f64 {
        if self.budget_units == 0 {
            0.0
        } else {
            self.work_units as f64 / self.budget_units as f64
        }
    }
}

impl Default for SchedulerMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Scheduling work observed during one simulation tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TickReport {
    pub tick: u64,
    pub work_units: u32,
    pub idle_units: u32,
    pub selected_task_ids: Vec<u64>,
    pub completed_task_ids: Vec<u64>,
}

/// Executes scheduler selections against a fixed work budget each tick.
#[derive(Debug)]
pub struct FixedTickEngine<S> {
    scheduler: S,
    tick_budget: u32,
    current_tick: u64,
    metrics: BTreeMap<u64, TaskMetrics>,
    aggregate_metrics: SchedulerMetrics,
}

impl<S: Scheduler> FixedTickEngine<S> {
    #[must_use]
    pub fn new(scheduler: S, tick_budget: u32) -> Self {
        Self {
            scheduler,
            tick_budget,
            current_tick: 0,
            metrics: BTreeMap::new(),
            aggregate_metrics: SchedulerMetrics::new(),
        }
    }

    pub fn add_task(&mut self, task: Task) {
        self.metrics
            .insert(task.id, TaskMetrics::new(task.id, self.current_tick));
        self.scheduler.add_task(task);
    }

    #[must_use]
    pub fn current_tick(&self) -> u64 {
        self.current_tick
    }

    #[must_use]
    pub fn task_metrics(&self) -> &BTreeMap<u64, TaskMetrics> {
        &self.metrics
    }

    #[must_use]
    pub fn aggregate_metrics(&self) -> &SchedulerMetrics {
        &self.aggregate_metrics
    }

    #[must_use]
    pub fn pending_tasks(&self) -> usize {
        self.scheduler.pending_tasks()
    }

    pub fn run_tick(&mut self) -> TickReport {
        let tick = self.current_tick;
        let mut budget_remaining = self.tick_budget;
        let mut work_units = 0;
        let mut selected_task_ids = Vec::new();
        let mut completed_task_ids = Vec::new();
        let mut serviced_task_ids = HashSet::new();

        while budget_remaining > 0 {
            let Some(mut task) = self.scheduler.next_task() else {
                break;
            };
            selected_task_ids.push(task.id);
            let task_metrics = self
                .metrics
                .get_mut(&task.id)
                .expect("every scheduled task must have metrics");

            task_metrics.record_start(tick);
            if task.is_overdue(tick) {
                task_metrics.record_deadline_miss();
            }

            if task.remaining_cost == 0 {
                task_metrics.record_completion(tick);
                completed_task_ids.push(task.id);
                self.aggregate_metrics.completed_tasks += 1;
                continue;
            }

            let work = task
                .remaining_cost
                .min(budget_remaining)
                .min(self.scheduler.quantum());
            task.remaining_cost -= work;
            budget_remaining -= work;
            work_units += work;
            if work > 0 && serviced_task_ids.insert(task.id) {
                task_metrics.record_service(1);
            }

            if task.remaining_cost == 0 {
                let completion_tick = tick + 1;
                task_metrics.record_completion(completion_tick);
                if completion_tick > task.deadline {
                    task_metrics.record_deadline_miss();
                }
                completed_task_ids.push(task.id);
                self.aggregate_metrics.completed_tasks += 1;
            } else {
                self.scheduler.requeue_task(task);
            }
        }

        self.current_tick += 1;
        self.aggregate_metrics.elapsed_ticks += 1;
        self.aggregate_metrics.work_units += u64::from(work_units);
        self.aggregate_metrics.budget_units += u64::from(self.tick_budget);
        if work_units > 0 {
            self.aggregate_metrics.busy_ticks += 1;
        }

        TickReport {
            tick,
            work_units,
            idle_units: self.tick_budget - work_units,
            selected_task_ids,
            completed_task_ids,
        }
    }
}

/// Common interface for schedulers that select ready tasks for each tick.
pub trait Scheduler {
    /// Adds a ready task to the scheduler.
    fn add_task(&mut self, task: Task);

    /// Selects the next ready task, if one is available.
    fn next_task(&mut self) -> Option<Task>;

    /// Returns the number of tasks currently waiting to run.
    fn pending_tasks(&self) -> usize;

    /// Returns the scheduler's display name.
    fn name(&self) -> &'static str;

    /// Returns the maximum amount of work one selected task may receive.
    fn quantum(&self) -> u32 {
        u32::MAX
    }

    /// Requeues a task that was not completed during its time slice.
    fn requeue_task(&mut self, task: Task) {
        self.add_task(task);
    }
}

/// FIFO scheduler used as the baseline implementation.
#[derive(Debug, Default)]
pub struct FifoScheduler {
    queue: VecDeque<Task>,
}

impl FifoScheduler {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl Scheduler for FifoScheduler {
    fn add_task(&mut self, task: Task) {
        self.queue.push_back(task);
    }

    fn next_task(&mut self) -> Option<Task> {
        self.queue.pop_front()
    }

    fn pending_tasks(&self) -> usize {
        self.queue.len()
    }

    fn name(&self) -> &'static str {
        "FIFO"
    }
}

/// Priority scheduler that selects the lowest numeric priority value first.
#[derive(Debug, Default)]
pub struct PriorityScheduler {
    queues: BTreeMap<u32, VecDeque<Task>>,
}

impl PriorityScheduler {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl Scheduler for PriorityScheduler {
    fn add_task(&mut self, task: Task) {
        self.queues.entry(task.priority).or_default().push_back(task);
    }

    fn next_task(&mut self) -> Option<Task> {
        let mut selected_priority = None;
        for (priority, queue) in &mut self.queues {
            if !queue.is_empty() {
                selected_priority = Some(*priority);
                break;
            }
        }

        let priority = selected_priority?;
        let task = self.queues.get_mut(&priority)?.pop_front();
        if self.queues.get(&priority).is_some_and(VecDeque::is_empty) {
            self.queues.remove(&priority);
        }
        task
    }

    fn pending_tasks(&self) -> usize {
        self.queues.values().map(VecDeque::len).sum()
    }

    fn name(&self) -> &'static str {
        "Priority"
    }
}

/// Round Robin scheduler that rotates unfinished tasks to the back of the queue.
#[derive(Debug)]
pub struct RoundRobinScheduler {
    queue: VecDeque<Task>,
    quantum: u32,
}

impl RoundRobinScheduler {
    #[must_use]
    pub fn new(quantum: u32) -> Self {
        assert!(quantum > 0, "Round Robin quantum must be greater than zero");
        Self {
            queue: VecDeque::new(),
            quantum,
        }
    }

    #[must_use]
    pub fn quantum(&self) -> u32 {
        self.quantum
    }
}

impl Scheduler for RoundRobinScheduler {
    fn add_task(&mut self, task: Task) {
        self.queue.push_back(task);
    }

    fn next_task(&mut self) -> Option<Task> {
        self.queue.pop_front()
    }

    fn pending_tasks(&self) -> usize {
        self.queue.len()
    }

    fn name(&self) -> &'static str {
        "Round Robin"
    }

    fn quantum(&self) -> u32 {
        self.quantum
    }

    fn requeue_task(&mut self, task: Task) {
        self.queue.push_back(task);
    }
}

#[derive(Debug)]
pub enum SchedulerPolicy {
    Fifo(FifoScheduler),
    Priority(PriorityScheduler),
    RoundRobin(RoundRobinScheduler),
}

impl SchedulerPolicy {
    #[must_use]
    pub fn fifo() -> Self {
        Self::Fifo(FifoScheduler::new())
    }

    #[must_use]
    pub fn priority() -> Self {
        Self::Priority(PriorityScheduler::new())
    }

    #[must_use]
    pub fn round_robin(quantum: u32) -> Self {
        Self::RoundRobin(RoundRobinScheduler::new(quantum))
    }

    fn queued_tasks(&self) -> Vec<&Task> {
        match self {
            Self::Fifo(scheduler) => scheduler.queue.iter().collect(),
            Self::Priority(scheduler) => scheduler
                .queues
                .values()
                .flat_map(|queue| queue.iter())
                .collect(),
            Self::RoundRobin(scheduler) => scheduler.queue.iter().collect(),
        }
    }
}

impl FixedTickEngine<SchedulerPolicy> {
    /// Returns tasks still queued in scheduler selection order.
    #[must_use]
    pub fn queued_tasks(&self) -> Vec<&Task> {
        self.scheduler.queued_tasks()
    }
}

impl Scheduler for SchedulerPolicy {
    fn add_task(&mut self, task: Task) {
        match self {
            Self::Fifo(scheduler) => scheduler.add_task(task),
            Self::Priority(scheduler) => scheduler.add_task(task),
            Self::RoundRobin(scheduler) => scheduler.add_task(task),
        }
    }

    fn next_task(&mut self) -> Option<Task> {
        match self {
            Self::Fifo(scheduler) => scheduler.next_task(),
            Self::Priority(scheduler) => scheduler.next_task(),
            Self::RoundRobin(scheduler) => scheduler.next_task(),
        }
    }

    fn pending_tasks(&self) -> usize {
        match self {
            Self::Fifo(scheduler) => scheduler.pending_tasks(),
            Self::Priority(scheduler) => scheduler.pending_tasks(),
            Self::RoundRobin(scheduler) => scheduler.pending_tasks(),
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Self::Fifo(scheduler) => scheduler.name(),
            Self::Priority(scheduler) => scheduler.name(),
            Self::RoundRobin(scheduler) => scheduler.name(),
        }
    }

    fn requeue_task(&mut self, task: Task) {
        match self {
            Self::Fifo(scheduler) => scheduler.requeue_task(task),
            Self::Priority(scheduler) => scheduler.requeue_task(task),
            Self::RoundRobin(scheduler) => scheduler.requeue_task(task),
        }
    }

    fn quantum(&self) -> u32 {
        match self {
            Self::Fifo(scheduler) => scheduler.quantum(),
            Self::Priority(scheduler) => scheduler.quantum(),
            Self::RoundRobin(scheduler) => scheduler.quantum(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(id: u64) -> Task {
        Task::new(
            id,
            format!("task-{id}"),
            1,
            10,
            DeadlineType::Soft,
            1,
            BehaviorType::Movement,
        )
    }

    #[test]
    fn task_records_scheduler_metadata() {
        let task = Task::new(
            7,
            "wall collision",
            10,
            20,
            DeadlineType::Hard,
            3,
            BehaviorType::CollisionCheck,
        );

        assert_eq!(task.id, 7);
        assert_eq!(task.name, "wall collision");
        assert_eq!(task.deadline_type, DeadlineType::Hard);
        assert!(!task.is_overdue(20));
        assert!(task.is_overdue(21));
    }

    #[test]
    fn fifo_returns_tasks_in_insertion_order() {
        let mut scheduler = FifoScheduler::new();
        scheduler.add_task(task(1));
        scheduler.add_task(task(2));

        assert_eq!(scheduler.name(), "FIFO");
        assert_eq!(scheduler.pending_tasks(), 2);
        assert_eq!(scheduler.next_task().map(|task| task.id), Some(1));
        assert_eq!(scheduler.next_task().map(|task| task.id), Some(2));
        assert_eq!(scheduler.next_task(), None);
    }

    #[test]
    fn incomplete_tasks_can_be_requeued_through_the_trait() {
        let mut scheduler = FifoScheduler::new();
        scheduler.add_task(task(1));

        let unfinished = scheduler.next_task().expect("task should be ready");
        scheduler.requeue_task(unfinished);

        assert_eq!(scheduler.next_task().map(|task| task.id), Some(1));
    }

    #[test]
    fn priority_returns_lowest_numeric_priority_first() {
        let mut scheduler = PriorityScheduler::new();
        let mut low_priority = task(1);
        low_priority.priority = 20;
        let mut high_priority = task(2);
        high_priority.priority = 5;
        scheduler.add_task(low_priority);
        scheduler.add_task(high_priority);

        assert_eq!(scheduler.name(), "Priority");
        assert_eq!(scheduler.next_task().map(|task| task.id), Some(2));
        assert_eq!(scheduler.next_task().map(|task| task.id), Some(1));
    }

    #[test]
    fn round_robin_requeues_unfinished_tasks_to_the_back() {
        let mut scheduler = RoundRobinScheduler::new(10);
        scheduler.add_task(task(1));
        scheduler.add_task(task(2));

        let unfinished = scheduler.next_task().expect("first task should be ready");
        assert_eq!(unfinished.id, 1);
        scheduler.requeue_task(unfinished);

        assert_eq!(scheduler.next_task().map(|task| task.id), Some(2));
        assert_eq!(scheduler.next_task().map(|task| task.id), Some(1));
    }

    #[test]
    fn queued_tasks_follow_policy_order_after_partial_work() {
        for (policy, expected) in [
            (SchedulerPolicy::fifo(), vec![2, 1]),
            (SchedulerPolicy::priority(), vec![1, 2]),
            (SchedulerPolicy::round_robin(1), vec![2, 1]),
        ] {
            let mut engine = FixedTickEngine::new(policy, 1);
            let mut first = task(1);
            first.simulated_cost = 2;
            first.remaining_cost = 2;
            let mut second = task(2);
            second.priority = 10;
            engine.add_task(first);
            engine.add_task(second);
            engine.run_tick();
            let ids: Vec<_> = engine.queued_tasks().iter().map(|task| task.id).collect();
            assert_eq!(ids, expected);
            while engine.pending_tasks() > 0 {
                engine.run_tick();
            }
            assert!(engine.queued_tasks().is_empty());
        }
    }

    #[test]
    fn round_robin_quantum_limits_work_before_requeueing() {
        let mut engine = FixedTickEngine::new(RoundRobinScheduler::new(2), 5);
        engine.add_task(Task::new(
            1,
            "long task",
            1,
            10,
            DeadlineType::Soft,
            5,
            BehaviorType::Movement,
        ));
        engine.add_task(Task::new(
            2,
            "short task",
            1,
            10,
            DeadlineType::Soft,
            1,
            BehaviorType::Movement,
        ));

        let report = engine.run_tick();

        assert_eq!(report.work_units, 5);
        assert_eq!(report.completed_task_ids, vec![2]);
        assert_eq!(engine.pending_tasks(), 1);
        assert_eq!(report.selected_task_ids, vec![1, 2, 1]);
        assert_eq!(engine.task_metrics()[&1].service_ticks, 1);
        assert_eq!(engine.task_metrics()[&2].completion_tick, Some(1));

        let second_tick = engine.run_tick();
        assert_eq!(second_tick.completed_task_ids, vec![1]);
        let metrics = &engine.task_metrics()[&1];
        assert_eq!(metrics.service_ticks, 2);
        assert_eq!(metrics.waiting_time(), Some(0));
        assert_eq!(metrics.turnaround_time(), Some(2));
    }

    #[test]
    fn task_metrics_calculate_standard_scheduler_times() {
        let mut metrics = TaskMetrics::new(1, 3);
        metrics.record_start(7);
        metrics.record_service(4);
        metrics.record_completion(15);

        assert_eq!(metrics.response_time(), Some(4));
        assert_eq!(metrics.turnaround_time(), Some(12));
        assert_eq!(metrics.waiting_time(), Some(8));
    }

    #[test]
    fn scheduler_metrics_calculate_throughput_and_utilization() {
        let metrics = SchedulerMetrics {
            elapsed_ticks: 10,
            busy_ticks: 7,
            completed_tasks: 4,
            work_units: 28,
            budget_units: 40,
        };

        assert_eq!(metrics.throughput(), 0.4);
        assert_eq!(metrics.cpu_utilization(), 0.7);
        assert_eq!(metrics.budget_utilization(), 0.7);
        assert_eq!(SchedulerMetrics::new().budget_utilization(), 0.0);
    }

    #[test]
    fn fixed_tick_engine_tracks_partial_work_and_metrics() {
        let mut engine = FixedTickEngine::new(FifoScheduler::new(), 2);
        engine.add_task(Task::new(
            1,
            "long task",
            1,
            10,
            DeadlineType::Soft,
            3,
            BehaviorType::Movement,
        ));

        let first_tick = engine.run_tick();
        assert_eq!(first_tick.work_units, 2);
        assert_eq!(first_tick.idle_units, 0);
        assert!(first_tick.completed_task_ids.is_empty());

        let second_tick = engine.run_tick();
        assert_eq!(second_tick.work_units, 1);
        assert_eq!(second_tick.idle_units, 1);
        assert_eq!(second_tick.completed_task_ids, vec![1]);

        let task_metrics = &engine.task_metrics()[&1];
        assert_eq!(task_metrics.response_time(), Some(0));
        assert_eq!(task_metrics.turnaround_time(), Some(2));
        assert_eq!(task_metrics.waiting_time(), Some(0));
        assert_eq!(engine.aggregate_metrics().completed_tasks, 1);
        assert_eq!(engine.aggregate_metrics().busy_ticks, 2);
        assert_eq!(engine.aggregate_metrics().work_units, 3);
        assert_eq!(engine.aggregate_metrics().budget_units, 4);
        assert_eq!(engine.aggregate_metrics().budget_utilization(), 0.75);

        engine.run_tick();
        assert_eq!(engine.aggregate_metrics().budget_utilization(), 0.5);
    }
}
