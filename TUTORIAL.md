# Tutorial — plato-engine-block

> **By the end of this tutorial, you will have built a complete monitoring room** for a fishing vessel's engine compartment — with real sensor simulation, overheat alarms, actuator control, and a text protocol you can interact with over TCP.

---

## Prerequisites

- Rust 1.70+ (`rustup update`)
- `netcat` (for TCP testing, optional)
- 20 minutes

## Step 1: Create the Project

```bash
cargo new engine-room-monitor
cd engine-room-monitor
```

Add the dependency to `Cargo.toml`:

```toml
[dependencies]
plato-engine-block = { version = "0.1", features = ["server"] }
tokio = { version = "1", features = ["full"] }
```

We enable the `server` feature so we can connect over TCP later.

## Step 2: Build a Basic Engine

Replace `src/main.rs` with:

```rust
use plato_engine_block::{PlatoEngine, PlatoEngineBuilder};

fn main() {
    let mut engine = PlatoEngine::builder()
        .sensor("coolant_temp_c", Box::new(|| 85.0))
        .sensor("oil_pressure_psi", Box::new(|| 55.0))
        .sensor("engine_rpm", Box::new(|| 1800.0))
        .tick_hz(1.0)          // 1 tick per second
        .history_capacity(60)  // 1 minute of history
        .build();

    // Take 3 ticks
    for _ in 0..3 {
        let tick = engine.tick();
        println!(
            "Tick {}: coolant={:.1}°C, oil={:.1} psi, rpm={:.0}",
            tick.index,
            tick.get("coolant_temp_c").unwrap(),
            tick.get("oil_pressure_psi").unwrap(),
            tick.get("engine_rpm").unwrap(),
        );
    }
}
```

Run it:

```bash
cargo run
```

You should see:

```
Tick 0: coolant=85.0°C, oil=55.0 psi, rpm=1800
Tick 1: coolant=85.0°C, oil=55.0 psi, rpm=1800
Tick 2: coolant=85.0°C, oil=55.0 psi, rpm=1800
```

**What happened:** The builder created an engine with 3 sensors (all returning fixed values), running at 1 Hz, with a 60-tick circular buffer. Each `tick()` call reads all sensors and records a snapshot.

## Step 3: Add Realistic Sensor Simulation

Static values are boring. Let's simulate a coolant temperature that slowly rises:

```rust
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use plato_engine_block::{PlatoEngine, PlatoEngineBuilder};

fn main() {
    let tick_count = Arc::new(AtomicU64::new(0));
    let tick_clone = tick_count.clone();

    let mut engine = PlatoEngine::builder()
        .sensor("coolant_temp_c", Box::new(move || {
            let t = tick_clone.fetch_add(1, Ordering::SeqCst);
            80.0 + (t as f64 * 0.5) // Rises 0.5°C per tick
        }))
        .sensor("oil_pressure_psi", Box::new(|| 55.0))
        .sensor("engine_rpm", Box::new(|| 1800.0))
        .tick_hz(1.0)
        .history_capacity(60)
        .build();

    for _ in 0..10 {
        let tick = engine.tick();
        let temp = tick.get("coolant_temp_c").unwrap();
        println!("Tick {}: coolant = {:.1}°C", tick.index, temp);
    }
}
```

Output:

```
Tick 0: coolant = 80.0°C
Tick 1: coolant = 80.5°C
Tick 2: coolant = 81.0°C
...
Tick 9: coolant = 84.5°C
```

**What happened:** We wrapped an `AtomicU64` in `Arc` to create a stateful sensor. Each call increments the counter and computes a rising temperature. This pattern — `Arc<Mutex<>>` or `Arc<Atomic*>` — is how you bridge async I/O or hardware reads into the engine's synchronous sensor model.

## Step 4: Add an Overheat Alarm

Now let's trigger an alarm when coolant exceeds 90°C:

```rust
.alarm(
    "coolant_overtemp",
    Box::new(|data| {
        data.iter().any(|(name, val)| name == "coolant_temp_c" && *val > 90.0)
    }),
    5,  // Cooldown: 5 ticks between fires
)
```

Add this to the builder chain. Then modify the loop to check for alarm fires:

```rust
for _ in 0..25 {
    let tick = engine.tick();
    let temp = tick.get("coolant_temp_c").unwrap();

    if !engine.alarm_fires.is_empty() {
        println!("🚨 ALARM: {:?} (coolant = {:.1}°C)", engine.alarm_fires, temp);
    } else {
        println!("Tick {}: coolant = {:.1}°C ✓", tick.index, temp);
    }
}
```

You'll see the alarm fire when the temperature crosses 90°C, then enter cooldown:

```
Tick 0: coolant = 80.0°C ✓
...
Tick 20: coolant = 90.0°C ✓
🚨 ALARM: ["coolant_overtemp"] (coolant = 90.5°C)
...
```

**What happened:** The alarm condition checks if `coolant_temp_c > 90.0`. On first detection, it transitions `Idle → Active` and records a fire. While `Active`, it won't re-fire. If the condition clears, it enters `Cooldown` for 5 ticks before re-arming.

## Step 5: Add Actuator Control

Add a cooling fan actuator:

```rust
.actuator("cooling_fan", Box::new(|v| {
    println!("   → Cooling fan set to {:.0}%", v);
    v >= 0.0 && v <= 100.0  // Validate range
}))
```

Control it via the text protocol:

```rust
// In the loop, after alarm detection:
if !engine.alarm_fires.is_empty() && tick.get("coolant_temp_c").unwrap() > 90.0 {
    engine.handle_command("cooling_fan 100.0");
}
```

## Step 6: Use the Text Protocol

The text protocol lets you interact with the engine through simple commands:

```rust
// Outside the loop, try some commands:
println!("\n--- Protocol Demo ---");
println!("{}", engine.handle_command("history 3"));
println!("{}", engine.handle_command("alarm list"));
println!("{}", engine.handle_command("cooling_fan 50.0"));
println!("{}", engine.handle_command("help"));
```

## Step 7: Expose via TCP Server

Replace `src/main.rs` with the full server version:

```rust
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use plato_engine_block::{PlatoEngine, PlatoEngineBuilder};
use plato_engine_block::server::run_server;

#[tokio::main]
async fn main() {
    let tick_count = Arc::new(AtomicU64::new(0));
    let tc = tick_count.clone();

    let engine = PlatoEngine::builder()
        .sensor("coolant_temp_c", Box::new(move || {
            let t = tc.fetch_add(1, Ordering::SeqCst);
            80.0 + (t as f64 * 0.3)
        }))
        .sensor("oil_pressure_psi", Box::new(|| 55.0))
        .actuator("cooling_fan", Box::new(|v| {
            println!("Fan set to {:.0}%", v);
            v >= 0.0 && v <= 100.0
        }))
        .alarm(
            "overheat",
            Box::new(|d| d.iter().any(|(n, v)| n == "coolant_temp_c" && *v > 90.0)),
            5,
        )
        .tick_hz(1.0)
        .history_capacity(100)
        .build();

    let (handle, join) = run_server(engine, "0.0.0.0:7070").await.unwrap();

    handle.broadcast("Engine room monitor online".to_string());
    println!("Server running on :7070. Connect with: nc localhost 7070");

    join.await.unwrap();
}
```

Run it, then in another terminal:

```bash
$ nc localhost 7070
tick
history 5
cooling_fan 75.0
alarm list
subscribe
help
```

**Congratulations!** You've built a complete engine room monitor with simulated sensors, alarm detection, actuator control, and a TCP interface. This is the same pattern used in production Plato deployments on fishing vessels, server rooms, and factory floors.

## What's Next?

- Connect real hardware sensors by replacing the closure with GPIO/I2C reads
- Add more alarms for oil pressure, RPM thresholds, and multi-sensor conditions
- Compose multiple engine blocks with `plato-fleet-manager` for fleet-wide monitoring
- Convert sensor data to ternary with `plato-ternary-bridge` for compressed history and consensus
- Compile alarm conditions to FLUX bytecode with `plato-flux-compiler` for deterministic cross-platform execution
