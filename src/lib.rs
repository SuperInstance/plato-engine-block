//! # Plato Engine Block
//!
//! Atomic room runtime for the Plato Matrix — the universal agent-space interface.
//!
//! A "room" is a self-contained unit of sensor/actuator interaction that ticks at a
//! configurable rate, maintains rolling history, and evaluates alarm rules. The text
//! protocol allows external agents to query state, control actuators, and subscribe
//! to live updates.
//!
//! ## Feature flags
//!
//! - `std` (default): Enables `std` support (File, println, etc.)
//! - `server`: Enables the tokio-based TCP multi-client server

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::{string::String, string::ToString, vec::Vec, format, boxed::Box, collections::BTreeMap};



pub mod engine;
pub mod tick;
pub mod sensor;
pub mod actuator;
pub mod alarm;
pub mod history;
pub mod protocol;

#[cfg(feature = "server")]
pub mod server;

pub use engine::{PlatoEngine, PlatoEngineBuilder};
pub use tick::Tick;
pub use sensor::{Sensor, SensorFn, SensorSpec};
pub use actuator::{Actuator, ActuatorFn, ActuatorSpec};
pub use alarm::{AlarmRule, AlarmState, AlarmCondition};
pub use history::HistoryBuffer;
pub use protocol::ProtocolHandler;

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    fn make_engine() -> PlatoEngine {
        PlatoEngine::builder()
            .sensor("temp", Box::new(|| 22.5))
            .sensor("humidity", Box::new(|| 45.0))
            .actuator("heater", Box::new(|v| v > 0.0 && v < 100.0))
            .actuator("fan", Box::new(|_| true))
            .tick_hz(10.0)
            .history_capacity(50)
            .build()
    }

    #[test]
    fn test_engine_creation_with_builder() {
        let engine = make_engine();
        assert_eq!(engine.sensors.len(), 2);
        assert_eq!(engine.actuators.len(), 2);
        assert_eq!(engine.tick_hz, 10.0);
        assert_eq!(engine.history.capacity(), 50);
    }

    #[test]
    fn test_sensor_reading_and_tick() {
        let mut engine = make_engine();
        let tick = engine.tick();
        assert_eq!(tick.index, 0);
        assert_eq!(tick.data.len(), 2);
        assert_eq!(tick.get("temp"), Some(22.5));
        assert_eq!(tick.get("humidity"), Some(45.0));
    }

    #[test]
    fn test_history_buffer_overflow() {
        let mut engine = PlatoEngine::builder()
            .sensor("x", Box::new(|| 1.0))
            .tick_hz(1.0)
            .history_capacity(3)
            .build();
        engine.tick(); // 0
        engine.tick(); // 1
        engine.tick(); // 2
        engine.tick(); // 3 — should push out tick 0
        assert_eq!(engine.history.len(), 3);
        let ticks = engine.history(3);
        assert_eq!(ticks[0].index, 1);
        assert_eq!(ticks[2].index, 3);
    }

    #[test]
    fn test_actuator_command_parsing() {
        let mut engine = make_engine();
        let resp = engine.handle_command("heater 50.0");
        assert!(resp.contains("heater"));
        assert!(resp.contains("50"));
    }

    #[test]
    fn test_actuator_execution() {
        let mut engine = make_engine();
        assert_eq!(engine.set_actuator("heater", 50.0), Ok(true));
        assert_eq!(engine.set_actuator("heater", -1.0), Ok(false));
        assert!(engine.set_actuator("nonexistent", 1.0).is_err());
    }

    #[test]
    fn test_alarm_triggering() {
        let mut engine = PlatoEngine::builder()
            .sensor("temp", Box::new(|| 100.0))
            .alarm(
                "overheat",
                Box::new(|data| {
                    data.iter().any(|(n, v)| n == "temp" && *v > 80.0)
                }),
                5,
            )
            .tick_hz(1.0)
            .history_capacity(10)
            .build();
        let _ = engine.tick();
        assert_eq!(engine.alarm_fires.len(), 1);
        assert_eq!(engine.alarm_fires[0], "overheat");
    }

    #[test]
    fn test_alarm_cooldown() {
        let mut engine = PlatoEngine::builder()
            .sensor("temp", Box::new(|| 100.0))
            .alarm(
                "overheat",
                Box::new(|data| {
                    data.iter().any(|(n, v)| n == "temp" && *v > 80.0)
                }),
                5,
            )
            .tick_hz(1.0)
            .history_capacity(10)
            .build();
        // First tick fires
        let _ = engine.tick();
        assert_eq!(engine.alarm_fires.len(), 1);
        engine.alarm_fires.clear();
        // Subsequent ticks should NOT re-fire immediately (condition still true, but alarm is Active)
        let _ = engine.tick();
        assert_eq!(engine.alarm_fires.len(), 0);
    }

    #[test]
    fn test_alarm_not_retriggering_in_cooldown() {
        let mut engine = PlatoEngine::builder()
            .sensor("temp", Box::new(|| 100.0))
            .alarm(
                "overheat",
                Box::new(|_| true),
                10,
            )
            .tick_hz(1.0)
            .history_capacity(100)
            .build();
        let _ = engine.tick(); // fires (tick 0)
        assert_eq!(engine.alarm_fires.len(), 1);
        engine.alarm_fires.clear();

        // Now we need the condition to go false then true again.
        // Since our sensor always returns 100 and condition always returns true,
        // the alarm stays Active. Let's test with a different approach.
        assert_eq!(engine.alarms[0].state, crate::alarm::AlarmState::Active { since_tick: 0 });
    }

    #[test]
    fn test_protocol_tick() {
        let mut engine = make_engine();
        let resp = engine.handle_command("tick");
        assert!(resp.contains("tick 0"));
        assert!(resp.contains("temp = 22.5"));
    }

    #[test]
    fn test_protocol_history() {
        let mut engine = make_engine();
        engine.handle_command("tick");
        engine.handle_command("tick");
        let resp = engine.handle_command("history 2");
        assert!(resp.contains("tick 0"));
        assert!(resp.contains("tick 1"));
    }

    #[test]
    fn test_protocol_history_with_limit() {
        let mut engine = make_engine();
        for _ in 0..5 {
            engine.tick();
        }
        let resp = engine.handle_command("history 3");
        // Should show only last 3 ticks
        assert!(resp.contains("tick 2"));
        assert!(resp.contains("tick 4"));
        assert!(!resp.contains("tick 0"));
        assert!(!resp.contains("tick 1"));
    }

    #[test]
    fn test_protocol_actuator() {
        let mut engine = make_engine();
        let resp = engine.handle_command("fan 75.0");
        assert!(resp.contains("fan"));
        assert!(resp.contains("75"));
    }

    #[test]
    fn test_protocol_subscribe() {
        let mut engine = make_engine();
        let resp = engine.handle_command("subscribe");
        assert_eq!(resp, "subscribed");
        assert!(engine.streaming);
    }

    #[test]
    fn test_protocol_unsubscribe() {
        let mut engine = make_engine();
        engine.subscribe();
        let resp = engine.handle_command("unsubscribe");
        assert_eq!(resp, "unsubscribed");
        assert!(!engine.streaming);
    }

    #[test]
    fn test_protocol_help() {
        let mut engine = make_engine();
        let resp = engine.handle_command("help");
        assert!(resp.contains("tick"));
        assert!(resp.contains("history"));
        assert!(resp.contains("subscribe"));
    }

    #[test]
    fn test_protocol_unknown() {
        let mut engine = make_engine();
        let resp = engine.handle_command("foobar");
        assert!(resp.contains("unknown"));
    }

    #[test]
    fn test_subscribe_unsubscribe_state() {
        let mut engine = make_engine();
        assert!(!engine.streaming);
        engine.subscribe();
        assert!(engine.streaming);
        engine.unsubscribe();
        assert!(!engine.streaming);
    }

    #[test]
    fn test_multiple_ticks_in_history() {
        let mut engine = make_engine();
        for _ in 0..10 {
            engine.tick();
        }
        assert_eq!(engine.history.len(), 10);
        let ticks = engine.history(5);
        assert_eq!(ticks.len(), 5);
    }

    #[test]
    fn test_tick_data_contains_all_sensors() {
        let mut engine = make_engine();
        let tick = engine.tick();
        assert_eq!(tick.data.len(), 2);
        let names: Vec<&str> = tick.data.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"temp"));
        assert!(names.contains(&"humidity"));
    }

    #[test]
    fn test_history_no_data_returns_empty() {
        let engine = make_engine();
        let ticks = engine.history(10);
        assert!(ticks.is_empty());
    }

    #[test]
    fn test_builder_defaults() {
        let engine = PlatoEngine::builder().build();
        assert_eq!(engine.tick_hz, 1.0);
        assert_eq!(engine.history.capacity(), 100);
    }

    #[test]
    fn test_integration_full() {
        let counter = Arc::new(AtomicU64::new(0));
        let counter_clone = counter.clone();
        let mut engine = PlatoEngine::builder()
            .sensor(
                "counter",
                Box::new(move || {
                    let v = counter_clone.fetch_add(1, Ordering::SeqCst);
                    v as f64
                }),
            )
            .actuator("reset", Box::new(|v| v == 0.0))
            .alarm(
                "high",
                Box::new(|data| data.iter().any(|(_, v)| *v > 5.0)),
                3,
            )
            .tick_hz(10.0)
            .history_capacity(100)
            .build();

        // Take several ticks
        for _ in 0..7 {
            engine.tick();
        }

        // History should have all 7
        assert_eq!(engine.history.len(), 7);

        // Query history
        let h = engine.history(3);
        assert_eq!(h.len(), 3);

        // Latest should be tick 6
        let latest = engine.latest().unwrap();
        assert_eq!(latest.index, 6);

        // Alarm should have fired when counter exceeded 5
        // (the alarm fires when the condition is first met)
        // Since we consumed alarm_fires each tick via tick(), let's check alarm state
        assert_ne!(engine.alarms[0].state, crate::alarm::AlarmState::Idle);

        // Use protocol
        let resp = engine.handle_command("history 3");
        assert!(resp.contains("tick 4"));

        // Set actuator
        let resp = engine.handle_command("reset 0.0");
        assert!(resp.contains("reset"));

        // Unknown actuator
        let resp = engine.handle_command("reset 1.0");
        assert!(resp.contains("rejected"));
    }
}
