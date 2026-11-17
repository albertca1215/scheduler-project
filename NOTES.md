# Simple Scheduler

This project demonstrates how a CPU scheduler can coordinate work for multiple
ongoing entities. It is written in Rust with Macroquad 0.4.16 and currently
simulates two independently tracked hamsters.

## Scheduler foundation

The initial scheduler model is in `src/scheduler.rs`:

- `Task` stores an ID, name, priority, deadline tick, deadline type, simulated cost, and behavior type.
- `DeadlineType` distinguishes hard, soft, and event-driven work.
- `BehaviorType` identifies movement, chasing, fleeing, pouncing, knockback, collision checks, periodic color changes, and collision-triggered color changes. The color behaviors are retained for the examples but are not used by the main application.
- `Scheduler` defines the queue operations required by scheduler implementations.
- `FifoScheduler` is the baseline implementation and supports re-queuing unfinished work.

The current deadline value is an absolute tick. A task is overdue when the current tick is greater than its deadline.

The current application uses FIFO scheduling only. Round Robin, Priority, and
MLFQ policies are not implemented yet.

The main application now also includes waypoint-based steering. Hamsters choose
new random waypoints during normal movement and smoothly turn toward them rather
than following one fixed straight line.

## Try it yourself

The scheduler has a human-readable command-line demo. From the project root, run:

```powershell
cargo run --example scheduler_demo
```

It adds three tasks, prints their metadata, and shows the order in which FIFO
runs them. The simulated cost advances the displayed tick, so you can edit
the task costs or deadlines in `examples/scheduler_demo.rs` and immediately
see how the results change.

To run the automated checks instead:

```powershell
cargo test --lib
```

To run the single-entity Macroquad example where movement, collision checks, and
collision-triggered color changes are scheduled as tasks:

```powershell
cargo run --example task_entity
```

The main application runs two independently tracked, named hamsters. Each
hamster has separate movement and collision-check tasks, direction, velocity,
and collision counter. Hamsters bounce from window boundaries and collide with
each other by separating and exchanging velocities; each hamster records the
collision:

```powershell
cargo run
```

The main application runs one scheduler work unit per simulation step so FIFO
context switches are visible. The HUD shows the active task, scheduler tick,
completed-task totals, each hamster's direction and velocity, and its collision
count.

Controls:

- `Up` decreases the step interval and speeds up task execution.
- `Down` increases the step interval and slows down task execution.
- `Space` pauses or resumes the simulation.

The interval ranges from `0.001` seconds to `1.0` seconds. The minimum setting
is displayed as `REAL-TIME`. The application uses a monotonic clock, limits
catch-up work after a long frame or window operation, and prevents queued task
backlog from creating an artificial burst.

When the hamsters collide, one is randomly selected as the predator and the
other becomes the prey. The predator steers toward the prey for five simulated
seconds. The prey continues waypoint movement, and both hamsters move at 1.5
times their normal speed until the chase ends. The fleeing hamster continues
waypoint movement at twice its normal speed. The HUD identifies chasing and
fleeing states and shows the remaining chase time. An active chase is not
restarted by repeated collisions. Hamster-to-hamster collision overlap is
separated once and guarded by a 0.75-second cooldown so that repeated contacts
do not produce infinite velocity swaps or attach the hamsters together.

A three-hamster encounter includes **Meep**, **Beep**, and **Peep**. A fleeing
hamster moves away from the closest hamster rather than following a fixed
waypoint. The other hamster in a collision randomly chooses either Chase or
Pounce. Pounce pauses for 0.5 to 1.0 seconds, then launches at 3.5 times normal
speed. When a chase or pounce collision reaches the fleeing hamster, the prey is
punched into Knockback at 5 times normal speed, bounces linearly off walls and
objects, decays for each collision and over time, then returns to normal behavior.

## Timing model

The project distinguishes between a simulation tick and a scheduler quantum:

- A **simulation tick** is one call to `FixedTickEngine::run_tick`. It selects
  ready work and advances scheduler metrics.
- A **scheduler quantum** is the amount of simulated work the scheduler may
  execute during one tick. The main application uses a budget of one work unit,
  making each FIFO task visible.
- A task's `simulated_cost` describes how many work units it needs. If the remaining budget is too small, the task must continue during a later tick or be re-queued according to the active policy.

The current interactive demonstration focuses on FIFO. The next scheduler
policies are planned as:

- **Priority:** select the ready task with the highest scheduling precedence.
  The current design uses lower numeric priority values as the higher
  precedence.
- **Round Robin:** run tasks for a fixed quantum, then re-queue unfinished work
  so that additional activities become visible through repeated context
  switches. The initial proposed quantum is 10 work units.
- **MLFQ:** future work after FIFO, Priority, and Round Robin.

## Metrics

The scheduler model includes `TaskMetrics` for per-task measurements:

- **Waiting time:** completion tick minus release tick minus the simulation ticks in which the task received service.
- **Turnaround time:** completion tick minus release tick.
- **Response time:** first-run tick minus release tick.
- **Deadline miss:** recorded when a task is not completed by its deadline.

`SchedulerMetrics` stores experiment-wide elapsed ticks, busy ticks, and completed tasks. It derives:

- **Throughput:** completed tasks per elapsed tick.
- **CPU utilization:** busy ticks divided by elapsed ticks.

`FixedTickEngine<S>` populates these values while consuming task
`remaining_cost`, re-queuing incomplete tasks, and recording per-tick reports.
The command-line demo prints these metrics, and the main Macroquad application
displays completed-task counts and the current scheduler tick.