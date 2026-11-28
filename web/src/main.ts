// Living Simulator — browser entry point. The visitor can only watch: move the camera,
// follow someone and read their card. Nothing here can change the world.

import * as THREE from 'three';
import './style.css';
import { City, type Content } from './render/city';
import { People } from './render/people';
import { Stage } from './render/scene';
import type { FromWorker, Manifest, RegionStatic, ToWorker } from './sim/protocol';
import { formatClock, formatDate, tickAt } from './sim/time';
import { h, renderAgentCard, renderBuildingCard, renderNews, renderStats, setSiteHandler, type AgentCard, type BuildingCard, type NewsLine, type Stats } from './ui/panels';

const $ = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;

const stage = new Stage($('view'));
const worker = new Worker(new URL('./sim/worker.ts', import.meta.url), { type: 'module' });
let manifest: Manifest;
let city: City;
let people: People;
let statics: RegionStatic[] = [];
let simTick = 0;
let catchingUp = false;
let selected: { region: number; agent: number } | null = null;
let following = false;
let activeRegion = 0;
let reqId = 0;
const pending = new Map<number, (json: string) => void>();

type Request = Extract<ToWorker, { req: number }>;
type DistributiveOmit<T, K extends keyof never> = T extends unknown ? Omit<T, K> : never;

function ask<T>(msg: DistributiveOmit<Request, 'req'>): Promise<T> {
  const req = ++reqId;
  return new Promise((resolve) => {
    pending.set(req, (json) => resolve(JSON.parse(json) as T));
    worker.postMessage({ ...msg, req } as ToWorker);
  });
}

worker.onmessage = (ev: MessageEvent<FromWorker>) => {
  const m = ev.data;
  switch (m.type) {
    case 'progress': {
      $('load-label').textContent = m.label;
      ($('load-bar').firstElementChild as HTMLElement).style.width = `${Math.round((m.done / Math.max(1, m.total)) * 100)}%`;
      break;
    }
    case 'ready':
      try {
        onReady(m.manifest, JSON.parse(m.content), m.regions);
      } catch (e) {
        console.error('onReady failed', e);
        $('load-label').textContent = `Could not start the renderer: ${e}`;
      }
      break;
    case 'tick':
      people?.addRoutes(m.routes);
      for (const r of m.regions) {
        people?.setAgents(r.id, r.agents);
        const ri = statics.findIndex((s) => s.info.id === r.id);
        city?.setOccupancy(ri, r.occupancy);
        city?.setSites(ri, r.sites);
        city?.setIncidents(ri, r.incidents, r.tick);
        people?.setPatrols(r.id, r.patrols);
        simTick = r.tick;
      }
      catchingUp = m.catchingUp;
      break;
    case 'static':
      if (!city) break;
      statics = m.regions;
      if (m.highways) {
        manifest.highways = m.highways;
        city.highways = m.highways;
      }
      city.rebuild(m.regions);
      people.clearRoutes();
      buildRegionButtons();
      buildLabels();
      refreshSide();
      break;
    case 'busy':
      $('notice').textContent = m.message ?? '';
      $('notice').classList.toggle('hidden', !m.message);
      break;
    case 'reply':
      pending.get(m.req)?.(m.json);
      pending.delete(m.req);
      break;
    case 'status':
      $('notice').textContent = m.message;
      $('notice').classList.toggle('hidden', !m.stalled);
      break;
    case 'reload':
      $('load-label').textContent = `${m.reason} Reloading…`;
      setTimeout(() => location.reload(), 4000);
      break;
    case 'error':
      console.error(m.message);
      $('load-label').textContent = `Something went wrong: ${m.message}`;
      break;
  }
};
// "#town=2" opens (and simulates) that town first.
const hashTown = Number(new URLSearchParams(location.hash.slice(1)).get('town') ?? '0') || 0;
worker.postMessage({ type: 'init', base: new URL('./world/', location.href).href, focus: hashTown } satisfies ToWorker);

function onReady(m: Manifest, content: Content, regions: RegionStatic[]) {
  manifest = m;
  statics = regions;
  city = new City(content, stage.night);
  city.highways = m.highways;
  city.build(regions);
  stage.scene.add(city.group);
  const total = m.regions.reduce((a, r) => a + r.population, 0);
  people = new People(content, city.regions, Math.max(4096, Math.ceil(total * 1.3)));
  stage.scene.add(people.group);
  stage.scene.add(marker);
  focusRegion(Math.max(0, statics.findIndex((s) => s.info.id === hashTown)));
  buildRegionButtons();
  buildLabels();
  $('loading').classList.add('hidden');
  refreshSide();
  setInterval(refreshSide, 10_000);
  requestAnimationFrame(frame);
}

function focusRegion(i: number) {
  const r = statics[i];
  if (!r) return;
  activeRegion = i;
  // Towns drawn from files start being simulated here when the visitor goes there.
  if (!r.live) worker.postMessage({ type: 'focus', region: r.info.id } satisfies ToWorker);
  history.replaceState(null, '', r.info.id ? `#town=${r.info.id}` : location.pathname + location.search);
  stage.lookAt(r.info.origin[0] + r.info.width / 2, r.info.origin[1] + r.info.height / 2);
  following = false;
  document.querySelectorAll('#regions button').forEach((b, k) => b.classList.toggle('active', k === i));
  refreshSide();
}

function buildRegionButtons() {
  const el = $('regions');
  el.replaceChildren();
  statics.forEach((s, i) => {
    const b = document.createElement('button');
    b.className = 'btn';
    b.textContent = s.info.name;
    b.addEventListener('click', () => focusRegion(i));
    el.append(b);
  });
  const world = document.createElement('button');
  world.className = 'btn';
  world.textContent = 'World';
  world.addEventListener('click', () => {
    const minX = Math.min(...statics.map((s) => s.info.origin[0]));
    const maxX = Math.max(...statics.map((s) => s.info.origin[0] + s.info.width));
    const minZ = Math.min(...statics.map((s) => s.info.origin[1]));
    const maxZ = Math.max(...statics.map((s) => s.info.origin[1] + s.info.height));
    stage.lookAt((minX + maxX) / 2, (minZ + maxZ) / 2);
    // Fit every town (the camera shows roughly 1/zoom * 120 tiles across).
    stage.camera.zoom = Math.min(0.3, 130 / Math.max(maxX - minX, maxZ - minZ, 1));
    stage.camera.updateProjectionMatrix();
    following = false;
  });
  el.append(world);
}

// ---------------------------------------------------------------- side panel

let sideTab: 'stats' | 'news' = 'stats';
document.querySelectorAll<HTMLButtonElement>('#tabs button').forEach((b) =>
  b.addEventListener('click', () => {
    sideTab = b.dataset.tab as 'stats' | 'news';
    document.querySelectorAll('#tabs button').forEach((x) => x.classList.toggle('active', x === b));
    refreshSide();
  }),
);
$('side-toggle').addEventListener('click', () => $('side').classList.toggle('collapsed'));
if (window.innerWidth < 720) $('side').classList.add('collapsed');

async function refreshSide() {
  if (!statics.length) return;
  const r = statics[activeRegion];
  const region = r.info.id;
  if (!r.live) {
    $('side-body').replaceChildren(h('h2', {}, r.info.name), h('p', { class: 'muted' }, 'Loading this town…'));
    return;
  }
  if (sideTab === 'stats') {
    const s = await ask<Stats | null>({ type: 'stats', region });
    if (s) renderStats($('side-body'), s);
  } else {
    const n = await ask<NewsLine[] | null>({ type: 'news', region });
    if (n) renderNews($('side-body'), r.info.name, n);
  }
}

// ---------------------------------------------------------------- world map labels

/** One label per town, shown when zoomed out: name, people and a few figures. */
const labels: HTMLElement[] = [];
function buildLabels() {
  const layer = $('labels');
  layer.replaceChildren();
  labels.length = 0;
  statics.forEach((s, i) => {
    const m = manifest.regions.find((r) => r.id === s.info.id);
    const sum = m?.summary;
    const parent = sum?.parent != null ? manifest.regions.find((r) => r.id === sum.parent)?.name : null;
    const el = h(
      'button',
      { class: 'town-label' },
      h('strong', {}, s.info.name),
      h('span', {}, `${(m?.population ?? s.info.population).toLocaleString('en-US')} people`),
      sum ? h('span', { class: 'muted' }, `${(sum.unemployment_permille / 10).toFixed(1)}% unemployed · ${sum.under_construction} building sites`) : null,
      parent ? h('span', { class: 'muted' }, `founded by settlers from ${parent}`) : null,
    );
    el.addEventListener('click', () => {
      focusRegion(i);
      stage.camera.zoom = 1;
      stage.camera.updateProjectionMatrix();
    });
    layer.append(el);
    labels.push(el);
  });
}

const tmpV = new THREE.Vector3();
function updateLabels() {
  const show = stage.camera.zoom < 0.45;
  $('labels').classList.toggle('hidden', !show);
  if (!show) return;
  const rect = stage.renderer.domElement.getBoundingClientRect();
  statics.forEach((s, i) => {
    const el = labels[i];
    if (!el) return;
    tmpV.set(s.info.origin[0] + s.info.width / 2, 2, s.info.origin[1] + s.info.height / 2).project(stage.camera);
    el.style.transform = `translate(${((tmpV.x + 1) / 2) * rect.width}px, ${((1 - tmpV.y) / 2) * rect.height}px) translate(-50%, -50%)`;
  });
}

// ---------------------------------------------------------------- cards and picking

const marker = new THREE.Mesh(new THREE.ConeGeometry(0.18, 0.35, 4).rotateX(Math.PI), new THREE.MeshBasicMaterial({ color: 0xffd54f }));
marker.visible = false;

async function openAgent(region: number, agent: number) {
  selected = { region, agent };
  const card = await ask<AgentCard | null>({ type: 'card', region, agent });
  if (!card) return closeCard();
  renderAgentCard($('card-body'), card, (id) => openAgent(region, id), following, () => {
    following = !following;
    openAgent(region, agent);
  });
  $('card').classList.remove('hidden');
}

async function openBuilding(region: number, building: number) {
  selected = null;
  following = false;
  const card = await ask<BuildingCard | null>({ type: 'building', region, building });
  if (!card) return;
  renderBuildingCard($('card-body'), card, (id) => openAgent(region, id));
  $('card').classList.remove('hidden');
}

setSiteHandler((b) => {
  const s = statics[activeRegion];
  if (!s) return;
  openBuilding(s.info.id, b);
  const o = b * 9;
  stage.lookAt(s.info.origin[0] + s.buildings[o + 1] + s.buildings[o + 3] / 2, s.info.origin[1] + s.buildings[o + 2] + s.buildings[o + 4] / 2);
  if (stage.camera.zoom < 2) {
    stage.camera.zoom = 2.5;
    stage.camera.updateProjectionMatrix();
  }
});

function closeCard() {
  selected = null;
  following = false;
  $('card').classList.add('hidden');
}
$('card-close').addEventListener('click', closeCard);
stage.onUserMove = () => {
  following = false;
};

const ray = new THREE.Raycaster();
stage.renderer.domElement.addEventListener('click', (e) => {
  if (!people || stage.wasDrag()) return;
  const rect = stage.renderer.domElement.getBoundingClientRect();
  const ndc = new THREE.Vector2(((e.clientX - rect.left) / rect.width) * 2 - 1, -((e.clientY - rect.top) / rect.height) * 2 + 1);
  ray.setFromCamera(ndc, stage.camera);
  const p = people.pick(ray);
  if (p) {
    openAgent(p.region, p.agent);
    return;
  }
  const hit = ray.intersectObject(city.bodies, false)[0];
  if (hit && hit.instanceId !== undefined) {
    const owner = city.bodyOwner[hit.instanceId];
    openBuilding(statics[owner >>> 20].info.id, owner & 0xfffff);
    return;
  }
  const site = ray.intersectObject(city.siteBodies, false)[0];
  if (site && site.instanceId !== undefined) {
    const owner = city.siteOwner[site.instanceId];
    openBuilding(statics[owner >>> 20].info.id, owner & 0xfffff);
  }
});

setInterval(() => {
  if (selected && !$('card').classList.contains('hidden')) openAgent(selected.region, selected.agent);
}, 2500);

// ---------------------------------------------------------------- frame loop

let interiorsOn = false;
function frame() {
  requestAnimationFrame(frame);
  const now = Date.now();
  const real = tickAt(now, manifest.epoch_ms);
  // The state at simTick covers [simTick - 1, simTick): interpolate inside it.
  const t = Math.min(Math.max(real, simTick - 1), simTick - 0.001);
  stage.setTime(t);
  const zoom = stage.camera.zoom;
  const wantInteriors = zoom > 4;
  if (wantInteriors !== interiorsOn) {
    interiorsOn = wantInteriors;
    city.setInteriors(interiorsOn);
  }
  people.interiors = interiorsOn;
  people.interiorCentre.copy(stage.target);
  people.interiorRadius = 30 / Math.max(1, zoom / 4);
  city.updateInteriors(stage.target, people.interiorRadius + 4);
  people.update(t, zoom, now);
  if (selected) {
    const ri = statics.findIndex((s) => s.info.id === selected!.region);
    const pos = people.positionOf(ri, selected.agent);
    marker.visible = !!pos;
    if (pos) {
      marker.position.set(pos.x, pos.y + 0.9 + Math.sin(now / 250) * 0.06, pos.z);
      if (following) stage.target.lerp(new THREE.Vector3(pos.x, 0, pos.z), 0.08);
    }
  } else {
    marker.visible = false;
  }
  updateLabels();
  $('time').textContent = formatClock(t);
  $('date').textContent = formatDate(t) + (catchingUp ? ' · catching up…' : '');
  stage.render();
}
