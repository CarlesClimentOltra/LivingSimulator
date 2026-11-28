/// <reference lib="webworker" />
// Simulation worker: downloads region checkpoints + message logs, catches up to "now" in
// WASM and then advances one tick every 2.5 s, posting compact state to the main thread.

import init, { Sim, sim_version, content_json } from '../wasm/livingsim_wasm.js';
import wasmUrl from '../wasm/livingsim_wasm_bg.wasm?url';
import {
  AGENT_STRIDE,
  A_ACT,
  A_FROM,
  A_INFO,
  A_TO,
  F_CAR,
  R_TRAVEL,
  routeKey,
  type FromWorker,
  type Manifest,
  type RegionDynamic,
  type RegionStatic,
  type ToWorker,
} from './protocol';
import { tickAt } from './time';

declare const self: DedicatedWorkerGlobalScope;

let sim: Sim;
let manifest: Manifest;
let base = './world/';
const regionIds: number[] = [];
const fetchedLogs = new Set<string>();
const sentRoutes = new Set<string>();
let busy = false;

function post(msg: FromWorker, transfer: Transferable[] = []) {
  self.postMessage(msg, transfer);
}

async function fetchBytes(path: string): Promise<Uint8Array> {
  const res = await fetch(base + path, { cache: 'force-cache' });
  if (!res.ok) throw new Error(`${path}: HTTP ${res.status}`);
  return new Uint8Array(await res.arrayBuffer());
}

async function fetchManifest(): Promise<Manifest> {
  const res = await fetch(base + 'manifest.json', { cache: 'no-store' });
  if (!res.ok) throw new Error(`manifest.json: HTTP ${res.status}`);
  return res.json();
}

function nowTick(): number {
  return tickAt(Date.now(), manifest.epoch_ms);
}

async function ensureLogs(region: number) {
  const tick = sim.tick(region);
  const wanted = manifest.logs.filter((l) => l.region === region && l.to > tick && l.from <= tick + manifest.checkpoint_ticks);
  for (const l of wanted.sort((a, b) => a.from - b.from)) {
    const key = `${manifest.ver}/${region}/${l.from}`;
    if (fetchedLogs.has(key)) continue;
    fetchedLogs.add(key);
    try {
      sim.add_log(await fetchBytes(`${manifest.ver}/msg/r${region}/${l.from}.bin.zst`));
    } catch (e) {
      fetchedLogs.delete(key);
      throw e;
    }
  }
}

async function boot() {
  post({ type: 'progress', label: 'Loading the simulation engine…', done: 0, total: 1 });
  await init({ module_or_path: wasmUrl });
  manifest = await fetchManifest();
  if (manifest.sim_version !== sim_version()) {
    // The page and the world are deployed together; a mismatch means a deploy is in
    // progress (or the browser cached an old page).
    post({ type: 'reload', reason: 'A new version of the world is being published.' });
    return;
  }
  sim = new Sim();
  const now = nowTick();
  const cps = manifest.checkpoints.filter((c) => c.tick <= now);
  const cp = cps.length ? cps[cps.length - 1] : manifest.checkpoints[0];
  let i = 0;
  for (const r of cp.regions) {
    post({ type: 'progress', label: `Downloading ${manifest.regions.find((x) => x.id === r.id)?.name ?? 'region'}…`, done: i++, total: cp.regions.length });
    sim.load_region(await fetchBytes(`${manifest.ver}/cp/${cp.tick}/r${r.id}.bin.zst`));
    regionIds.push(r.id);
    await ensureLogs(r.id);
  }
  // Catch up to now, in slices so we can report progress.
  const start = cp.tick;
  const goal = () => Math.min(Math.floor(nowTick()) + 1, Math.max(...regionIds.map((r) => sim.known_until(r))));
  let target = goal();
  const t0 = performance.now();
  while (true) {
    let behind = false;
    for (const r of regionIds) {
      const tr = Math.min(target, sim.known_until(r));
      if (sim.tick(r) < tr) {
        sim.step_to(r, tr, 120);
        behind = behind || sim.tick(r) < tr;
      }
      await ensureLogs(r);
    }
    const done = Math.min(...regionIds.map((r) => sim.tick(r))) - start;
    post({ type: 'progress', label: 'Catching up with the present…', done, total: Math.max(1, target - start) });
    if (!behind) break;
    target = goal();
    // Let the event loop breathe (progress messages, requests).
    await new Promise((r) => setTimeout(r, 0));
  }
  console.info(`caught up ${target - start} ticks in ${(performance.now() - t0).toFixed(0)} ms`);

  const regions = statics();
  post(
    { type: 'ready', manifest, content: content_json(), regions },
    regions.flatMap((r) => [r.tiles.buffer, r.buildings.buffer, r.roads.buffer]),
  );
  sendTick(false);
  setInterval(loop, 250);
  setInterval(refreshManifest, 5 * 60 * 1000);
}

let lastSent = -1;
let stalled = false;
const staticVersions = new Map<number, number>();

function statics(): RegionStatic[] {
  return regionIds.map((id) => {
    staticVersions.set(id, sim.static_version(id));
    return {
      info: JSON.parse(sim.region_info(id)),
      tiles: sim.tiles(id),
      buildings: sim.buildings(id),
      roads: sim.roads(id),
    };
  });
}

/** Resends the map when construction changed it (routes are recomputed too). */
function checkStatic() {
  if (!regionIds.some((id) => sim.static_version(id) !== staticVersions.get(id))) return;
  const regions = statics();
  sentRoutes.clear();
  post({ type: 'static', regions }, regions.flatMap((r) => [r.tiles.buffer, r.buildings.buffer, r.roads.buffer]));
}

async function loop() {
  if (busy) return;
  busy = true;
  try {
    const target = Math.floor(nowTick()) + 1;
    let advanced = false;
    let catchingUp = false;
    let isStalled = false;
    for (const r of regionIds) {
      await ensureLogs(r);
      const known = sim.known_until(r);
      const tr = Math.min(target, known);
      if (target > known) isStalled = true;
      if (sim.tick(r) < tr) {
        // A backgrounded tab may be far behind: catch up in slices.
        sim.step_to(r, tr, 240);
        advanced = true;
        catchingUp = catchingUp || sim.tick(r) < tr;
      }
    }
    if (isStalled !== stalled) {
      stalled = isStalled;
      post({ type: 'status', stalled, message: stalled ? 'The world runner is late: time is paused until it publishes more of the future.' : '' });
    }
    const t = sim.tick(regionIds[0]);
    if (!catchingUp) checkStatic();
    if (advanced && t !== lastSent) sendTick(catchingUp);
  } catch (e) {
    post({ type: 'error', message: String(e) });
  } finally {
    busy = false;
  }
}

function sendTick(catchingUp: boolean) {
  const regions: RegionDynamic[] = [];
  const routes: { key: string; pts: Int32Array }[] = [];
  for (const id of regionIds) {
    const agents = sim.agents(id);
    const occupancy = sim.occupancy(id);
    regions.push({ id, tick: sim.tick(id), agents, occupancy, sites: sim.sites(id) });
    if (catchingUp) continue;
    for (let i = 0; i < agents.length; i += AGENT_STRIDE) {
      if (agents[i + A_ACT] !== R_TRAVEL) continue;
      const car = (agents[i + A_INFO] & F_CAR) !== 0;
      const key = `${id}/${routeKey(agents[i + A_FROM], agents[i + A_TO], car)}`;
      if (sentRoutes.has(key)) continue;
      sentRoutes.add(key);
      routes.push({ key, pts: sim.route(id, agents[i + A_FROM], agents[i + A_TO], car) });
    }
  }
  lastSent = sim.tick(regionIds[0]);
  post(
    { type: 'tick', regions, routes, catchingUp },
    [...regions.flatMap((r) => [r.agents.buffer, r.occupancy.buffer, r.sites.buffer]), ...routes.map((r) => r.pts.buffer)],
  );
}

async function refreshManifest() {
  try {
    const m = await fetchManifest();
    if (m.sim_version !== manifest.sim_version) {
      post({ type: 'reload', reason: 'The world was updated to a new version.' });
      return;
    }
    manifest = m;
  } catch {
    // Offline or Pages hiccup: try again later.
  }
}

self.onmessage = (ev: MessageEvent<ToWorker>) => {
  const m = ev.data;
  try {
    switch (m.type) {
      case 'init':
        base = m.base;
        boot().catch((e) => post({ type: 'error', message: String(e?.message ?? e) }));
        break;
      case 'card':
        post({ type: 'reply', req: m.req, json: sim.agent_card(m.region, m.agent) });
        break;
      case 'building':
        post({ type: 'reply', req: m.req, json: sim.building_card(m.region, m.building) });
        break;
      case 'stats':
        post({ type: 'reply', req: m.req, json: sim.stats(m.region) });
        break;
      case 'news':
        post({ type: 'reply', req: m.req, json: sim.news(m.region) });
        break;
    }
  } catch (e) {
    post({ type: 'error', message: String(e) });
  }
};
