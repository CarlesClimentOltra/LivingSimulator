//! Per-agent journal: a fixed-size ring buffer of compact entries (no strings).
//! Text is produced only when someone reads it (see `query`).

use crate::time::Tick;
use livingsim_content::JournalKind;
use serde::{Deserialize, Serialize};

pub const JOURNAL_LEN: usize = 16;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub tick: Tick,
    pub kind: u16,
    /// Small argument: activity index, a count…
    pub arg: u16,
    /// Building, region or amount, depending on `kind`.
    pub subject: u32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Journal {
    entries: Vec<[Entry; JOURNAL_LEN]>,
    /// Number of entries written (mod 256); the next slot is `head % JOURNAL_LEN`.
    head: Vec<u8>,
}

impl Journal {
    pub fn push_agent(&mut self) {
        self.entries.push([Entry::default(); JOURNAL_LEN]);
        self.head.push(0);
    }
    pub fn clear(&mut self, a: u32) {
        self.entries[a as usize] = [Entry::default(); JOURNAL_LEN];
        self.head[a as usize] = 0;
    }
    #[inline]
    pub fn write(&mut self, a: u32, tick: Tick, kind: JournalKind, arg: u16, subject: u32) {
        let h = self.head[a as usize];
        self.entries[a as usize][h as usize % JOURNAL_LEN] = Entry {
            tick,
            kind: kind as u16 + 1,
            arg,
            subject,
        };
        self.head[a as usize] = h.wrapping_add(1);
    }
    /// Translates the activity index stored in `Started` entries (content remapping).
    pub fn remap_activities(&mut self, f: impl Fn(u16) -> u16) {
        let started = livingsim_content::JournalKind::Started as u16 + 1;
        for entries in self.entries.iter_mut() {
            for e in entries.iter_mut() {
                if e.kind == started {
                    e.arg = f(e.arg);
                }
            }
        }
    }

    /// Entries from newest to oldest.
    pub fn read(&self, a: u32) -> Vec<Entry> {
        let h = self.head[a as usize] as usize;
        let e = &self.entries[a as usize];
        (1..=JOURNAL_LEN)
            .map(|i| e[(h + JOURNAL_LEN * 16 - i) % JOURNAL_LEN])
            .filter(|x| x.kind != 0)
            .map(|mut x| {
                x.kind -= 1;
                x
            })
            .collect()
    }
    pub fn bytes_per_agent() -> usize {
        std::mem::size_of::<[Entry; JOURNAL_LEN]>() + 1
    }
}
