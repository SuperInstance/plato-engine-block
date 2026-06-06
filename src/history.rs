//! History — circular buffer of ticks.

use crate::tick::Tick;

/// A circular buffer holding recent ticks.
#[derive(Debug, Clone)]
pub struct HistoryBuffer {
    buffer: Vec<Option<Tick>>,
    capacity: usize,
    head: usize,
    len: usize,
}

impl HistoryBuffer {
    /// Create a new history buffer with the given capacity.
    pub fn new(capacity: usize) -> Self {
        let mut buffer = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            buffer.push(None);
        }
        HistoryBuffer {
            buffer,
            capacity,
            head: 0,
            len: 0,
        }
    }

    /// Push a tick into the buffer. Overwrites oldest if full.
    pub fn push(&mut self, tick: Tick) {
        self.buffer[self.head] = Some(tick);
        self.head = (self.head + 1) % self.capacity;
        if self.len < self.capacity {
            self.len += 1;
        }
    }

    /// Get the most recent tick.
    pub fn latest(&self) -> Option<&Tick> {
        if self.len == 0 {
            return None;
        }
        let idx = if self.head == 0 {
            self.capacity - 1
        } else {
            self.head - 1
        };
        self.buffer[idx].as_ref()
    }

    /// Get the last `n` ticks, ordered oldest-first.
    pub fn query(&self, n: usize) -> Vec<&Tick> {
        let count = n.min(self.len);
        let mut result = Vec::with_capacity(count);
        // The oldest entry is at position (head - len + capacity) % capacity
        let start = if self.len < self.capacity {
            0
        } else {
            self.head
        };
        for i in 0..count {
            let idx = (start + self.len - count + i) % self.capacity;
            if let Some(ref tick) = self.buffer[idx] {
                result.push(tick);
            }
        }
        result
    }

    /// Current number of ticks in the buffer.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Is the buffer empty?
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Buffer capacity.
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}
