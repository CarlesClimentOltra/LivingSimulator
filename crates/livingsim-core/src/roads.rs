//! Road graph, routing and the route cache.
//!
//! Routing is exact Dijkstra/A*-equivalent on a small per-region graph (intersections are
//! nodes). Distances from a source node are cached as a full table; any (source, target)
//! query after the first is O(1). The cache is *derived* data: it is never serialized and
//! never affects results, only speed. Every edit to the graph bumps `version`, which
//! invalidates the cache.

use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::collections::BinaryHeap;

pub const NONE: u32 = u32::MAX;
const UNREACHABLE: u32 = u32::MAX / 4;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoadClass {
    Street = 0,
    Avenue = 1,
    Highway = 2,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    Walk = 0,
    Car = 1,
}

/// Deciseconds needed to cross one tile (8 m).
pub fn decis_per_tile(mode: Mode, class: RoadClass) -> u32 {
    match (mode, class) {
        (Mode::Walk, _) => 57,                // ~1.4 m/s
        (Mode::Car, RoadClass::Street) => 10, // ~30 km/h
        (Mode::Car, RoadClass::Avenue) => 6,  // ~50 km/h
        (Mode::Car, RoadClass::Highway) => 3, // ~95 km/h
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RoadGraph {
    pub node_x: Vec<u16>,
    pub node_y: Vec<u16>,
    pub edge_a: Vec<u32>,
    pub edge_b: Vec<u32>,
    /// Length in tiles.
    pub edge_len: Vec<u16>,
    pub edge_class: Vec<RoadClass>,
    pub version: u32,
    #[serde(skip)]
    adj_start: Vec<u32>,
    #[serde(skip)]
    adj: Vec<(u32, u32)>,
}

impl RoadGraph {
    pub fn add_node(&mut self, x: u16, y: u16) -> u32 {
        self.node_x.push(x);
        self.node_y.push(y);
        self.version += 1;
        (self.node_x.len() - 1) as u32
    }
    pub fn add_edge(&mut self, a: u32, b: u32, len: u16, class: RoadClass) -> u32 {
        self.edge_a.push(a);
        self.edge_b.push(b);
        self.edge_len.push(len.max(1));
        self.edge_class.push(class);
        self.version += 1;
        (self.edge_a.len() - 1) as u32
    }
    pub fn node_count(&self) -> usize {
        self.node_x.len()
    }
    pub fn edge_count(&self) -> usize {
        self.edge_a.len()
    }
    /// (Re)builds the adjacency index. Must be called after edits and after loading.
    pub fn finalize(&mut self) {
        let n = self.node_count();
        let mut deg = vec![0u32; n + 1];
        for e in 0..self.edge_count() {
            deg[self.edge_a[e] as usize] += 1;
            deg[self.edge_b[e] as usize] += 1;
        }
        let mut start = vec![0u32; n + 1];
        for i in 0..n {
            start[i + 1] = start[i] + deg[i];
        }
        let mut fill = start.clone();
        let mut adj = vec![(0u32, 0u32); start[n] as usize];
        for e in 0..self.edge_count() {
            let (a, b) = (self.edge_a[e], self.edge_b[e]);
            adj[fill[a as usize] as usize] = (b, e as u32);
            fill[a as usize] += 1;
            adj[fill[b as usize] as usize] = (a, e as u32);
            fill[b as usize] += 1;
        }
        // Deterministic neighbour order: by neighbour id, then edge id.
        for i in 0..n {
            adj[start[i] as usize..start[i + 1] as usize].sort_unstable();
        }
        self.adj_start = start;
        self.adj = adj;
    }
    pub fn neighbours(&self, n: u32) -> &[(u32, u32)] {
        &self.adj[self.adj_start[n as usize] as usize..self.adj_start[n as usize + 1] as usize]
    }
    pub fn edge_cost(&self, e: u32, mode: Mode) -> u32 {
        self.edge_len[e as usize] as u32 * decis_per_tile(mode, self.edge_class[e as usize])
    }
    /// True if every node can reach every other node.
    pub fn is_connected(&self) -> bool {
        let n = self.node_count();
        if n == 0 {
            return true;
        }
        let mut seen = vec![false; n];
        let mut stack = vec![0u32];
        seen[0] = true;
        let mut count = 1;
        while let Some(x) = stack.pop() {
            for &(y, _) in self.neighbours(x) {
                if !seen[y as usize] {
                    seen[y as usize] = true;
                    count += 1;
                    stack.push(y);
                }
            }
        }
        count == n
    }
    /// Removes an edge (swap-remove; edge ids may change). Rebuilds the index.
    pub fn remove_edge(&mut self, e: u32) {
        let e = e as usize;
        self.edge_a.swap_remove(e);
        self.edge_b.swap_remove(e);
        self.edge_len.swap_remove(e);
        self.edge_class.swap_remove(e);
        self.version += 1;
        self.finalize();
    }
    pub fn find_edge(&self, a: u32, b: u32) -> Option<u32> {
        self.neighbours(a)
            .iter()
            .find(|(n, _)| *n == b)
            .map(|(_, e)| *e)
    }
}

/// Lazily filled all-pairs distance tables (per mode, per source node).
#[derive(Clone, Debug, Default)]
pub struct RouteCache {
    version: u32,
    tables: [Vec<Vec<u32>>; 2],
    pub misses: u64,
}

impl RouteCache {
    fn check(&mut self, g: &RoadGraph) {
        if self.version != g.version || self.tables[0].len() != g.node_count() {
            self.version = g.version;
            self.tables = [
                vec![Vec::new(); g.node_count()],
                vec![Vec::new(); g.node_count()],
            ];
        }
    }

    fn table(&mut self, g: &RoadGraph, mode: Mode, src: u32) -> &[u32] {
        self.check(g);
        let slot = &mut self.tables[mode as usize][src as usize];
        if slot.is_empty() {
            self.misses += 1;
            *slot = dijkstra(g, mode, src);
        }
        &self.tables[mode as usize][src as usize]
    }

    /// Travel time between two nodes in deciseconds.
    pub fn travel(&mut self, g: &RoadGraph, mode: Mode, a: u32, b: u32) -> u32 {
        if a == b {
            return 0;
        }
        self.table(g, mode, a)[b as usize]
    }

    /// Node path from `a` to `b` (inclusive). Ties are broken by the lowest node id, so the
    /// result is deterministic.
    pub fn path(&mut self, g: &RoadGraph, mode: Mode, a: u32, b: u32) -> Vec<u32> {
        if a == b {
            return vec![a];
        }
        let dist = self.table(g, mode, a).to_vec();
        if dist[b as usize] >= UNREACHABLE {
            return vec![a, b];
        }
        let mut path = vec![b];
        let mut cur = b;
        while cur != a {
            let mut best = NONE;
            for &(n, e) in g.neighbours(cur) {
                if dist[n as usize] + g.edge_cost(e, mode) == dist[cur as usize] && n < best {
                    best = n;
                }
            }
            if best == NONE {
                break;
            }
            path.push(best);
            cur = best;
        }
        path.reverse();
        path
    }
}

fn dijkstra(g: &RoadGraph, mode: Mode, src: u32) -> Vec<u32> {
    let mut dist = vec![UNREACHABLE; g.node_count()];
    let mut heap = BinaryHeap::new();
    dist[src as usize] = 0;
    heap.push(Reverse((0u32, src)));
    while let Some(Reverse((d, n))) = heap.pop() {
        if d > dist[n as usize] {
            continue;
        }
        for &(m, e) in g.neighbours(n) {
            let nd = d + g.edge_cost(e, mode);
            if nd < dist[m as usize] {
                dist[m as usize] = nd;
                heap.push(Reverse((nd, m)));
            }
        }
    }
    dist
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_routing() {
        let mut g = RoadGraph::default();
        // 3x3 grid
        for y in 0..3 {
            for x in 0..3 {
                g.add_node(x * 10, y * 10);
            }
        }
        for y in 0..3u32 {
            for x in 0..3u32 {
                let i = y * 3 + x;
                if x < 2 {
                    g.add_edge(i, i + 1, 10, RoadClass::Street);
                }
                if y < 2 {
                    g.add_edge(i, i + 3, 10, RoadClass::Street);
                }
            }
        }
        g.finalize();
        assert!(g.is_connected());
        let mut c = RouteCache::default();
        assert_eq!(c.travel(&g, Mode::Car, 0, 8), 40 * 10);
        let p = c.path(&g, Mode::Car, 0, 8);
        assert_eq!(p.first(), Some(&0));
        assert_eq!(p.last(), Some(&8));
        assert_eq!(p.len(), 5);
        assert_eq!(p, c.path(&g, Mode::Car, 0, 8));
    }
}
