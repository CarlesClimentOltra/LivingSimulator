//! Messages between regions. Regions never read each other's state: everything that
//! crosses a border is a message, delivered at the start of the next exchange window.

use crate::time::Tick;
use serde::{Deserialize, Serialize};

/// Ticks per exchange window (1 simulated hour).
pub const WINDOW: Tick = 60;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub src: u16,
    pub dest: u16,
    /// Order inside the source region's outbox.
    pub seq: u32,
    pub sent: Tick,
    pub body: Body,
}

/// A person travelling to another region for a visit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Traveller {
    /// Agent id in the home region.
    pub agent: u32,
    pub given: String,
    pub family: String,
    pub birth: i64,
    pub female: bool,
    pub traits: u32,
    pub needs: [u16; livingsim_content::NEED_COUNT],
    pub money: i64,
    /// Tick at which the traveller appears at the destination's highway exit.
    pub arrival: Tick,
    /// Tick at which they must be back at the highway exit to leave.
    pub leave_by: Tick,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Body {
    /// Goods given to the destination without payment (Phase 1 format, kept so old
    /// snapshots still decode).
    Trade {
        good: u16,
        qty: u32,
    },
    Traveller(Traveller),
    /// Sent back to the home region when a visit ends.
    VisitReport {
        agent: u32,
        spent_cents: i64,
        places_visited: u16,
    },
    /// Phase 3: a household moves to another region.
    Migration {
        household_size: u8,
    },
    /// Phase 4: construction materials.
    MaterialShipment {
        good: u16,
        qty: u32,
    },
    /// Phase 6: a new highway connects regions.
    RoadBuilt {
        from_gateway: u32,
    },
    /// Phase 7: a technology spreads.
    TechDiffusion {
        tech: u16,
    },
    /// Newspaper item worth reporting in neighbouring towns.
    NewsItem {
        kind: u16,
        value: u32,
    },
    /// Phase 2: goods shipped with an invoice; the receiver pays with `Payment`.
    Sale {
        good: u16,
        qty: u32,
        unit_price: i64,
        seller_firm: u32,
    },
    /// Phase 2: money for a `Sale`, credited to the seller firm.
    Payment {
        firm: u32,
        amount: i64,
    },
}

/// Messages delivered to one region over a span of ticks, as published by the runner.
/// `deliveries` holds only ticks with at least one message; ticks in `from..to` without an
/// entry had no messages (so the span is complete).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageLog {
    pub region: u16,
    pub from: Tick,
    pub to: Tick,
    pub deliveries: Vec<(Tick, Vec<Message>)>,
}
