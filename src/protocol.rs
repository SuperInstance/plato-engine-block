//! Protocol — text command parser and JSON response formatter.
//!
//! Implements the PLATO Wire Protocol v0.1 specification.
//! All responses are single-line JSON objects with a `type` field.

use crate::engine::PlatoEngine;
use crate::tick::Tick;

/// Handles text protocol commands for a PlatoEngine.
pub struct ProtocolHandler;

impl ProtocolHandler {
    /// Parse and execute a text command against the engine.
    /// Returns a JSON response string per PLATO Wire Protocol v0.1.
    pub fn handle(engine: &mut PlatoEngine, command: &str) -> String {
        let cmd = command.trim();

        // Handle multi-word commands first
        if cmd == "alarm list" {
            return Self::format_alarm_list(engine);
        }
        if cmd.starts_with("alarm set ") {
            return Self::handle_alarm_set(engine, cmd);
        }

        let parts: Vec<&str> = cmd.splitn(3, ' ').collect();

        match parts.get(0).map(|s| *s) {
            Some("tick") => {
                let tick = engine.tick();
                Self::format_tick(tick)
            }
            Some("history") => {
                let n: usize = parts
                    .get(1)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(10);
                Self::format_history(engine, n)
            }
            Some("actuator") => {
                // actuator <name> <value>
                if let (Some(name), Some(val_str)) = (parts.get(1), parts.get(2)) {
                    if let Ok(value) = val_str.parse::<f64>() {
                        match engine.set_actuator(name, value) {
                            Ok(true) => format!(
                "{{\"type\":\"ack\",\"command\":\"actuator\",\"name\":\"{}\",\"value\":{:.4}}}",
                name, value
                            ),
                            Ok(false) => format!(
                "{{\"type\":\"error\",\"message\":\"actuator '{}' rejected value {}\"}}",
                name, value
                            ),
                            Err(e) => format!(
                "{{\"type\":\"error\",\"message\":\"{}\"}}",
                e
                            ),
                        }
                    } else {
                        format!(
                            "{{\"type\":\"error\",\"message\":\"invalid actuator value: {}\"}}",
                            val_str
                        )
                    }
                } else {
                    "{{\"type\":\"error\",\"message\":\"usage: actuator <name> <value>\"}}".to_string()
                }
            }
            Some("subscribe") => {
                engine.subscribe();
                format!(
                    "{{\"type\":\"subscribed\",\"tick_hz\":{:.1}}}",
                    engine.tick_hz
                )
            }
            Some("unsubscribe") => {
                engine.unsubscribe();
                "{\"type\":\"unsubscribed\"}".to_string()
            }
            Some("help") => {
                Self::format_help()
            }
            Some("quit") => {
                "{\"type\":\"bye\"}".to_string()
            }
            Some(name) => {
                // Backward compat: bare <name> <value> as actuator command
                if let Some(val_str) = parts.get(1) {
                    if let Ok(value) = val_str.parse::<f64>() {
                        match engine.set_actuator(name, value) {
                            Ok(true) => format!(
                "{{\"type\":\"ack\",\"command\":\"actuator\",\"name\":\"{}\",\"value\":{:.4}}}",
                name, value
                            ),
                            Ok(false) => format!(
                "{{\"type\":\"error\",\"message\":\"actuator '{}' rejected value {}\"}}",
                name, value
                            ),
                            Err(e) => format!(
                "{{\"type\":\"error\",\"message\":\"{}\"}}",
                e
                            ),
                        }
                    } else {
                        format!(
                            "{{\"type\":\"error\",\"message\":\"unknown command: {}\"}}",
                            cmd
                        )
                    }
                } else {
                    format!(
                        "{{\"type\":\"error\",\"message\":\"unknown command: {}\"}}",
                        cmd
                    )
                }
            }
            None => {
                "{\"type\":\"error\",\"message\":\"empty command\"}".to_string()
            }
        }
    }

    fn format_tick(tick: &Tick) -> String {
        let data_pairs: Vec<String> = tick
            .data
            .iter()
            .map(|(name, val)| format!("\"{}\":{:.4}", name, val))
            .collect();
        format!(
            "{{\"type\":\"tick\",\"t\":{:.3},\"seq\":{},\"data\":{{{}}}}}",
            tick.timestamp,
            tick.index,
            data_pairs.join(",")
        )
    }

    fn format_history(engine: &PlatoEngine, n: usize) -> String {
        let ticks = engine.history(n);
        if ticks.is_empty() {
            return "{\"type\":\"history\",\"count\":0,\"ticks\":[]}".to_string();
        }
        let tick_strs: Vec<String> = ticks
            .iter()
            .map(|tick| {
                let data_pairs: Vec<String> = tick
                    .data
                    .iter()
                    .map(|(name, val)| format!("\"{}\":{:.4}", name, val))
                    .collect();
                format!(
                    "{{\"t\":{:.3},\"seq\":{},\"data\":{{{}}}}}",
                    tick.timestamp,
                    tick.index,
                    data_pairs.join(",")
                )
            })
            .collect();
        format!(
            "{{\"type\":\"history\",\"count\":{},\"ticks\":[{}]}}",
            ticks.len(),
            tick_strs.join(",")
        )
    }

    fn format_alarm_list(engine: &PlatoEngine) -> String {
        if engine.alarms.is_empty() {
            return "{\"type\":\"alarm_list\",\"alarms\":[]}".to_string();
        }
        let alarm_strs: Vec<String> = engine
            .alarms
            .iter()
            .map(|a| {
                let state = match &a.state {
                    crate::alarm::AlarmState::Idle => "idle",
                    crate::alarm::AlarmState::Active { .. } => "active",
                    crate::alarm::AlarmState::Cooldown { .. } => "cooldown",
                };
                format!(
                    "{{\"id\":\"{}\",\"condition\":\"\",\"cooldown_sec\":{},\"last_triggered\":null,\"state\":\"{}\"}}",
                    a.name, a.cooldown_ticks, state
                )
            })
            .collect();
        format!(
            "{{\"type\":\"alarm_list\",\"alarms\":[{}]}}",
            alarm_strs.join(",")
        )
    }

    fn handle_alarm_set(_engine: &mut PlatoEngine, _cmd: &str) -> String {
        // alarm set <id> <condition> <cooldown>
        // Full runtime alarm configuration will require extending the engine API.
        // For now, return an ack with the parsed id.
        let parts: Vec<&str> = _cmd.split_whitespace().collect();
        if parts.len() >= 4 {
            let id = parts.get(2).unwrap_or("");
            format!(
                "{{\"type\":\"ack\",\"command\":\"alarm_set\",\"id\":\"{}\"}}",
                id
            )
        } else {
            "{\"type\":\"error\",\"message\":\"usage: alarm set <id> <condition> <cooldown>\"}"
                .to_string()
        }
    }

    fn format_help() -> String {
        format!(
            "{{\"type\":\"help\",\"commands\":[\"tick\",\"history [N]\",\"actuator <name> <value>\",\"alarm list\",\"alarm set <id> <condition> <cooldown>\",\"subscribe\",\"unsubscribe\",\"help\",\"quit\"]}}"
        )
    }
}
