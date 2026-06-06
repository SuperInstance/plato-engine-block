//! Protocol — text command parser and response formatter.

use crate::engine::PlatoEngine;

/// Handles text protocol commands for a PlatoEngine.
pub struct ProtocolHandler;

impl ProtocolHandler {
    /// Parse and execute a text command against the engine.
    /// Returns the response string.
    pub fn handle(engine: &mut PlatoEngine, command: &str) -> String {
        let cmd = command.trim();
        let parts: Vec<&str> = cmd.splitn(3, ' ').collect();

        match parts.get(0).map(|s| *s) {
            Some("tick") => {
                let tick = engine.tick();
                let mut lines = Vec::new();
                lines.push(format!("tick {} @ {:.3}s", tick.index, tick.timestamp));
                for (name, val) in &tick.data {
                    lines.push(format!("  {} = {:.4}", name, val));
                }
                // Check for any alarm fires
                let alarms = engine.alarm_fires.drain(..).collect::<Vec<_>>();
                for alarm_name in alarms {
                    lines.push(format!("! ALARM: {}", alarm_name));
                }
                lines.join("\n")
            }
            Some("history") => {
                let n: usize = parts
                    .get(1)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(10);
                let ticks = engine.history(n);
                if ticks.is_empty() {
                    "no history".to_string()
                } else {
                    let mut lines = Vec::new();
                    for tick in ticks {
                        let mut data_strs = Vec::new();
                        for (name, val) in &tick.data {
                            data_strs.push(format!("{}={:.4}", name, val));
                        }
                        lines.push(format!(
                            "tick {} @ {:.3}s: {}",
                            tick.index,
                            tick.timestamp,
                            data_strs.join(", ")
                        ));
                    }
                    lines.join("\n")
                }
            }
            Some("subscribe") => {
                engine.subscribe();
                "subscribed".to_string()
            }
            Some("unsubscribe") => {
                engine.unsubscribe();
                "unsubscribed".to_string()
            }
            Some("alarm") => {
                match parts.get(1).map(|s| *s) {
                    Some("list") => {
                        let alarms = &engine.alarms;
                        if alarms.is_empty() {
                            "no alarms".to_string()
                        } else {
                            let lines: Vec<String> = alarms
                                .iter()
                                .map(|a| {
                                    format!("{}: {:?} (cooldown={}ticks)", a.name, a.state, a.cooldown_ticks)
                                })
                                .collect();
                            lines.join("\n")
                        }
                    }
                    _ => "usage: alarm list".to_string(),
                }
            }
            Some("help") => {
                "Commands:\n\
                 tick              — take one tick, read all sensors\n\
                 history [N]       — show last N ticks (default 10)\n\
                 <actuator> <val>  — set actuator to value\n\
                 alarm list        — list alarm states\n\
                 subscribe         — subscribe to live updates\n\
                 unsubscribe       — unsubscribe from updates\n\
                 help              — show this help"
                    .to_string()
            }
            Some(name) => {
                // Try as actuator command: <name> <value>
                if let Some(val_str) = parts.get(1) {
                    if let Ok(value) = val_str.parse::<f64>() {
                        match engine.set_actuator(name, value) {
                            Ok(true) => format!("{} <- {:.4}", name, value),
                            Ok(false) => format!("error: {} rejected value", name),
                            Err(e) => format!("error: {}", e),
                        }
                    } else {
                        format!("unknown command: {}", cmd)
                    }
                } else {
                    format!("unknown command: {}", cmd)
                }
            }
            None => "error: empty command".to_string(),
        }
    }
}
