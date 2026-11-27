//! Event queue: a timing wheel of one-minute buckets covering one day, plus an overflow map
//! for events further away. Each agent has at most one *live* event (its `next_event`);
//! stale entries are skipped when popped, which makes cancellation free.
//!
//! The queue is derived state: it is rebuilt from the agents' `next_event` column after
//! loading, so it is never serialized.

use crate::time::{Tick, TICKS_PER_DAY};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct EventQueue {
    buckets: Vec<Vec<u32>>,
    overflow: BTreeMap<Tick, Vec<u32>>,
    now: Tick,
    pub scheduled: u64,
}

impl Default for EventQueue {
    fn default() -> Self {
        EventQueue::new(0)
    }
}

impl EventQueue {
    pub fn new(now: Tick) -> EventQueue {
        EventQueue {
            buckets: vec![Vec::new(); TICKS_PER_DAY as usize],
            overflow: BTreeMap::new(),
            now,
            scheduled: 0,
        }
    }

    /// Schedules `agent` at `t` (`t >= now`).
    #[inline]
    pub fn schedule(&mut self, agent: u32, t: Tick) {
        debug_assert!(t >= self.now, "event in the past: {t} < {}", self.now);
        self.scheduled += 1;
        if t - self.now < TICKS_PER_DAY {
            self.buckets[(t % TICKS_PER_DAY) as usize].push(agent);
        } else {
            self.overflow.entry(t).or_default().push(agent);
        }
    }

    /// Takes every agent scheduled at `now` (sorted by id, de-duplicated) and advances the
    /// queue's clock to `now`. Callers must still check each agent's `next_event`.
    pub fn take_due(&mut self, now: Tick, out: &mut Vec<u32>) {
        self.now = now;
        out.clear();
        std::mem::swap(out, &mut self.buckets[(now % TICKS_PER_DAY) as usize]);
        // Overflow entries that are now within the wheel's horizon move into buckets.
        while let Some((&t, _)) = self.overflow.first_key_value() {
            if t >= now + TICKS_PER_DAY {
                break;
            }
            let (t, agents) = self.overflow.pop_first().unwrap();
            if t == now {
                out.extend(agents);
            } else {
                self.buckets[(t % TICKS_PER_DAY) as usize].extend(agents);
            }
        }
        out.sort_unstable();
        out.dedup();
    }

    /// Marks the start of tick `now` (used after loading).
    pub fn set_now(&mut self, now: Tick) {
        self.now = now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wheel_and_overflow() {
        let mut q = EventQueue::new(0);
        q.schedule(5, 10);
        q.schedule(3, 10);
        q.schedule(7, 3000);
        let mut out = vec![];
        for t in 0..10 {
            q.take_due(t, &mut out);
            assert!(out.is_empty());
        }
        q.take_due(10, &mut out);
        assert_eq!(out, vec![3, 5]);
        for t in 11..3000 {
            q.take_due(t, &mut out);
            assert!(out.is_empty(), "t={t}");
        }
        q.take_due(3000, &mut out);
        assert_eq!(out, vec![7]);
    }
}
