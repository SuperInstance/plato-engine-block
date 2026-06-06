//! Actuator — callback-based writers that take f64.

/// An actuator callback: takes an f64 value, returns success.
pub trait Actuator {
    fn write(&self, value: f64) -> bool;
}

/// Type-erased actuator callback.
pub type ActuatorFn = Box<dyn Fn(f64) -> bool + Send + Sync>;

/// A named actuator with its callback.
pub struct ActuatorSpec {
    pub name: String,
    pub callback: ActuatorFn,
}

impl ActuatorSpec {
    pub fn new(name: impl Into<String>, callback: ActuatorFn) -> Self {
        ActuatorSpec {
            name: name.into(),
            callback,
        }
    }

    /// Set the actuator value. Returns true on success.
    pub fn set(&self, value: f64) -> bool {
        (self.callback)(value)
    }
}

impl core::fmt::Debug for ActuatorSpec {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ActuatorSpec")
            .field("name", &self.name)
            .finish()
    }
}
