//! Alarm — condition-based rules with cooldowns.

/// An alarm condition callback: receives latest tick data, returns true to fire.
pub type AlarmCondition = Box<dyn Fn(&[(String, f64)]) -> bool + Send + Sync>;

/// State of an alarm.
#[derive(Debug, Clone, PartialEq)]
pub enum AlarmState {
    /// Alarm is idle (not triggered, or cooldown expired).
    Idle,
    /// Alarm is active (currently triggered).
    Active { since_tick: u64 },
    /// Alarm is in cooldown (recently fired, waiting).
    Cooldown { until_tick: u64 },
}

/// An alarm rule with condition and cooldown.
pub struct AlarmRule {
    pub name: String,
    pub condition: AlarmCondition,
    /// Minimum ticks between alarm fires.
    pub cooldown_ticks: u64,
    /// Current state.
    pub state: AlarmState,
    /// Tick when the alarm last fired (for cooldown tracking).
    pub last_fired_tick: u64,
}

impl AlarmRule {
    pub fn new(
        name: impl Into<String>,
        condition: AlarmCondition,
        cooldown_ticks: u64,
    ) -> Self {
        AlarmRule {
            name: name.into(),
            condition,
            cooldown_ticks,
            state: AlarmState::Idle,
            last_fired_tick: 0,
        }
    }

    /// Evaluate the alarm condition against tick data.
    /// Returns true if the alarm fires (transitioned to Active).
    pub fn evaluate(&mut self, data: &[(String, f64)], current_tick: u64) -> bool {
        let triggered = (self.condition)(data);

        match &self.state {
            AlarmState::Idle => {
                if triggered {
                    self.state = AlarmState::Active { since_tick: current_tick };
                    self.last_fired_tick = current_tick;
                    return true;
                }
            }
            AlarmState::Active { since_tick: _ } => {
                if !triggered {
                    self.state = AlarmState::Cooldown {
                        until_tick: current_tick + self.cooldown_ticks,
                    };
                }
            }
            AlarmState::Cooldown { until_tick } => {
                if current_tick >= *until_tick {
                    if triggered {
                        self.state = AlarmState::Active { since_tick: current_tick };
                        self.last_fired_tick = current_tick;
                        return true;
                    } else {
                        self.state = AlarmState::Idle;
                    }
                }
            }
        }
        false
    }

    /// Reset alarm to idle.
    pub fn reset(&mut self) {
        self.state = AlarmState::Idle;
    }
}

impl core::fmt::Debug for AlarmRule {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("AlarmRule")
            .field("name", &self.name)
            .field("cooldown_ticks", &self.cooldown_ticks)
            .field("state", &self.state)
            .finish()
    }
}
