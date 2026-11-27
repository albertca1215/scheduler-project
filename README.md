# Hamster Scheduler

A Rust project for exploring CPU scheduling concepts with a deterministic
terminal demonstration and an interactive Macroquad hamster simulation. Both
use the same scheduler engine, but their workloads differ. This is a
simulation, not an operating-system kernel.

## Requirements

- Rust and Cargo with Rust 2024 edition support (Rust 1.85 or newer).
- A graphical desktop for the Macroquad application.

Run the commands below from the repository root so the application can find
`examples/hamster.png`.

## Run the simulation

```powershell
cargo run
```

The simulation starts paused with Meep, FIFO scheduling, a budget of 4 work
units per tick, and a Round Robin quantum of 2. Press Space to start.

| Control | Action |
| --- | --- |
| Space | Pause or resume |
| A / R | Add or remove a hamster (up to Meep, Beep, and Peep; Meep remains) |
| F1 / F2 / F3 | Select FIFO, Priority, or Round Robin |
| Q / E | Decrease or increase tick budget (1-16) |
| Z / X | Decrease or increase RR quantum (1-8) |
| Up / Down | Increase or decrease simulation speed |

Changing the policy, budget, or quantum resets the simulation to Meep, clears
the log and counters, and resets the interval; the pause state is preserved.
Adding or removing a hamster does **not** reset the simulation. For a clean
start, configure the policy and budget while paused, then add hamsters before
starting.

The HUD shows the policy, budget, quantum, tick counter, controls, action
costs and priorities. The scheduler log shows selections (`started`), tasks
completed, selected tasks still incomplete at the end of the tick, and the
number left queued. A task may appear as `started` again when it resumes; an
incomplete task has retained its progress. When paused, the status shows the
most recently processed tick and lists tasks still queued afterward, including
partially completed tasks and those not selected yet. Queued names reflect the
tasks as created. Tick reports are zero-indexed, while the top
counter advances after processing, so the latest report is normally one less
than that counter.

Behavior countdowns display approximate remaining scheduler ticks, converted
from the existing simulated-time timers. Pounce shows its setup countdown;
knockback may end earlier if its velocity decays sufficiently.

## Run the deterministic scheduler demo

```powershell
cargo run --example scheduler_demo
```

With no arguments, this runs the same fixed eight-task workload through FIFO,
Priority, and Round Robin with a budget of 4 and RR quantum of 2. It prints
the workload, per-tick selections, completions, incomplete work and queue
count, followed by task metrics and aggregate throughput and utilization.
The workload is synthetic: it includes all encounter action types, not a
literal snapshot of one three-hamster simulation cycle.

Choose a single policy and settings:

```powershell
cargo run --example scheduler_demo -- fifo 4
cargo run --example scheduler_demo -- priority 4
cargo run --example scheduler_demo -- rr 4 1
```

The numeric arguments are positive integers: budget, then RR quantum. If the
RR quantum is omitted, it defaults to 2. Alternatively, use the text prompts:

```powershell
cargo run --example scheduler_demo -- interactive
```

Use the deterministic demo for repeatable comparisons; the graphical
simulation uses random movement and encounter choices, so separate runs need
not produce identical outcomes.

## Scheduling model

Each selected task receives up to the minimum of its remaining cost, the
remaining tick budget, and the scheduler's quantum. FIFO and Priority do not
impose an additional per-selection quantum. Unfinished work is requeued with
its remaining cost.

- **FIFO:** selects in queue insertion order. If a task exhausts the remaining
  tick budget before finishing, it returns to the back of the queue. This is
  not strict non-preemptive FIFO; there is no separate per-task quantum.
- **Priority:** selects the lowest numeric priority, with FIFO order among
  tasks of equal priority. An unfinished task returns to the back of its
  priority queue.
- **Round Robin:** selects in queue order and rotates unfinished tasks to the
  back after at most its configured per-selection quantum. It ignores priority
  and can select the same task again within a tick if budget remains.

The terminal demo submits one fixed batch; the live simulation waits for its
current batch to drain before creating the next. This finite workload prevents
new arrivals from indefinitely delaying that batch, but is not a general
starvation-prevention algorithm. A real ready queue can receive new work while
existing tasks are waiting. Retaining a task's remaining cost models progress
across selections, not a full OS context switch.

The live simulation uses these costs and Priority values:

| Action | Work units | Priority |
| --- | ---: | ---: |
| Collision check | 1 | 1 |
| Knockback | 3 | 2 |
| Pounce | 3 | 3 |
| Chase | 2 | 4 |
| Flee | 2 | 5 |
| Normal movement | 1 | 10 |

Each live cycle queues one behavior task and one wall-collision-check task per
hamster. A new cycle is queued only after the previous queue drains. Hamster
pair collisions are also checked separately after each scheduler tick, outside
the scheduled collision-check tasks. Deadlines are synthetic metadata for
miss reporting; none of these policies uses them for selection or cancellation.

## Tests and further reading

```powershell
cargo test
```

See [PRESENTATION.md](PRESENTATION.md) for a five-minute presentation outline
and [NOTES.md](NOTES.md) for project notes and implementation details.
