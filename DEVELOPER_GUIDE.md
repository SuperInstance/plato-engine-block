# Developer Guide — plato-engine-block

> Architecture deep-dive, module walkthrough, extension points, and contributing guide for the atomic room runtime.

---

## Architecture Overview

`plato-engine-block` is the fundamental building block of the Plato Matrix — a single "room" that reads sensors, drives actuators, evaluates alarms, records history, and speaks a text protocol. It is deliberately synchronous and dependency-minimal: the core requires only `alloc`, making it suitable for embedded targets, WASM, and bare-metal environments.

The optional `server` feature adds a tokio-based TCP server for networked deployments.

### Data Flow

```
Sensor closures ──▶ tick() ──▶ Tick snapshot
                                   │
                      ┌────────────┼────────────┐
                      ▼            ▼            ▼
               HistoryBuffer   AlarmRules   ProtocolHandler
               (circular)      (eval+cool)  (text commands)
                                               │
                                        TCP Server (optional)
                                        (subscribe/broadcast)
```

Every call to `tick()` does exactly three things:
1. Reads all sensor closures, producing a `Tick` with an index and timestamp.
2. Pushes the tick into the `HistoryBuffer` (circular, evicts oldest on overflow).
3. Evaluates each `AlarmRule` against the tick's sensor data, tracking state transitions (Idle → Active → Cooldown → Idle) and collecting fires.

The `ProtocolHandler` is embedded in `PlatoEngine` — `handle_command()` parses a text line and dispatches to the appropriate subsystem.

---

## Module-by-Module Walkthrough

### `engine.rs` — PlatoEngine & PlatoEngineBuilder

The central struct and its builder. `PlatoEngine` holds:

| Field | Type | Purpose |
|-------|------|---------|
| `sensors` | `Vec<SensorSpec>` | Named sensor closures |
| `actuators` | `Vec<ActuatorSpec>` | Named actuator closures |
| `alarms` | `Vec<AlarmRule>` | Alarm rules with state machines |
| `history` | `HistoryBuffer` | Circular buffer of `Tick` structs |
| `tick_hz` | `f64` | Configured tick rate |
| `tick_counter` | `u64` | Monotonically increasing tick index |
| `alarm_fires` | `Vec<String>` | Alarms that fired on the last tick |
| `streaming` | `bool` | Whether the engine is in subscribe mode |

**Builder pattern:** `PlatoEngine::builder()` returns a `PlatoEngineBuilder` with sensible defaults (1 Hz, 100-tick history). Chain `.sensor()`, `.actuator()`, `.alarm()`, `.tick_hz()`, `.history_capacity()`, then `.build()`.

**Extension point:** To add new engine-level behavior, add fields to `PlatoEngine` and methods on the builder. The builder consumes itself on `.build()`, so new fields must have defaults.

### `tick.rs` — Tick

A single tick snapshot:

```rust
pub struct Tick {
    pub index: u64,
    pub timestamp: f64,  // seconds since engine creation
    pub data: Vec<(String, f64)>,  // (sensor_name, value) pairs
}
```

Key methods: `get(name) → Option<f64>`, `sensor_names() → Vec<&str>`.

**Extension point:** Add derived fields (e.g., computed sensors, delta from previous tick) by extending the `Tick` struct and populating them in `engine.tick()`.

### `sensor.rs` — Sensor Trait & SensorSpec

The `Sensor` trait has a single method: `read() → f64`. `SensorFn` is a blanket impl for `Fn() → f64`. `SensorSpec` pairs a name with a boxed sensor.

Sensors are polled synchronously on each tick. For async I/O, wrap the sensor in an `Arc<Mutex<>>` that the closure reads from, and update it from a separate async task.

### `actuator.rs` — Actuator Trait & ActuatorSpec

The `Actuator` trait: `set(value: f64) → bool` (returns success/failure). `ActuatorFn` blankets `Fn(f64) → bool`. `ActuatorSpec` pairs a name with a boxed actuator.

Actuators are set via `engine.set_actuator(name, value)` or through the text protocol (`"heater 75.0"`). The return value indicates whether the actuator accepted the command.

### `alarm.rs` — AlarmRule, AlarmState, AlarmCondition

The alarm state machine:

```
         condition met                condition cleared
Idle ───────────────▶ Active ───────────────────────▶ Cooldown
 ▲                                                   │
 └────────────── condition re-met after cooldown ────┘
```

`AlarmCondition` is `Fn(&[(String, f64)]) → bool`. `AlarmRule` tracks:
- `name: String`
- `condition: Box<dyn AlarmCondition>`
- `cooldown: u64` — ticks to wait after the condition clears before re-arming
- `state: AlarmState` — `Idle`, `Active { since_tick }`, or `Cooldown { remaining }`

A fire occurs on the **Idle → Active** transition. While `Active`, the alarm does not re-fire. When the condition clears, the alarm enters `Cooldown` and counts down. Only after cooldown reaches zero does it return to `Idle`.

**Extension point:** New alarm states (e.g., `Escalated`, `Muted`) can be added to the `AlarmState` enum. Modify `evaluate()` in `alarm.rs` to handle the new transitions.

### `history.rs` — HistoryBuffer

A `VecDeque<Tick>` wrapped with capacity management:

- `push(tick)` — appends, evicts oldest if at capacity
- `latest() → Option<&Tick>` — O(1) access to the most recent tick
- `query(n) → Vec<&Tick>` — last N ticks in chronological order
- `len()`, `capacity()`, `is_empty()`

The circular buffer is the core observability mechanism. All tick data flows through it.

### `protocol.rs` — ProtocolHandler (embedded in PlatoEngine)

Parses text commands and returns string responses. The command set:

| Command | Handler |
|---------|---------|
| `tick` | Take one tick, format sensor readings |
| `history [N]` | Query history buffer |
| `<name> <value>` | Set actuator |
| `alarm list` | Show alarm states |
| `subscribe` / `unsubscribe` | Toggle streaming mode |
| `help` | Command reference |

**Extension point:** Add commands by extending the `match` in `handle_command()`. Each command maps to a string response.

### `server.rs` — TCP Server (feature-gated)

Enabled by `features = ["server"]`. Uses tokio to accept multiple TCP connections. Each client gets a line-oriented protocol session. Subscribed clients receive broadcast messages (tick results, alarm fires).

Key API: `run_server(engine, addr) → (BroadcastHandle, JoinHandle)`. The `BroadcastHandle` allows external code to send messages to all subscribers.

---

## Feature Flags

```toml
# Core only (no_std + alloc compatible)
plato-engine-block = { version = "0.1", default-features = false }

# Standard (default)
plato-engine-block = "0.1"

# With TCP server
plato-engine-block = { version = "0.1", features = ["server"] }
```

The `std` feature enables `std::time::Instant` for precise timestamps and `std::fs` for potential file-backed history. Without `std`, timestamps are derived from tick index × tick period.

---

## Testing Strategy

The crate includes 20+ unit tests covering:

- **Builder & defaults:** Verifying field values after construction
- **Sensor reading:** Tick produces correct sensor data
- **History overflow:** Circular buffer eviction at capacity
- **Actuator execution:** Success/failure returns, unknown actuator errors
- **Alarm triggering:** Idle→Active transitions, fire recording
- **Alarm cooldown:** No re-fire while Active, countdown after clear
- **Protocol parsing:** Each command produces correct output
- **Integration:** Full lifecycle (tick→history→alarm→protocol→actuator)

Tests use `Box::new(|| value)` for deterministic sensor readings and `Arc<AtomicU64>` for stateful sensors. No external dependencies or I/O required.

### Running Tests

```bash
cargo test                  # Core tests
cargo test --features server  # Include server tests
```

---

## Contributing Guide

### Adding a New Sensor Type

1. Implement `Fn() → f64` (or the `Sensor` trait).
2. Register it via `.sensor("name", Box::new(my_sensor))` on the builder.

### Adding a New Protocol Command

1. Add the command name and handler logic in `protocol.rs` (or the `handle_command` method).
2. Update the `help` output.
3. Add a test in `tests` module of `lib.rs`.

### Adding a New Alarm State

1. Extend `AlarmState` enum in `alarm.rs`.
2. Update `AlarmRule::evaluate()` to handle the new state.
3. Update display/formatting code.
4. Add tests for the new transition.

### Code Style

- Follow `rustfmt` defaults.
- Public API surfaces should have doc comments with examples.
- Prefer composition over inheritance (Rust makes this natural).
- Keep `no_std` compatibility in mind — avoid `std::` types in core modules.

---

## Project Structure

```
plato-engine-block/
├── Cargo.toml
├── README.md
├── DEVELOPER_GUIDE.md
├── src/
│   ├── lib.rs        # Re-exports, feature gates, test suite
│   ├── engine.rs     # PlatoEngine, PlatoEngineBuilder
│   ├── tick.rs       # Tick snapshot struct
│   ├── sensor.rs     # Sensor trait, SensorSpec, SensorFn
│   ├── actuator.rs   # Actuator trait, ActuatorSpec, ActuatorFn
│   ├── alarm.rs      # AlarmRule, AlarmState, AlarmCondition
│   ├── history.rs    # HistoryBuffer (circular)
│   ├── protocol.rs   # Text command parser + formatter
│   └── server.rs     # Tokio TCP server (feature-gated)
└── tests/
    └── (integration tests if separated)
```

---

## Design Decisions

| Decision | Rationale |
|----------|-----------|
| Builder pattern for construction | Complex configuration with sensible defaults; consumes self to prevent mutation after build |
| `Vec<(String, f64)>` for tick data | Simple, ownership-friendly, no HashMap overhead for small N |
| Closures for sensors/actuators | Zero-cost abstraction; users bring their own I/O |
| Circular buffer for history | Bounded memory; O(1) push; oldest data least relevant |
| Text protocol | Human-debuggable (`nc localhost 7070`); no serialization dependency |
| `no_std` core | Embedded targets, WASM, bare-metal deployments |
| Alarm cooldown as tick count | Deterministic across platforms; no clock dependency |
