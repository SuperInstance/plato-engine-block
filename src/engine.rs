//! Engine — the core PlatoEngine struct.

use crate::actuator::ActuatorSpec;
use crate::alarm::AlarmRule;
use crate::history::HistoryBuffer;
use crate::sensor::SensorSpec;
use crate::tick::Tick;

/// The core engine: sensors, actuators, history, alarms.
pub struct PlatoEngine {
    pub sensors: Vec<SensorSpec>,
    pub actuators: Vec<ActuatorSpec>,
    pub alarms: Vec<AlarmRule>,
    pub history: HistoryBuffer,
    pub tick_hz: f64,
    pub tick_index: u64,
    pub start_time: f64,
    pub streaming: bool,
    /// Alarm names that fired on the last tick (consumed by protocol handler).
    pub alarm_fires: Vec<String>,
}

impl PlatoEngine {
    /// Create a new builder.
    pub fn builder() -> PlatoEngineBuilder {
        PlatoEngineBuilder::default()
    }

    /// Take one tick: read sensors, push to history, evaluate alarms.
    pub fn tick(&mut self) -> &Tick {
        let ts = self.start_time + self.tick_index as f64 / self.tick_hz.max(0.001);
        let tick = Tick::from_sensors(self.tick_index, ts, &self.sensors);
        // Evaluate alarms before pushing
        let data_slice: Vec<(String, f64)> = tick.data.clone();
        for alarm in &mut self.alarms {
            if alarm.evaluate(&data_slice, self.tick_index) {
                self.alarm_fires.push(alarm.name.clone());
            }
        }
        self.tick_index += 1;
        self.history.push(tick);
        self.history.latest().unwrap()
    }

    /// Get the latest tick.
    pub fn latest(&self) -> Option<&Tick> {
        self.history.latest()
    }

    /// Get the last `n` ticks from history.
    pub fn history(&self, n: usize) -> Vec<&Tick> {
        self.history.query(n)
    }

    /// Set an actuator by name.
    pub fn set_actuator(&mut self, name: &str, value: f64) -> Result<bool, String> {
        for act in &self.actuators {
            if act.name == name {
                return Ok(act.set(value));
            }
        }
        Err(format!("unknown actuator: {}", name))
    }

    /// Subscribe to live updates.
    pub fn subscribe(&mut self) {
        self.streaming = true;
    }

    /// Unsubscribe from live updates.
    pub fn unsubscribe(&mut self) {
        self.streaming = false;
    }

    /// Handle a text command.
    pub fn handle_command(&mut self, command: &str) -> String {
        crate::protocol::ProtocolHandler::handle(self, command)
    }
}

/// Builder for PlatoEngine.
#[derive(Default)]
pub struct PlatoEngineBuilder {
    sensors: Vec<SensorSpec>,
    actuators: Vec<ActuatorSpec>,
    alarms: Vec<AlarmRule>,
    tick_hz: f64,
    history_capacity: usize,
}

impl PlatoEngineBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a sensor with a callback.
    pub fn sensor(mut self, name: impl Into<String>, callback: crate::sensor::SensorFn) -> Self {
        self.sensors.push(SensorSpec::new(name, callback));
        self
    }

    /// Add an actuator with a callback.
    pub fn actuator(
        mut self,
        name: impl Into<String>,
        callback: crate::actuator::ActuatorFn,
    ) -> Self {
        self.actuators.push(ActuatorSpec::new(name, callback));
        self
    }

    /// Add an alarm rule.
    pub fn alarm(
        mut self,
        name: impl Into<String>,
        condition: crate::alarm::AlarmCondition,
        cooldown_ticks: u64,
    ) -> Self {
        self.alarms
            .push(AlarmRule::new(name, condition, cooldown_ticks));
        self
    }

    /// Set tick rate in Hz.
    pub fn tick_hz(mut self, hz: f64) -> Self {
        self.tick_hz = hz;
        self
    }

    /// Set history buffer capacity.
    pub fn history_capacity(mut self, capacity: usize) -> Self {
        self.history_capacity = capacity;
        self
    }

    /// Build the engine.
    pub fn build(self) -> PlatoEngine {
        let tick_hz = if self.tick_hz > 0.0 { self.tick_hz } else { 1.0 };
        let history_capacity = if self.history_capacity > 0 {
            self.history_capacity
        } else {
            100
        };
        PlatoEngine {
            sensors: self.sensors,
            actuators: self.actuators,
            alarms: self.alarms,
            history: HistoryBuffer::new(history_capacity),
            tick_hz,
            tick_index: 0,
            start_time: 0.0,
            streaming: false,
            alarm_fires: Vec::new(),
        }
    }
}
