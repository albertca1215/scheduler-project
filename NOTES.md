# Simple Scheduler

This project demonstrates how a CPU scheduler can coordinate work for multiple
ongoing entities. It is written in Rust with Macroquad 0.4.16 and supports up
to three independently tracked hamsters.

## Scheduler foundation

The initial scheduler model is in `src/scheduler.rs`:

- `Task` stores an ID, name, priority, deadline tick, deadline type, simulated cost, and behavior type.
- `DeadlineType` distinguishes hard, soft, and event-driven work.
- `BehaviorType` identifies movement, chasing, fleeing, pouncing, knockback, collision checks, periodic color changes, and collision-triggered color changes. The color behaviors are retained for the examples but are not used by the main application.
- `Scheduler` defines the queue operations required by scheduler implementations.
- `FifoScheduler` selects from the front and returns unfinished work to the
  back when the tick budget runs out. It is not strict non-preemptive FIFO.

The current deadline value is an absolute tick. A task is overdue when the current tick is greater than its deadline.

The application supports FIFO, Priority, and Round Robin scheduling. MLFQ is
not implemented yet.

The main application now also includes waypoint-based steering. Hamsters choose
new random waypoints during normal movement and smoothly turn toward them rather
than following one fixed straight line.

## Try it yourself

The scheduler has a human-readable command-line comparison demo. A variety of prototypes were worked on in the process of getting accustomed to Macroquad. From the project root, run:

```powershell
cargo run --example scheduler_demo
cargo run --example scheduler_demo -- interactive
cargo run --example scheduler_demo -- fifo 4
cargo run --example scheduler_demo -- priority 4
cargo run --example scheduler_demo -- rr 4 2
```

Running with no arguments performs a deterministic three-policy comparison using
a budget of four work units and an RR quantum of two. `interactive` opens a
text-based prompt for policy, budget, and, for Round Robin, quantum. Direct
policy commands provide reproducible one-policy runs for screenshots and tables.

The demo uses the same fixed eight-task workload for FIFO, Priority, and Round
Robin. All tasks enter the queue at the start; no new tasks arrive during the
run. It records tasks started, completed, and left incomplete in
each tick, along with queue depth and per-task timing metrics. It also prints
aggregate throughput and budget utilization. Per-task output shows response,
waiting, and turnaround times in ticks; deadline misses are not displayed.

To run the automated checks instead:

```powershell
cargo test --lib
```

To run the single-entity Macroquad example where movement, collision checks, and
collision-triggered color changes are scheduled as tasks:

```powershell
cargo run --example task_entity
```

The main application runs up to three independently tracked, named hamsters.
Each hamster has separate movement or encounter tasks and collision-check tasks,
direction, velocity, and collision counter. Hamsters bounce from window
boundaries and collide with each other by separating and exchanging velocities;
each hamster records the collision:

```powershell
cargo run
```

The main application uses a four-unit scheduler budget per simulation step.
The HUD shows the scheduler status, scheduler tick, budget, completed-task
totals, each hamster's direction and velocity, and its collision count.
The scheduler log records tasks started, completed, and left incomplete at the
end of each tick. The tick-status area remains on the last paused tick while
running and displays "Waiting for pause" during continuous playback.

Controls:

- `Up` decreases the step interval and speeds up task execution.
- `Down` increases the step interval and slows down task execution.
- `Space` pauses or resumes the simulation.
- `A` or the **Add** button adds the next available hamster.
- `R` or the **Remove** button removes the last added hamster.
  **Meep** remains the default and is never removed.
- `F1` switches to FIFO.
- `F2` switches to Priority.
- `F3` switches to Round Robin.
- `Q`/`E` decrease/increase the scheduler tick budget from 1 to 16.
- `Z`/`X` decrease/increase the Round Robin quantum from 1 to 8.

Changing the scheduling type, tick budget, or Round Robin quantum resets the
simulation to one **Meep** hamster, zeroes scheduler and simulation counters,
restores the default step interval, and clears the scheduler log and tick
status. The current paused or playing state is preserved.

The interval ranges from `0.025` seconds to `1.0` seconds. The fastest
setting displays as `0.025s`. The application uses a monotonic clock, limits
catch-up work after a long frame or window operation, and prevents queued task
backlog from creating an artificial burst.

The sandbox starts with **Meep** only. **Beep** and **Peep** can be added or
removed through the in-window GUI or keyboard controls. A removed hamster has
no movement, collision, rendering, or scheduled-task logic while absent.

When the hamsters collide, one is randomly selected as the predator and the
other becomes the prey. The predator steers toward the prey for five simulated
seconds. The prey continues waypoint movement, and both hamsters move at 1.5
times their normal speed until the chase ends. The fleeing hamster continues
waypoint movement at twice its normal speed. The HUD identifies chasing and
fleeing states and shows the remaining chase time. An active chase is not
restarted by repeated collisions. A 0.75-second cooldown per hamster pair
prevents repeated collision events and encounter triggers, without blocking
either hamster from colliding with a third. Overlaps are still separated and
approaching hamsters bounce during the cooldown. This is to prevent a pair of
hamsters from infinitely bumping into one another and restarting a chase
sequence ad infiniutm, as entertaining as it might be to watch a Beyblade-style
match between hamsters.

Knockback impacts now use an equal-mass pool-ball impulse model. The vector
between two overlapping hamsters is the contact normal. Incoming velocity is
split into normal and tangent components: the normal component is transferred
to the struck hamster, and the tangent component is retained for a glancing
deflection. The 0.92 restitution factor reduces transferred speed while wall
impacts and subsequent knockback collisions continue to decay velocity until
the struck hamster returns to normal movement.

A three-hamster encounter includes **Meep**, **Beep**, and **Peep**. A fleeing
hamster moves away from the closest hamster rather than following a fixed
waypoint. The other hamster in a collision randomly chooses either Chase or
Pounce. Pounce pauses for 0.5 to 1.0 seconds, then launches at 3.5 times normal
speed. When a chase or pounce collision reaches the fleeing hamster, the prey is
punched into Knockback at 5 times normal speed, bounces linearly off walls and
objects, decays for each collision and over time, then returns to normal behavior.
Pounce remains its explicit state after launch until the encounter ends.
Movement and timers now use a fixed 60 Hz simulation quantum:

- Every fixed step advances a hamster by at most `velocity / 60` seconds of
  movement, regardless of the selected display interval.
- Real-time mode consumes elapsed monotonic wall time and advances 60 fixed
  simulation steps per second. A 0.25-second stall cap prevents window moves
  and resizing from creating an unrealistic catch-up burst.
- Macroquad's rendered FPS is shown in the HUD as `FPS:`. A rendered FPS of
  60 or 144 is a display rate, not a second simulation clock.
- Slowed modes present FIFO task cycles at the selected interval, while each
  visible cycle advances only one fixed 1/60-second physics quantum.
- Pounce setup and launch use the fixed simulation timer, not the movement-task
  rate. The HUD converts behavior timers to approximate remaining scheduler
  ticks; pounce shows setup time and knockback may finish early as it slows.

When a knocked-back hamster strikes another hamster, that struck hamster
overrides Pounce, Chase, Flee, or Normal behavior and enters Knockback
immediately. A new encounter is not assigned while any hamster is in Knockback,
preventing knockback impact from being replaced by a scheduled Pounce or Flee
state.

## Timing model

The project distinguishes between a simulation tick and a scheduler quantum:

- A **simulation tick** is one call to `FixedTickEngine::run_tick`. It selects
  ready work and advances scheduler metrics.
- The **tick budget** limits the total work in one tick. The main application
  defaults to four work units. The **Round Robin quantum** limits work per
  selection, and a task can be selected again during the same tick. Normal
  movement and collision checks cost one unit, while chase, flee, pounce, and
  knockback actions cost two or three units.
- A task's `simulated_cost` describes how many work units it needs. If the remaining budget is too small, the task must continue during a later tick or be re-queued according to the active policy.
- Priority scheduling uses these numeric precedence values, where lower values
  run first: collision check `1`, knockback `2`, pounce `3`, chase `4`, flee
  `5`, and normal movement `10`. FIFO and Round Robin ignore priorities;
  Round Robin rotates unfinished work according to its quantum.

## Scheduling behavior and starvation

The live application replenishes tasks in complete action cycles. It creates
one cycle containing movement or encounter work and collision checks for all
active hamsters. It does not create a fresh cycle until all tasks from the
existing cycle leave the scheduler queue.

This is batch-based replenishment: an existing cycle drains before newly
collected behavior can enter the queue. It does not mean
that a single task always runs to completion within one scheduler tick.
`remaining_cost` allows a task to receive only part of its work during a tick,
then preserve the unfinished work for a later tick.

A task may be suspended because of:

- insufficient budget remaining for the task's full cost, deferring it to a
  later tick;
- a Round Robin quantum, which can rotate it behind other ready tasks even
  within the same tick.

The process is therefore best described as **batch-based task-cycle
collection with simulated time-sliced execution**. Priority selection
and Round Robin rotation occur within a cycle, but neither policy adds a new
cycle until the current cycle is empty.

This cycle-draining design reduces starvation caused by continuous queue
replenishment because no new tasks are admitted while the current finite cycle
is queued. However, it does not guarantee fairness in a continuous Priority
workload: a higher-priority task stream could delay lower-priority work
indefinitely unless priorities age or lower-priority tasks receive a policy
guarantee. Round Robin provides stronger fairness through queue rotation.
The live simulation limits starvation risk with its finite-cycle queue rather
than with an independent aging or starvation-prevention mechanism.

Adding more hamsters adds movement and collision-check tasks to the scheduler
queue. As task count grows, the fixed four-unit scheduler budget completes a
smaller portion of the overall activity per frame, so the simulation is
expected to appear slower even though each completed movement step advances by
the same 1/60-second physics quantum. The HUD displays the active policy,
budget, and scheduler log so the work distribution remains observable.

The interactive demonstration supports:

- **FIFO:** select in queue insertion order, but return unfinished tasks to
  the back when the tick budget runs out. Unlike strict non-preemptive FIFO,
  this can allow other tasks to run before a task finishes.
- **Priority:** select the ready task with the highest scheduling precedence.
  Lower numeric values represent higher precedence; unfinished tasks return
  to the back of their own priority queue.
- **Round Robin:** select in queue order and rotate unfinished tasks to the
  back. Its per-selection quantum is initially 2 work units; a task can
  receive multiple turns during one tick if budget remains.
- **MLFQ:** future work after the current FIFO, Priority, and Round Robin policies.

## Metrics

The scheduler model includes `TaskMetrics` for per-task measurements:

- **Waiting time:** completion tick minus release tick minus the simulation ticks in which the task received service.
- **Turnaround time:** completion tick minus release tick.
- **Response time:** first-run tick minus release tick.
- **Deadline miss:** recorded when a task is not completed by its deadline.

Service time counts distinct scheduler ticks in which a task received work,
even when Round Robin selects it multiple times in one tick. Waiting time is
therefore measured in whole ticks without service.

`SchedulerMetrics` stores experiment-wide elapsed ticks, busy ticks, completed
tasks, work units used, and available budget units. It derives:

- **Throughput:** completed tasks per elapsed tick.
- **Budget utilization:** work units used divided by available budget units
  across all elapsed ticks. This measures simulated capacity, not real CPU usage.
- **Busy-tick ratio:** busy ticks divided by elapsed ticks, available through
  `cpu_utilization()` for existing callers; a tick is busy if it does any work.

`FixedTickEngine<S>` populates these values while consuming task
`remaining_cost`, re-queuing incomplete tasks, and recording per-tick reports.
The command-line demo prints these metrics, and the main Macroquad application
displays completed-task counts and the current scheduler tick.