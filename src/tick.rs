//! Tick — a single snapshot of all sensor readings at a point in time.

use crate::sensor::SensorSpec;

/// A single tick: timestamp + all sensor values.
#[derive(Debug, Clone)]
pub struct Tick {
    /// Monotonic tick index.
    pub index: u64,
    /// Timestamp in seconds since engine start (or epoch).
    pub timestamp: f64,
    /// Sensor readings: (name, value).
    pub data: Vec<(String, f64)>,
}

impl Tick {
    /// Create a new tick by reading all sensors.
    pub fn from_sensors(index: u64, timestamp: f64, sensors: &[SensorSpec]) -> Self {
        let data: Vec<(String, f64)> = sensors
            .iter()
            .map(|s| {
                let val = (s.callback)();
                (s.name.clone(), val)
            })
            .collect();
        Tick {
            index,
            timestamp,
            data,
        }
    }

    /// Get a sensor value by name.
    pub fn get(&self, sensor_name: &str) -> Option<f64> {
        self.data
            .iter()
            .find(|(name, _)| name == sensor_name)
            .map(|(_, v)| *v)
    }
}
