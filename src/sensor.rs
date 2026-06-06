//! Sensor — callback-based readers that return f64.

/// A sensor callback: takes nothing, returns an f64 reading.
pub trait Sensor {
    fn read(&self) -> f64;
}

/// Type-erased sensor callback.
pub type SensorFn = Box<dyn Fn() -> f64 + Send + Sync>;

/// A named sensor with its callback.
pub struct SensorSpec {
    pub name: String,
    pub callback: SensorFn,
}

impl SensorSpec {
    pub fn new(name: impl Into<String>, callback: SensorFn) -> Self {
        SensorSpec {
            name: name.into(),
            callback,
        }
    }
}

impl core::fmt::Debug for SensorSpec {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SensorSpec")
            .field("name", &self.name)
            .finish()
    }
}

/// Blanket impl for closures.
impl<F: Fn() -> f64 + Send + Sync + 'static> Sensor for F {
    fn read(&self) -> f64 {
        self()
    }
}
