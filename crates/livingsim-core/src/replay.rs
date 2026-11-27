//! Simulating a single region alone, fed by the runner's message log. This is what the
//! browser does: it downloads one region checkpoint plus the messages delivered to it and
//! reaches a state bit-identical to the runner's, without simulating any other region.

use crate::messages::{Message, MessageLog, WINDOW};
use crate::region::Region;
use crate::time::Tick;
use std::collections::BTreeMap;

pub struct RegionReplay {
    pub region: Region,
    inbound: BTreeMap<Tick, Vec<Message>>,
    /// Inbound messages are known for ticks `< known_until`.
    known_until: Tick,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ReplayError {
    /// Messages for this tick are not known yet (the runner has not published them).
    MissingMessages(Tick),
}

impl RegionReplay {
    pub fn new(region: Region) -> RegionReplay {
        let t = region.tick;
        RegionReplay {
            region,
            inbound: BTreeMap::new(),
            known_until: t,
        }
    }

    /// Adds a message log. Logs must be contiguous with what is already known.
    pub fn add_log(&mut self, log: MessageLog) {
        if log.region != self.region.id {
            return;
        }
        for (t, msgs) in log.deliveries {
            if t >= self.region.tick {
                self.inbound.insert(t, msgs);
            }
        }
        if log.from <= self.known_until {
            self.known_until = self.known_until.max(log.to);
        }
    }

    pub fn known_until(&self) -> Tick {
        self.known_until
    }

    pub fn tick(&self) -> Tick {
        self.region.tick
    }

    pub fn step(&mut self) -> Result<(), ReplayError> {
        let t = self.region.tick;
        if t.is_multiple_of(WINDOW) {
            if t >= self.known_until {
                return Err(ReplayError::MissingMessages(t));
            }
            // Our own outbox was already delivered by the runner; the log has the result.
            self.region.take_outbox();
            let msgs = self.inbound.remove(&t).unwrap_or_default();
            self.region.receive(&msgs);
        }
        self.region.step();
        Ok(())
    }

    pub fn step_to(&mut self, target: Tick) -> Result<(), ReplayError> {
        while self.region.tick < target {
            self.step()?;
        }
        Ok(())
    }
}
