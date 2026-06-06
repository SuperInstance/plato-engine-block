# Plug & Play — plato-engine-block

> Copy these templates. Change the sensor names and values. You're running.

---

## Pattern 1: Minimal Room Monitor

The simplest useful engine — 2 sensors, 1 Hz, queryable via text protocol.

```rust
use plato_engine_block::{PlatoEngine, PlatoEngineBuilder};

fn main() {
    let mut engine = PlatoEngine::builder()
        // ↓ Change these sensor names and closures ↓
        .sensor("temperature", Box::new(|| read_my_sensor_1()))
        .sensor("humidity", Box::new(|| read_my_sensor_2()))
        .tick_hz(1.0)
        .history_capacity(100)
        .build();

    loop {
        let tick = engine.tick();
        println!("{}: temp={:?}, hum={:?}",
            tick.index,
            tick.get("temperature"),
            tick.get("humidity"),
        );
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

fn read_my_sensor_1() -> f64 { 22.5 }  // Replace with real sensor I/O
fn read_my_sensor_2() -> f64 { 45.0 }  // Replace with real sensor I/O
```

**Change:** sensor names, closures, tick_hz, history_capacity.

---

## Pattern 2: Room with Alarm + Actuator

A room that watches for problems and can take action.

```rust
use plato_engine_block::{PlatoEngine, PlatoEngineBuilder};

fn main() {
    let mut engine = PlatoEngine::builder()
        // ↓ Sensors ↓
        .sensor("coolant_temp", Box::new(|| read_coolant()))
        .sensor("oil_pressure", Box::new(|| read_oil()))

        // ↓ Actuator — triggered via protocol or code ↓
        .actuator("cooling_fan", Box::new(|v| {
            println!("Setting fan to {}%", v);
            v >= 0.0 && v <= 100.0
        }))

        // ↓ Alarm — fires when coolant > 95°C ↓
        .alarm(
            "overheat",
            Box::new(|data| {
                data.iter().any(|(n, v)| n == "coolant_temp" && *v > 95.0)
            }),
            10,  // Cooldown ticks
        )

        .tick_hz(2.0)
        .history_capacity(200)
        .build();

    loop {
        engine.tick();

        // Check for alarm fires
        if !engine.alarm_fires.is_empty() {
            for alarm in &engine.alarm_fires {
                println!("🚨 Alarm: {}", alarm);
            }
            // Auto-respond
            engine.handle_command("cooling_fan 100.0");
        }

        std::thread::sleep(std::time::Duration::from_millis(500));
    }
}

fn read_coolant() -> f64 { 85.0 }   // Replace with real read
fn read_oil() -> f64 { 55.0 }       // Replace with real read
```

**Change:** sensor names, alarm conditions, actuator names, thresholds.

---

## Pattern 3: TCP-Connected Room (Networked)

A room accessible over TCP for remote monitoring and control.

```rust
use plato_engine_block::{PlatoEngine, PlatoEngineBuilder};
use plato_engine_block::server::run_server;

#[tokio::main]
async fn main() {
    let engine = PlatoEngine::builder()
        // ↓ Your sensors ↓
        .sensor("temp", Box::new(|| read_temperature()))
        .sensor("pressure", Box::new(|| read_pressure()))
        .actuator("heater", Box::new(|v| { println!("heater={}", v); true }))
        .alarm("overheat", Box::new(|d| {
            d.iter().any(|(n, v)| n == "temp" && *v > 90.0)
        }), 5)
        .tick_hz(1.0)
        .history_capacity(100)
        .build();

    // ↓ Change bind address ↓
    let (handle, join) = run_server(engine, "0.0.0.0:7070").await.unwrap();
    handle.broadcast("Room online".to_string());

    println!("Listening on :7070");
    join.await.unwrap();
}

fn read_temperature() -> f64 { 22.5 }
fn read_pressure() -> f64 { 1013.0 }
```

Connect from any machine:

```bash
$ nc 192.168.1.10 7070
tick                           # Take a reading
history 10                     # Last 10 ticks
heater 75.0                    # Set heater to 75%
subscribe                      # Stream live updates
alarm list                     # Check alarm states
help                           # All commands
```

**Change:** sensors, actuators, alarms, bind address (`host:port`).

---

## Quick Reference

| What | API | Example |
|------|-----|---------|
| Add sensor | `.sensor("name", Box::new(\|\| value))` | `.sensor("temp", Box::new(\|\| 22.5))` |
| Add actuator | `.actuator("name", Box::new(\|v\| bool))` | `.actuator("fan", Box::new(\|v\| v <= 100.0))` |
| Add alarm | `.alarm("name", condition, cooldown)` | `.alarm("hot", Box::new(\|d\| ...), 5)` |
| Set tick rate | `.tick_hz(f64)` | `.tick_hz(10.0)` |
| Set history size | `.history_capacity(usize)` | `.history_capacity(500)` |
| Take a tick | `engine.tick()` | Returns `Tick` with index + sensor data |
| Send command | `engine.handle_command("...")` | `engine.handle_command("tick")` |
| Check alarms | `engine.alarm_fires` | `Vec<String>` of alarm names that fired |
| Get latest | `engine.latest()` | `Option<&Tick>` |
| Query history | `engine.history(n)` | `Vec<&Tick>` — last N ticks |
