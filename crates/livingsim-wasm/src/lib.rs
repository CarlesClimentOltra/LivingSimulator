//! WebAssembly bindings. The browser runs these inside a Web Worker: it loads one region
//! checkpoint (or a few) plus the message log, catches up to "now" and keeps stepping in
//! real time. Everything here is a thin layer over `livingsim-core`.

use livingsim_core::snapshot::{decode_log, decode_region, region_hash};
use livingsim_core::{render, RegionReplay, World};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

/// Hex hash of the engine + content build. Must match `manifest.json`'s `sim_version`.
#[wasm_bindgen]
pub fn sim_version() -> String {
    livingsim_core::sim_version().to_string()
}

/// Compiled content (buildings, activities, needs…) as JSON, for colours and labels.
#[wasm_bindgen]
pub fn content_json() -> String {
    serde_json::to_string(livingsim_core::content()).unwrap()
}

/// State hash of a fresh world after `ticks` ticks (native/WASM determinism test).
#[wasm_bindgen]
pub fn world_hash(seed: u32, ticks: u32) -> String {
    let mut w = World::new(seed as u64);
    w.step_to(ticks);
    w.state_hash().to_hex()
}

#[wasm_bindgen]
pub struct Sim {
    regions: Vec<RegionReplay>,
}

impl Sim {
    fn get(&self, id: u16) -> Result<&RegionReplay, JsError> {
        self.regions
            .iter()
            .find(|r| r.region.id == id)
            .ok_or_else(|| JsError::new("region not loaded"))
    }
    fn get_mut(&mut self, id: u16) -> Result<&mut RegionReplay, JsError> {
        self.regions
            .iter_mut()
            .find(|r| r.region.id == id)
            .ok_or_else(|| JsError::new("region not loaded"))
    }
}

#[wasm_bindgen]
impl Sim {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Sim {
        Sim {
            regions: Vec::new(),
        }
    }

    /// Loads a region checkpoint; returns its id. Replaces an already loaded copy.
    pub fn load_region(&mut self, bytes: &[u8]) -> Result<u16, JsError> {
        let r = decode_region(bytes).map_err(|e| JsError::new(&e.to_string()))?;
        let id = r.id;
        self.regions.retain(|x| x.region.id != id);
        self.regions.push(RegionReplay::new(r));
        Ok(id)
    }

    /// Adds a message log for the region it belongs to.
    pub fn add_log(&mut self, bytes: &[u8]) -> Result<(), JsError> {
        let log = decode_log(bytes).map_err(|e| JsError::new(&e.to_string()))?;
        if let Ok(r) = self.get_mut(log.region) {
            r.add_log(log);
        }
        Ok(())
    }

    pub fn tick(&self, region: u16) -> Result<u32, JsError> {
        Ok(self.get(region)?.tick())
    }

    /// Ticks for which inbound messages are known (the replay cannot go further).
    pub fn known_until(&self, region: u16) -> Result<u32, JsError> {
        Ok(self.get(region)?.known_until())
    }

    /// Advances towards `target`, at most `max_ticks` ticks. Returns the tick reached.
    pub fn step_to(&mut self, region: u16, target: u32, max_ticks: u32) -> Result<u32, JsError> {
        let r = self.get_mut(region)?;
        let goal = target.min(r.tick().saturating_add(max_ticks));
        let _ = r.step_to(goal);
        Ok(r.tick())
    }

    pub fn state_hash(&self, region: u16) -> Result<String, JsError> {
        Ok(region_hash(&self.get(region)?.region).to_hex())
    }

    pub fn region_info(&self, region: u16) -> Result<String, JsError> {
        let r = &self.get(region)?.region;
        let gateways: Vec<_> = r
            .gateways
            .iter()
            .map(|g| serde_json::json!({"building": g.building, "to_region": g.to_region, "to_name": g.to_name, "node": g.node}))
            .collect();
        Ok(serde_json::json!({
            "id": r.id, "name": r.name(), "width": r.map.width, "height": r.map.height,
            "origin": [r.origin.0, r.origin.1], "gateways": gateways, "population": r.population(),
        })
        .to_string())
    }

    pub fn tiles(&self, region: u16) -> Result<Vec<u8>, JsError> {
        Ok(self.get(region)?.region.map.kind.clone())
    }

    pub fn buildings(&self, region: u16) -> Result<Vec<u32>, JsError> {
        Ok(render::buildings_packed(&self.get(region)?.region))
    }

    pub fn roads(&self, region: u16) -> Result<Vec<u32>, JsError> {
        Ok(render::roads_packed(&self.get(region)?.region))
    }

    pub fn agents(&self, region: u16) -> Result<Vec<u32>, JsError> {
        Ok(render::agents_packed(&self.get(region)?.region))
    }

    pub fn occupancy(&self, region: u16) -> Result<Vec<u32>, JsError> {
        Ok(render::occupancy(&self.get(region)?.region))
    }

    /// Trip polyline in half-tiles: [x0, y0, x1, y1, …].
    pub fn route(
        &mut self,
        region: u16,
        from: u32,
        to: u32,
        car: bool,
    ) -> Result<Vec<i32>, JsError> {
        Ok(render::route_points(
            &mut self.get_mut(region)?.region,
            from,
            to,
            car,
        ))
    }

    pub fn agent_card(&self, region: u16, agent: u32) -> Result<String, JsError> {
        let r = &self.get(region)?.region;
        Ok(serde_json::to_string(&r.agent_card(agent)).unwrap())
    }

    pub fn building_card(&self, region: u16, b: u32) -> Result<String, JsError> {
        let r = &self.get(region)?.region;
        Ok(serde_json::to_string(&r.building_card(b)).unwrap())
    }

    pub fn stats(&self, region: u16) -> Result<String, JsError> {
        Ok(serde_json::to_string(&self.get(region)?.region.stats()).unwrap())
    }

    pub fn news(&self, region: u16) -> Result<String, JsError> {
        Ok(serde_json::to_string(&self.get(region)?.region.newspaper()).unwrap())
    }
}

impl Default for Sim {
    fn default() -> Self {
        Sim::new()
    }
}
