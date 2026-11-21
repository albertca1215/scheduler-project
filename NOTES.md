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

The application supports FIFO, Priority, and Round Robin scheduling. MLFQ is
not implemented yet.

The main application now also includes waypoint-based steering. Hamsters choose
new random waypoints during normal movement and smoothly turn toward them rather
than following one fixed straight line.

## Try it yourself

The scheduler has a human-readable command-line demo. A variety of prototypes were worked on in the process of getting accustomed to Macroquad. From the project root, run:

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
- `A` or the **Add** button adds the next available hamster.
- `R` or the **Remove** button removes the last added hamster.
  **Meep** remains the default and is never removed.
- `F1` switches to FIFO.
- `F2` switches to Priority.
- `F3` switches to Round Robin.

Switching scheduling types resets the simulation to one **Meep** hamster,
zeroes scheduler and simulation counters, restores the default step interval,
resumes execution, and clears the active-task label.

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
restarted by repeated collisions. Hamster-to-hamster collision overlap is
separated once and guarded by a 0.75-second cooldown so that repeated contacts
do not produce infinite velocity swaps or attach the hamsters together.

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
  rate.

When a knocked-back hamster strikes another hamster, that struck hamster
overrides Pounce, Chase, Flee, or Normal behavior and enters Knockback
immediately. A new encounter is not assigned while any hamster is in Knockback,
preventing knockback impact from being replaced by a scheduled Pounce or Flee
state.

## Timing model

The project distinguishes between a simulation tick and a scheduler quantum:

- A **simulation tick** is one call to `FixedTickEngine::run_tick`. It selects
  ready work and advances scheduler metrics.
- A **scheduler quantum** is the amount of simulated work the scheduler may
  execute during one tick. The main application uses a budget of one work unit,
  making each FIFO task visible.
- A task's `simulated_cost` describes how many work units it needs. If the remaining budget is too small, the task must continue during a later tick or be re-queued according to the active policy.

Adding more hamsters adds movement and collision-check tasks to the FIFO queue.
As task count grows, the fixed scheduler work budget completes a smaller
portion of the overall activity per frame, so the simulation is expected to
appear slower even though each movement step advances by the same 1/60-second
physics quantum. The HUD's run-time state indicator is not used; FIFO tick and
active-task information are sufficient to show scheduling progress.

The interactive demonstration supports:

- **Priority:** select the ready task with the highest scheduling precedence.
  Lower numeric priority values represent higher precedence.
- **Round Robin:** re-queue unfinished work and rotate ready tasks so that
  waiting tasks receive a fair turn. The initial policy quantum is 10 work units.
- **MLFQ:** future work after the current FIFO, Priority, and Round Robin policies.

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