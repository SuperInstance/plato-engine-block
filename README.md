# Plato Engine Block

> **Plato Engine Block — a sub-400-line room runtime for agent-space interaction.**  
> *The atomic room runtime for the Plato Matrix — a universal agent-space interface.*

## What Is This?

Plato Engine Block is a Rust library that implements the concept of a **Room** — a self-contained unit of sensor/actuator interaction that:

- **Ticks** at a configurable rate (Hz), reading all sensors each tick
- **Records** a rolling history of tick snapshots in a circular buffer
- **Evaluates** alarm rules against sensor data with cooldown semantics
- **Accepts** a human-friendly text protocol for queries and control
- **Streams** live updates to subscribed TCP clients

Think of it as the smallest building block in a larger sensor/actuator mesh — a single "room" in the Plato Matrix that can be composed, replicated, and networked into arbitrarily complex systems.

## Architecture

```
┌──────────────────────────────────────────────┐
│                 PlatoEngine                   │
│                                              │
│  ┌─────────┐  ┌──────────┐  ┌────────────┐  │
│  │ Sensors │  │Actuators │  │   Alarms   │  │
│  │ (read)  │  │ (write)  │  │ (evaluate) │  │
│  └────┬────┘  └────┬─────┘  └─────┬──────┘  │
│       │            │              │          │
│       ▼            │              │          │
│  ┌─────────┐       │              │          │
│  │  Tick   │───────┼──────────────┘          │
│  │(snapshot)│      │                          │
│  └────┬────┘       │                          │
│       │            │                          │
│       ▼            ▼                          │
│  ┌─────────────────────────┐                  │
│  │    History Buffer       │                  │
│  │    (circular, N ticks)  │                  │
│  └─────────────────────────┘                  │
│                                              │
│  ┌─────────────────────────┐                  │
│  │    Protocol Handler     │                  │
│  │  (text command parser)  │                  │
│  └─────────────────────────┘                  │
│                                              │
│  ┌─────────────────────────┐  (server feat)  │
│  │    TCP Server           │                  │
│  │  (tokio, multi-client)  │                  │
│  └─────────────────────────┘                  │
└──────────────────────────────────────────────┘
```

## Feature Flags

| Feature   | Default | Description                                    |
|-----------|---------|------------------------------------------------|
| `std`     | ✅      | Standard library support (File, println, etc.) |
| `server`  | ❌      | Tokio-based TCP multi-client server            |

The core engine is designed to be `no_std` + `alloc` compatible. The `std` flag enables convenience features, and `server` pulls in tokio for TCP networking.

```toml
[dependencies]
plato-engine-block = { version = "0.1", default-features = false }  # no_std
plato-engine-block = "0.1"                                            # std
plato-engine-block = { version = "0.1", features = ["server"] }      # full
```

## Quick Start

### Building an Engine

```rust
use plato_engine_block::{PlatoEngine, PlatoEngineBuilder};

let mut engine = PlatoEngine::builder()
    .sensor("temperature", Box::new(|| 22.5))
    .sensor("humidity", Box::new(|| 45.0))
    .actuator("heater", Box::new(|v| { println!("heater={}", v); true }))
    .actuator("fan", Box::new(|v| { println!("fan={}", v); true }))
    .alarm(
        "overheat",
        Box::new(|data| {
            data.iter().any(|(name, val)| name == "temperature" && *val > 30.0)
        }),
        5, // cooldown: 5 ticks between fires
    )
    .tick_hz(10.0)          // 10 ticks per second
    .history_capacity(1000)  // remember last 1000 ticks
    .build();
```

### Ticking

```rust
// Take one tick — reads all sensors, evaluates alarms, pushes to history
let tick = engine.tick();
println!("Tick {}: temp={:?}", tick.index, tick.get("temperature"));
```

### Text Protocol

```rust
// Query and control via text commands
let response = engine.handle_command("tick");
// → "tick 0 @ 0.000s\n  temperature = 22.5000\n  humidity = 45.0000"

let response = engine.handle_command("history 5");
// → Shows last 5 ticks

let response = engine.handle_command("heater 75.0");
// → "heater ← 75.0000"

let response = engine.handle_command("subscribe");
// → "subscribed"

let response = engine.handle_command("help");
// → Shows all available commands
```

### TCP Server (with `server` feature)

```rust
use plato_engine_block::server::run_server;

#[tokio::main]
async fn main() {
    let engine = PlatoEngine::builder()
        .sensor("temp", Box::new(|| read_temperature()))
        .tick_hz(1.0)
        .history_capacity(100)
        .build();

    let (handle, _join) = run_server(engine, "0.0.0.0:7070")
        .await
        .unwrap();

    // Broadcast custom messages to all subscribers
    handle.broadcast("System online".to_string());
}
```

## Text Protocol Reference

| Command              | Description                                |
|----------------------|--------------------------------------------|
| `tick`               | Take one tick, read all sensors            |
| `history [N]`        | Show last N ticks (default 10)             |
| `<actuator> <value>` | Set named actuator to floating-point value  |
| `alarm list`         | List all alarm rules and their states      |
| `subscribe`          | Subscribe to live tick/alarm broadcasts    |
| `unsubscribe`        | Unsubscribe from broadcasts                |
| `help`               | Show command reference                     |

## Alarm System

Alarms evaluate a user-defined condition against each tick's sensor data. They follow a state machine:

```
Idle → Active (condition met) → Cooldown (condition cleared) → Idle
         ↑                                                       |
         └─────────── condition re-met after cooldown ───────────┘
```

- **Cooldown** prevents alarm spam: after an alarm fires, it won't re-fire for N ticks.
- **Active** state persists as long as the condition remains true.
- **Cooldown** begins when the condition transitions from true to false.

```rust
.alarm(
    "low_battery",
    Box::new(|data| {
        data.iter().any(|(n, v)| n == "battery" && *v < 10.0)
    }),
    100, // cooldown: 100 ticks (~10 seconds at 10Hz)
)
```

## History Buffer

The circular buffer provides efficient O(1) push and O(N) query:

- **Latest**: constant-time access to the most recent tick
- **Query(N)**: returns the last N ticks in chronological order
- **Overflow**: oldest ticks are automatically evicted when capacity is reached

```rust
engine.tick();
engine.tick();
engine.tick();

let latest = engine.latest().unwrap();
let recent = engine.history(3);
assert_eq!(recent.len(), 3);
```

## no_std Support

The core engine works without the standard library, requiring only `alloc`:

```rust
// In your Cargo.toml:
// plato-engine-block = { version = "0.1", default-features = false }

// In your no_std crate:
#![no_std]

use plato_engine_block::{PlatoEngine, PlatoEngineBuilder};

// Builder, ticking, history, alarms, and protocol all work in no_std
```

## Design Philosophy

**Simplicity first.** This is an *engine block* — the fundamental, boring, reliable core that everything else bolts onto. No async runtime required for the core. No framework opinions. Just sensors, actuators, alarms, history, and a text protocol.

**Composable.** A single engine block represents one room/device/agent. Stack them, network them, orchestrate them with whatever higher-level system you want.

**Observable.** Every tick is recorded. Every alarm is tracked. The text protocol means you can `nc localhost 7070` and interact with a running room in real time.

**Embedded-friendly.** `no_std` + `alloc` means this runs on microcontrollers, WASM, or anywhere Rust's allocator works.

## Module Layout

```
src/
├── lib.rs        — Re-exports, crate docs, test suite
├── engine.rs     — PlatoEngine struct, PlatoEngineBuilder
├── tick.rs       — Tick struct (timestamp + sensor data)
├── sensor.rs     — Sensor trait, SensorSpec, SensorFn
├── actuator.rs   — Actuator trait, ActuatorSpec, ActuatorFn
├── alarm.rs      — AlarmRule, AlarmState, condition evaluation
├── history.rs    — Circular buffer of Ticks, query API
├── protocol.rs   — Text command parser, response formatter
└── server.rs     — Tokio TCP server (behind "server" feature)
```

## PLATO Engine Block Family

This is the original Rust implementation of the Plato Engine Block. The complete family:

| Implementation | Language | Repo | Focus |
|---|---|---|---|
| **Rust (Original)** ← you are here | Rust | [plato-engine-block](https://github.com/SuperInstance/plato-engine-block) | `no_std` + alloc, builder pattern, tokio server |
| **C Reference** | C99 | [plato-engine-block-c](https://github.com/SuperInstance/plato-engine-block-c) | Embedded, bare-metal, zero heap alloc |
| **Elixir/OTP** | Elixir | [plato-engine-block-elixir](https://github.com/SuperInstance/plato-engine-block-elixir) | BEAM supervision trees, fault tolerance, hot reload |
| **Zig** | Zig | [plato-engine-block-zig](https://github.com/SuperInstance/plato-engine-block-zig) | Comptime ternary packing, cross-compile, zero hidden control flow |
| **Python Core** | Python | [plato-core](https://github.com/SuperInstance/plato-core) | Foundation types, mesh registry, training tiles |
| **Runtime Kernel** | Rust | [plato-runtime-kernel](https://github.com/SuperInstance/plato-runtime-kernel) | Spatial model: tensor grid, batons, assertion traps |
| **Server** | Python | [plato-server](https://github.com/SuperInstance/plato-server) | Knowledge tiles, fleet sync via Matrix, HTTP API |

**Specs & Guides:**
- 📜 [PLATO Wire Protocol](https://github.com/SuperInstance/AI-Writings/blob/main/PLATO_WIRE_PROTOCOL.md)
- 📖 [PLATO Master Guide](https://github.com/SuperInstance/AI-Writings/blob/main/PLATO_MASTER_GUIDE.md)
- 🗺️ [PLATO Ecosystem Map](https://github.com/SuperInstance/AI-Writings/blob/main/PLATO_ECOSYSTEM_MAP.md)
- 🏗️ [PLATO Rust Architecture](https://github.com/SuperInstance/AI-Writings/blob/main/PLATO_RUST_ARCHITECTURE.md)


## License

MIT

## Ecosystem

This repo is part of the **SuperInstance** flagship ecosystem — agent-first computation, constraint theory, and self-improving runtimes.

### FLUX Runtime Family

| Repo | Language | Description |
|------|----------|-------------|
| [flux-runtime](https://github.com/SuperInstance/flux-runtime) | Python | Full FLUX runtime: markdown→bytecode, 2037 tests, zero deps |
| [flux-core](https://github.com/SuperInstance/flux-core) | Rust | Register-based bytecode VM, deterministic agent computation |
| [flux-js](https://github.com/SuperInstance/flux-js) | JavaScript | FLUX VM for Node.js and browsers, ~400ns/iter |
| [flux-compiler](https://github.com/SuperInstance/flux-compiler) | Rust/Python | Formal-methods compiler for safety-critical codegen |
| [flux-vm](https://github.com/SuperInstance/flux-vm) | Rust | Stack-based constraint-checking VM, 50 opcodes, Turing-incomplete |

### PLATO Engine Family

| Repo | Language | Description |
|------|----------|-------------|
| [plato-server](https://github.com/SuperInstance/plato-server) | Python | Knowledge tiles, fleet sync via Matrix, HTTP API |
| [plato-engine-block](https://github.com/SuperInstance/plato-engine-block) | Rust | Original room runtime: no_std + alloc, builder pattern |
| [plato-engine-block-c](https://github.com/SuperInstance/plato-engine-block-c) | C99 | Embedded reference: zero heap alloc, bare-metal portable |
| [plato-engine-block-elixir](https://github.com/SuperInstance/plato-engine-block-elixir) | Elixir | BEAM supervision trees, fault tolerance, hot reload |
| [plato-runtime-kernel](https://github.com/SuperInstance/plato-runtime-kernel) | Rust | Spatial model: tensor grid, batons, assertion traps |

### Constraint / Theory Family

| Repo | Language | Description |
|------|----------|-------------|
| [categorical-agents](https://github.com/SuperInstance/categorical-agents) | Rust | Category theory for agent composition (functors, naturality) |
| [cuda-constraint-engine](https://github.com/SuperInstance/cuda-constraint-engine) | CUDA/C | GPU constraint checking at 1B+ constraints/sec |
| [grand-pattern-rs](https://github.com/SuperInstance/grand-pattern-rs) | Rust | Fibonacci dual-direction cellular graph architecture |
| [lau-hodge-theory](https://github.com/SuperInstance/lau-hodge-theory) | Rust | Hodge decomposition, Betti numbers, spectral sequences |
| [ternary-science](https://github.com/SuperInstance/ternary-science) | Rust | Experimental evidence for ternary intelligence, 5 conservation laws |

### Agent / Infrastructure Family

| Repo | Language | Description |
|------|----------|-------------|
| [construct-core](https://github.com/SuperInstance/construct-core) | Rust | Layered trait system: bare-metal → alloc → async agent runtime |
| [crab](https://github.com/SuperInstance/crab) | Bash | Agent shell for repo entry/leave (MUD-room metaphor) |
| [exocortex](https://github.com/SuperInstance/exocortex) | Rust | Persistent cognitive substrate, S3-compatible memory |
| [git-agent](https://github.com/SuperInstance/git-agent) | Python | The repo IS the agent — autonomous lifecycle via Git |
| [capitaine-1](https://github.com/SuperInstance/capitaine-1) | TypeScript | Git-native repo-agent, Cloudflare Workers heartbeat |
| [codespace-edge-rd](https://github.com/SuperInstance/codespace-edge-rd) | Research | Codespace→Edge agent lifecycle and yoke transfer protocols |
| [git-agent-codespace](https://github.com/SuperInstance/git-agent-codespace) | DevContainer | One-click Codespace template for Git-Agent runtimes |

### Registries

| Registry | Package | Install |
|----------|---------|---------|
| **PyPI** | `flux-vm` | `pip install flux-vm` |
| **crates.io** | `fluxvm` | `cargo add fluxvm` |
| **npm** | `flux-js` | `npm install flux-js` *(coming soon)* |

### Philosophy & Architecture

- 📖 [AI-Writings](https://github.com/SuperInstance/AI-Writings) — Philosophy, essays, and design rationale
- 📦 [PACKAGES.md](https://github.com/SuperInstance/SuperInstance/blob/main/PACKAGES.md) — Full package index
