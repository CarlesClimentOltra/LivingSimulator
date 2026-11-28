/// <reference lib="webworker" />
// Simulation worker: downloads region checkpoints + message logs, catches up to "now" in
// WASM and then advances one tick every 2.5 s, posting compact state to the main thread.

import init, { Sim, sim_version, content_json } from '../wasm/livingsim_wasm.js';
import wasmUrl from '../wasm/livingsim_wasm_bg.wasm?url';
import {
  AGENT_STRIDE,
  A_ACT,
  A_PLACE,
  BUILDING_STRIDE,
  INCIDENT_STRIDE,
  PATROL_STRIDE,
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
  contentLite = JSON.parse(content_json());
  // Only the town being watched is simulated; the others are drawn from static files and
  // simulated once the visitor goes there (region streaming, Phase 6).
  const first = manifest.regions.some((r) => r.id === focusId) ? focusId : manifest.regions[0].id;
  await loadLive(first, true);
  await fetchRemote();
  const regions = statics();
  post(
    { type: 'ready', manifest, content: content_json(), regions },
    regions.flatMap((r) => [r.tiles.buffer, r.buildings.buffer, r.roads.buffer]),
  );
  sendTick(false);
  setInterval(loop, 250);
  setInterval(refreshManifest, 5 * 60 * 1000);
}

let focusId = 0;
/** Static data of towns that are not simulated here. */
const remote = new Map<number, { file: string; data: RegionStatic }>();

/** Downloads a town's checkpoint and messages and catches it up with the present. */
async function loadLive(id: number, boot: boolean) {
  if (regionIds.includes(id)) return;
  const name = manifest.regions.find((x) => x.id === id)?.name ?? 'the town';
  const say = (label: string, done: number, total: number) =>
    boot ? post({ type: 'progress', label, done, total }) : post({ type: 'busy', message: label });
  say(`Downloading ${name}…`, 0, 1);
  const now = nowTick();
  const cps = manifest.checkpoints.filter((c) => c.regions.some((r) => r.id === id));
  const before = cps.filter((c) => c.tick <= now);
  const cp = before.length ? before[before.length - 1] : cps[0];
  if (!cp) throw new Error(`no checkpoint for region ${id}`);
  sim.load_region(await fetchBytes(`${manifest.ver}/cp/${cp.tick}/r${id}.bin.zst`));
  await ensureLogs(id);
  const start = cp.tick;
  const t0 = performance.now();
  for (;;) {
    const target = Math.min(Math.floor(nowTick()) + 1, sim.known_until(id));
    if (sim.tick(id) >= target) break;
    sim.step_to(id, target, 120);
    await ensureLogs(id);
    say(`Catching up with the present in ${name}…`, sim.tick(id) - start, Math.max(1, target - start));
    await new Promise((r) => setTimeout(r, 0));
  }
  console.info(`${name}: caught up ${sim.tick(id) - start} ticks in ${(performance.now() - t0).toFixed(0)} ms`);
  regionIds.push(id);
  remote.delete(id);
  if (!boot) post({ type: 'busy', message: null });
}

function b64(s: string): Uint8Array {
  const bin = atob(s);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

/** Static files of the towns not simulated here (refetched when the runner publishes). */
async function fetchRemote(): Promise<boolean> {
  let changed = false;
  for (const r of manifest.regions) {
    if (regionIds.includes(r.id) || !r.static_file) continue;
    if (remote.get(r.id)?.file === r.static_file) continue;
    try {
      const res = await fetch(base + r.static_file, { cache: 'force-cache' });
      if (!res.ok) continue;
      const j = await res.json();
      remote.set(r.id, {
        file: r.static_file,
        data: { info: j.info, tiles: b64(j.tiles), buildings: Uint32Array.from(j.buildings), roads: Uint32Array.from(j.roads), live: false },
      });
      changed = true;
    } catch {
      // Not published yet: drawn later.
    }
  }
  return changed;
}

function postStatics() {
  const regions = statics();
  sentRoutes.clear();
  post({ type: 'static', regions, highways: manifest.highways }, regions.flatMap((r) => [r.tiles.buffer, r.buildings.buffer, r.roads.buffer]));
}

let lastSent = -1;
let stalled = false;
const staticVersions = new Map<number, number>();

// Police patrols (cosmetic: cars drive between the station and places around town).
interface ContentLite {
  buildings: { tags: number[]; roof: string }[];
  tags: string[];
  activities: { kind: string }[];
}
let contentLite: ContentLite;
const patrolInfo = new Map<number, { stations: number[]; targets: number[]; gateway: number; buses: number }>();
const PATROL_LEG = 16;

function hashU(a: number, b: number): number {
  let h = Math.imul(a ^ 0x9e3779b9, 0x85ebca6b) ^ Math.imul(b + 0x632be5ab, 0xc2b2ae35);
  h = Math.imul(h ^ (h >>> 15), 0x2c1b3c6d);
  return (h ^ (h >>> 13)) >>> 0;
}

function statics(): RegionStatic[] {
  const out: RegionStatic[] = [];
  for (const m of manifest.regions) {
    if (regionIds.includes(m.id)) out.push(liveStatic(m.id));
    else {
      const r = remote.get(m.id);
      // Copies: the originals stay here for the next refresh.
      if (r) out.push({ ...r.data, tiles: r.data.tiles.slice(), buildings: r.data.buildings.slice(), roads: r.data.roads.slice() });
    }
  }
  return out;
}

function liveStatic(id: number): RegionStatic {
  {
    staticVersions.set(id, sim.static_version(id));
    const info = JSON.parse(sim.region_info(id));
    const buildings = sim.buildings(id);
    const police = contentLite.tags.indexOf('police');
    const stations: number[] = [];
    const targets: number[] = [];
    for (let b = 0; b * BUILDING_STRIDE < buildings.length; b++) {
      const d = contentLite.buildings[buildings[b * BUILDING_STRIDE]];
      if (police >= 0 && d.tags.includes(police)) stations.push(b);
      if (d.roof !== 'None') targets.push(b);
    }
    const gateway = info.gateways.length ? info.gateways[0].building : -1;
    // Electric buses (Phase 7): one per ~1,200 people.
    const buses = (info.techs ?? []).includes('electric_buses') ? Math.min(10, Math.ceil(info.population / 1200)) : 0;
    patrolInfo.set(id, { stations, targets, gateway, buses });
    return {
      info,
      tiles: sim.tiles(id),
      buildings,
      roads: sim.roads(id),
      live: true,
    };
  }
}

/** Bus legs: buses drive between places all over town, a new leg every 20 ticks. */
function busLegs(id: number, tick: number): Uint32Array {
  const p = patrolInfo.get(id);
  if (!p || !p.buses || !p.targets.length) return new Uint32Array(0);
  const LEG = 20;
  const leg = Math.floor(tick / LEG);
  const at = (bus: number, l: number) => p.targets[hashU(bus * 104729 + id * 13, l) % p.targets.length];
  const out = new Uint32Array(p.buses * PATROL_STRIDE);
  for (let b = 0; b < p.buses; b++) out.set([at(b, leg - 1), at(b, leg), leg * LEG, leg * LEG + LEG - 1], b * PATROL_STRIDE);
  return out;
}

/** Patrol legs for the current tick: cars on duty drive every PATROL_LEG ticks. */
function patrols(id: number, tick: number, agents: Uint32Array, incidents: Uint32Array): Uint32Array {
  const p = patrolInfo.get(id);
  if (!p || !p.targets.length) return new Uint32Array(0);
  const workAct = contentLite.activities.findIndex((a) => a.kind === 'Work');
  let onDuty = 0;
  if (p.stations.length) {
    const st = new Set(p.stations);
    for (let i = 0; i < agents.length; i += AGENT_STRIDE) if (agents[i + A_ACT] === workAct && st.has(agents[i + A_PLACE])) onDuty++;
  }
  const hot: number[] = [];
  for (let i = 0; i < incidents.length; i += INCIDENT_STRIDE) if (tick - incidents[i + 2] < 360 && incidents[i + 3] <= 2) hot.push(incidents[i]);
  // Towns without a station: a car from the city comes while there is something to look into.
  const cars = p.stations.length ? Math.min(8, Math.ceil(onDuty / 4)) : hot.length && p.gateway >= 0 ? 1 : 0;
  const base = p.stations.length ? p.stations : [p.gateway];
  const leg = Math.floor(tick / PATROL_LEG);
  const target = (car: number, l: number) => {
    const h = hashU(car * 7919 + id, l);
    if (l % 4 === 0) return base[car % base.length];
    if (hot.length && h % 3 === 0) return hot[h % hot.length];
    return p.targets[h % p.targets.length];
  };
  const out = new Uint32Array(cars * PATROL_STRIDE);
  for (let c = 0; c < cars; c++) {
    out.set([target(c, leg - 1), target(c, leg), leg * PATROL_LEG, leg * PATROL_LEG + PATROL_LEG - 1], c * PATROL_STRIDE);
  }
  return out;
}

/** Resends the map when construction changed it (routes are recomputed too). */
function checkStatic() {
  if (!regionIds.some((id) => sim.static_version(id) !== staticVersions.get(id))) return;
  postStatics();
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
    const incidents = sim.incidents(id);
    const tick = sim.tick(id);
    const legs = patrols(id, tick, agents, incidents);
    const buses = busLegs(id, tick);
    regions.push({ id, tick, agents, occupancy, sites: sim.sites(id), incidents, patrols: legs, buses });
    if (catchingUp) continue;
    const all = new Uint32Array(legs.length + buses.length);
    all.set(legs);
    all.set(buses, legs.length);
    for (let i = 0; i < all.length; i += PATROL_STRIDE) {
      const legs = all;
      if (legs[i] === legs[i + 1]) continue;
      const key = `${id}/${routeKey(legs[i], legs[i + 1], true)}`;
      if (sentRoutes.has(key)) continue;
      sentRoutes.add(key);
      routes.push({ key, pts: sim.route(id, legs[i], legs[i + 1], true) });
    }
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
    [...regions.flatMap((r) => [r.agents.buffer, r.occupancy.buffer, r.sites.buffer, r.incidents.buffer, r.patrols.buffer, r.buses.buffer]), ...routes.map((r) => r.pts.buffer)],
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
    // New towns, or fresh static data of the towns drawn from files.
    if (await fetchRemote()) postStatics();
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
        focusId = m.focus ?? 0;
        boot().catch((e) => post({ type: 'error', message: String(e?.message ?? e) }));
        break;
      case 'focus':
        if (!regionIds.includes(m.region) && manifest.regions.some((r) => r.id === m.region)) {
          busy = true;
          loadLive(m.region, false)
            .then(() => {
              postStatics();
              sendTick(false);
            })
            .catch((e) => post({ type: 'error', message: String(e?.message ?? e) }))
            .finally(() => {
              busy = false;
            });
        }
        break;
      // Towns not simulated here have no cards or stats yet (null until they are loaded).
      case 'card':
        post({ type: 'reply', req: m.req, json: regionIds.includes(m.region) ? sim.agent_card(m.region, m.agent) : 'null' });
        break;
      case 'building':
        post({ type: 'reply', req: m.req, json: regionIds.includes(m.region) ? sim.building_card(m.region, m.building) : 'null' });
        break;
      case 'stats':
        post({ type: 'reply', req: m.req, json: regionIds.includes(m.region) ? sim.stats(m.region) : 'null' });
        break;
      case 'news':
        post({ type: 'reply', req: m.req, json: regionIds.includes(m.region) ? sim.news(m.region) : 'null' });
        break;
    }
  } catch (e) {
    post({ type: 'error', message: String(e) });
  }
};
