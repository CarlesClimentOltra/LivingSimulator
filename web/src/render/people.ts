// People and cars: instanced voxel figures, positions interpolated analytically along
// their trip routes (position = f(route, departure, arrival, now)).

import * as THREE from 'three';
import {
  AGENT_STRIDE,
  A_ACT,
  A_DEPART,
  A_FROM,
  A_HOUSEHOLD,
  A_INFO,
  A_PLACE,
  A_TO,
  A_UNTIL,
  BUILDING_STRIDE,
  F_ALIVE,
  F_CAR,
  NONE,
  R_AWAY,
  R_NONE,
  R_TRAVEL,
  routeKey,
} from '../sim/protocol';
import type { CityRegion, Content } from './city';
import { BASE_H, FLOOR_H } from './city';

interface Route {
  pts: Float32Array; // world x, z pairs
  cum: Float32Array; // cumulative length
}

const SHIRTS = [0xe53935, 0x1e88e5, 0x43a047, 0xfdd835, 0x8e24aa, 0xfb8c00, 0x00acc1, 0xf06292, 0x6d4c41, 0xeeeeee, 0x3949ab, 0x7cb342];
const SKINS = [0xf5d0b0, 0xe0ac80, 0xc68642, 0x8d5524, 0xf1c27d, 0xffdbac];
const CARS = [0xd32f2f, 0x1976d2, 0xfbc02d, 0xffffff, 0x424242, 0x388e3c, 0x7b1fa2, 0xf57c00];

function hash(a: number, b = 0): number {
  let h = Math.imul(a ^ 0x9e3779b9, 0x85ebca6b) ^ Math.imul(b + 0x632be5ab, 0xc2b2ae35);
  h = Math.imul(h ^ (h >>> 15), 0x2c1b3c6d);
  return ((h ^ (h >>> 13)) >>> 0) / 4294967296;
}

export interface Pick {
  region: number;
  agent: number;
}

export class People {
  readonly group = new THREE.Group();
  private routes = new Map<string, Route>();
  private dyn = new Map<number, Uint32Array>();
  private legs: THREE.InstancedMesh;
  private bodies: THREE.InstancedMesh;
  private heads: THREE.InstancedMesh;
  private cars: THREE.InstancedMesh;
  private cabins: THREE.InstancedMesh;
  private points: THREE.Points;
  /** Beds, sofas, TVs and desks for people seen inside buildings. */
  private furniture: THREE.InstancedMesh;
  private pointPos: Float32Array;
  /** body instance -> region << 24 | agent */
  bodyOwner: Int32Array;
  carOwner: Int32Array;
  private capacity: number;
  private kindOf: string[];
  /** Positions of agents by key (for following and the selection marker). */
  readonly lastPos = new Map<number, THREE.Vector3>();
  interiors = false;
  interiorCentre = new THREE.Vector3();
  interiorRadius = 45;

  constructor(private content: Content, private regions: CityRegion[], capacity: number) {
    this.capacity = capacity;
    this.kindOf = content.activities.map((a) => a.kind);
    const mat = new THREE.MeshLambertMaterial({ color: 0xffffff });
    this.legs = new THREE.InstancedMesh(new THREE.BoxGeometry(0.16, 0.16, 0.1).translate(0, 0.08, 0), new THREE.MeshLambertMaterial({ color: 0x37474f }), capacity);
    this.bodies = new THREE.InstancedMesh(new THREE.BoxGeometry(0.2, 0.2, 0.12).translate(0, 0.26, 0), mat, capacity);
    this.heads = new THREE.InstancedMesh(new THREE.BoxGeometry(0.15, 0.14, 0.14).translate(0, 0.44, 0), mat.clone(), capacity);
    this.cars = new THREE.InstancedMesh(new THREE.BoxGeometry(0.42, 0.2, 0.8).translate(0, 0.14, 0), mat.clone(), Math.ceil(capacity / 4));
    this.cabins = new THREE.InstancedMesh(new THREE.BoxGeometry(0.36, 0.16, 0.42).translate(0, 0.31, -0.04), new THREE.MeshLambertMaterial({ color: 0xcfe8f5 }), Math.ceil(capacity / 4));
    this.bodyOwner = new Int32Array(capacity);
    this.carOwner = new Int32Array(Math.ceil(capacity / 4));
    for (const im of [this.legs, this.bodies, this.heads, this.cars, this.cabins]) {
      im.instanceMatrix.setUsage(THREE.DynamicDrawUsage);
      im.frustumCulled = false;
      // Instances move every frame: use a huge bounding sphere so raycasting always tests them.
      im.boundingSphere = new THREE.Sphere(new THREE.Vector3(), 1e7);
      im.count = 0;
      this.group.add(im);
    }
    this.furniture = new THREE.InstancedMesh(new THREE.BoxGeometry(1, 1, 1).translate(0, 0.5, 0), new THREE.MeshLambertMaterial({ color: 0xffffff }), capacity * 2);
    this.furniture.instanceMatrix.setUsage(THREE.DynamicDrawUsage);
    this.furniture.frustumCulled = false;
    this.furniture.count = 0;
    this.group.add(this.furniture);
    this.pointPos = new Float32Array(capacity * 3);
    const pg = new THREE.BufferGeometry();
    pg.setAttribute('position', new THREE.BufferAttribute(this.pointPos, 3).setUsage(THREE.DynamicDrawUsage));
    this.points = new THREE.Points(pg, new THREE.PointsMaterial({ color: 0xfff3c4, size: 3, sizeAttenuation: false }));
    this.points.frustumCulled = false;
    this.group.add(this.points);
  }

  addRoutes(routes: { key: string; pts: Int32Array }[]) {
    for (const r of routes) {
      const region = Number(r.key.split('/')[0]);
      const reg = this.regions.find((x) => x.id === region);
      if (!reg) continue;
      const n = r.pts.length / 2;
      const pts = new Float32Array(n * 2);
      const cum = new Float32Array(n);
      for (let i = 0; i < n; i++) {
        pts[i * 2] = reg.origin[0] + r.pts[i * 2] / 2;
        pts[i * 2 + 1] = reg.origin[1] + r.pts[i * 2 + 1] / 2;
        if (i > 0) cum[i] = cum[i - 1] + Math.hypot(pts[i * 2] - pts[i * 2 - 2], pts[i * 2 + 1] - pts[i * 2 - 1]);
      }
      this.routes.set(r.key, { pts, cum });
    }
  }

  private patrols = new Map<number, Uint32Array>();
  private police!: THREE.InstancedMesh;
  private policeTop!: THREE.InstancedMesh;
  private policeLights!: THREE.InstancedMesh;

  /** Police patrol legs of a region (from the worker, every tick). */
  setPatrols(region: number, legs: Uint32Array) {
    this.patrols.set(region, legs);
  }

  private makePolice() {
    const n = 64;
    this.police = new THREE.InstancedMesh(new THREE.BoxGeometry(0.44, 0.2, 0.84).translate(0, 0.14, 0), new THREE.MeshLambertMaterial({ color: 0xf5f5f5 }), n);
    this.policeTop = new THREE.InstancedMesh(new THREE.BoxGeometry(0.38, 0.16, 0.44).translate(0, 0.31, -0.04), new THREE.MeshLambertMaterial({ color: 0x1a237e }), n);
    this.policeLights = new THREE.InstancedMesh(new THREE.BoxGeometry(0.3, 0.06, 0.1).translate(0, 0.42, -0.04), new THREE.MeshBasicMaterial({ color: 0xffffff }), n);
    for (const im of [this.police, this.policeTop, this.policeLights]) {
      im.instanceMatrix.setUsage(THREE.DynamicDrawUsage);
      im.frustumCulled = false;
      im.count = 0;
      this.group.add(im);
    }
    this.policeLights.setColorAt(0, new THREE.Color(0xffffff));
  }

  /** Police cars along their patrol legs; lights flash on the way to an incident. */
  private updatePolice(t: number, realMs: number) {
    if (!this.police) this.makePolice();
    const m = new THREE.Matrix4();
    const q = new THREE.Quaternion();
    const pos = new THREE.Vector3();
    const up = new THREE.Vector3(0, 1, 0);
    const one = new THREE.Vector3(1, 1, 1);
    const col = new THREE.Color();
    const cap = (this.police.instanceMatrix.array as Float32Array).length / 16;
    let n = 0;
    for (const reg of this.regions) {
      const legs = this.patrols.get(reg.id);
      if (!legs) continue;
      for (let i = 0; i + 3 < legs.length && n < cap; i += 4) {
        const [from, to, dep, arr] = [legs[i], legs[i + 1], legs[i + 2], legs[i + 3]];
        if (from === to) continue;
        const rt = this.routes.get(`${reg.id}/${routeKey(from, to, true)}`);
        if (!rt) continue;
        const f = Math.min(1, Math.max(0, (t - dep) / Math.max(1, arr - dep)));
        const yaw = this.along(rt, f, 0.2, pos);
        q.setFromAxisAngle(up, yaw);
        m.compose(pos, q, one);
        this.police.setMatrixAt(n, m);
        this.policeTop.setMatrixAt(n, m);
        this.policeLights.setMatrixAt(n, m);
        const flash = Math.floor(realMs / 300 + i) % 2 === 0;
        this.policeLights.setColorAt(n, col.setHex(flash ? 0xe53935 : 0x1e88e5));
        n++;
      }
    }
    for (const im of [this.police, this.policeTop, this.policeLights]) {
      im.count = n;
      im.instanceMatrix.needsUpdate = true;
      if (im.instanceColor) im.instanceColor.needsUpdate = true;
    }
  }

  /** Forgets route shapes (the street network or the map changed). */
  clearRoutes() {
    this.routes.clear();
  }

  setAgents(region: number, agents: Uint32Array) {
    this.dyn.set(region, agents);
  }

  agents(region: number): Uint32Array | undefined {
    return this.dyn.get(region);
  }

  /** Position along a route at fraction f, with a lateral offset; writes into out. */
  private along(rt: Route, f: number, side: number, out: THREE.Vector3): number {
    const total = rt.cum[rt.cum.length - 1];
    const d = Math.min(total, Math.max(0, f * total));
    let lo = 0;
    let hi = rt.cum.length - 1;
    while (lo < hi - 1) {
      const mid = (lo + hi) >> 1;
      if (rt.cum[mid] <= d) lo = mid;
      else hi = mid;
    }
    const seg = rt.cum[hi] - rt.cum[lo] || 1;
    const k = (d - rt.cum[lo]) / seg;
    const x0 = rt.pts[lo * 2], z0 = rt.pts[lo * 2 + 1], x1 = rt.pts[hi * 2], z1 = rt.pts[hi * 2 + 1];
    const dx = x1 - x0, dz = z1 - z0;
    const len = Math.hypot(dx, dz) || 1;
    // Keep to the right; no offset on the short door segments at both ends.
    const inner = lo > 0 && hi < rt.cum.length - 1 ? side : 0;
    out.set(x0 + dx * k - (dz / len) * inner, 0, z0 + dz * k + (dx / len) * inner);
    return Math.atan2(dx, dz);
  }

  /**
   * Where someone stands inside (or in the grounds of) a building. Members of a household
   * share a floor and an apartment; everyone else is spread over the floors.
   */
  private buildingSpot(reg: CityRegion, b: number, id: number, household: number, out: THREE.Vector3, floors: number, dwellings: number): number {
    const o = b * BUILDING_STRIDE;
    const outdoor = reg.outdoor[b] !== 0;
    const [x, y, w, d] = outdoor
      ? [reg.buildings[o + 5], reg.buildings[o + 6], reg.buildings[o + 7], reg.buildings[o + 8]]
      : [reg.buildings[o + 1], reg.buildings[o + 2], reg.buildings[o + 3], reg.buildings[o + 4]];
    const home = dwellings > 0 && household !== NONE;
    const key = home ? household : id;
    let fx = 0.2 + hash(key, 1) * 0.6;
    let fz = 0.2 + hash(key, 2) * 0.6;
    if (home) {
      // Family members close together inside their flat.
      fx = Math.min(0.9, Math.max(0.1, fx + (hash(id, 5) - 0.5) * 0.25));
      fz = Math.min(0.9, Math.max(0.1, fz + (hash(id, 6) - 0.5) * 0.25));
    }
    const floor = outdoor ? 0 : Math.floor(hash(key, 3) * floors);
    out.set(reg.origin[0] + x + fx * w, (outdoor ? 0 : BASE_H + 0.035) + floor * FLOOR_H, reg.origin[1] + y + fz * d);
    return Math.floor(hash(key, 4) * 4) * (Math.PI / 2);
  }

  /**
   * Rebuilds instance matrices for time `t` (fractional tick). `zoom` selects the level of
   * detail: points when far, voxel figures when near.
   */
  update(t: number, zoom: number, realMs: number) {
    const m = new THREE.Matrix4();
    const q = new THREE.Quaternion();
    const s = new THREE.Vector3();
    const pos = new THREE.Vector3();
    const up = new THREE.Vector3(0, 1, 0);
    const col = new THREE.Color();
    const far = zoom < 0.9;
    let ni = 0;
    let ci = 0;
    let pi = 0;
    let fi = 0;
    const lying = new THREE.Quaternion().setFromAxisAngle(new THREE.Vector3(1, 0, 0), -Math.PI / 2);
    this.lastPos.clear();
    this.regions.forEach((reg, ri) => {
      const a = this.dyn.get(reg.id);
      if (!a) return;
      for (let i = 0, id = 0; i < a.length; i += AGENT_STRIDE, id++) {
        const act = a[i + A_ACT];
        const info = a[i + A_INFO];
        if (act === R_NONE || act === R_AWAY || (info & F_ALIVE) === 0) continue;
        const age = (info >>> 8) & 0xff;
        const seed = info >>> 16;
        let yaw = 0;
        let pose = 0; // 0 stand/walk, 1 sit, 2 lie
        let car = false;
        let walking = false;
        let onSite = false;
        if (act === R_TRAVEL) {
          car = (info & F_CAR) !== 0;
          const rt = this.routes.get(`${reg.id}/${routeKey(a[i + A_FROM], a[i + A_TO], car)}`);
          if (!rt) continue;
          const dep = a[i + A_DEPART];
          const arr = a[i + A_UNTIL];
          const f = (t - dep) / Math.max(1, arr - dep);
          yaw = this.along(rt, f, car ? 0.2 : 0.4, pos);
          walking = !car;
        } else {
          const place = a[i + A_PLACE];
          if (place === NONE) continue;
          const def = this.content.buildings[reg.buildings[place * BUILDING_STRIDE]];
          const outdoor = reg.outdoor[place] !== 0;
          onSite = reg.outdoor[place] === 2;
          if (!outdoor) {
            if (!this.interiors || far) continue;
            const bx = reg.origin[0] + reg.buildings[place * BUILDING_STRIDE + 1];
            const bz = reg.origin[1] + reg.buildings[place * BUILDING_STRIDE + 2];
            if (Math.abs(bx - this.interiorCentre.x) > this.interiorRadius || Math.abs(bz - this.interiorCentre.z) > this.interiorRadius) continue;
            const kind = this.kindOf[act] ?? '';
            pose = kind === 'Sleep' ? 2 : kind === 'Work' || kind === 'School' || kind === 'Relax' || kind === 'EatHome' || kind === 'EatOut' ? 1 : 0;
          }
          yaw = this.buildingSpot(reg, place, id, a[i + A_HOUSEHOLD], pos, def.floors, def.dwellings);
          if (pose > 0 || (!outdoor && this.interiors)) fi = this.addFurniture(fi, pose, this.kindOf[act] ?? '', pos, yaw, a[i + A_HOUSEHOLD]);
        }
        const key = (ri << 24) | id;
        this.lastPos.set(key, pos.clone());
        if (far) {
          if (pi < this.capacity) {
            this.pointPos[pi * 3] = pos.x;
            this.pointPos[pi * 3 + 1] = pos.y + 0.3;
            this.pointPos[pi * 3 + 2] = pos.z;
            pi++;
          }
          continue;
        }
        if (car) {
          if (ci >= this.carOwner.length) continue;
          q.setFromAxisAngle(up, yaw);
          m.compose(pos, q, s.set(1, 1, 1));
          this.cars.setMatrixAt(ci, m);
          this.cabins.setMatrixAt(ci, m);
          this.cars.setColorAt(ci, col.setHex(CARS[seed % CARS.length]));
          this.carOwner[ci++] = key;
          continue;
        }
        if (ni >= this.capacity) continue;
        const scale = age < 6 ? 0.55 : age < 13 ? 0.72 : age < 16 ? 0.88 : 1;
        if (walking) pos.y += Math.abs(Math.sin(realMs / 110 + id)) * 0.04;
        if (pose === 2) {
          q.setFromAxisAngle(up, yaw).multiply(lying);
          pos.y += 0.2;
        } else {
          q.setFromAxisAngle(up, yaw);
        }
        m.compose(pos, q, s.set(scale, pose === 1 ? scale * 0.78 : scale, scale));
        this.bodies.setMatrixAt(ni, m);
        this.heads.setMatrixAt(ni, m);
        this.legs.setMatrixAt(ni, m);
        // Builders on a site wear high-visibility vests and yellow helmets.
        this.bodies.setColorAt(ni, col.setHex(onSite ? 0xff8f00 : SHIRTS[seed % SHIRTS.length]));
        this.heads.setColorAt(ni, col.setHex(onSite ? 0xffd600 : SKINS[(seed >> 4) % SKINS.length]));
        this.bodyOwner[ni++] = key;
      }
    });
    for (const im of [this.legs, this.bodies, this.heads]) {
      im.count = ni;
      im.instanceMatrix.needsUpdate = true;
      if (im.instanceColor) im.instanceColor.needsUpdate = true;
    }
    for (const im of [this.cars, this.cabins]) {
      im.count = ci;
      im.instanceMatrix.needsUpdate = true;
      if (im.instanceColor) im.instanceColor.needsUpdate = true;
    }
    this.furniture.count = fi;
    this.furniture.instanceMatrix.needsUpdate = true;
    if (this.furniture.instanceColor) this.furniture.instanceColor.needsUpdate = true;
    this.updatePolice(t, realMs);
    this.points.visible = far;
    this.points.geometry.setDrawRange(0, pi);
    (this.points.geometry.getAttribute('position') as THREE.BufferAttribute).needsUpdate = true;
  }

  /** Furniture under a person seen inside: bed, sofa + TV, desk + screen, table. */
  private addFurniture(fi: number, pose: number, kind: string, pos: THREE.Vector3, yaw: number, household: number): number {
    const cap = (this.furniture.instanceMatrix.array as Float32Array).length / 16 - 2;
    if (fi >= cap || pose === 0) return fi;
    const m = new THREE.Matrix4();
    const q = new THREE.Quaternion().setFromAxisAngle(new THREE.Vector3(0, 1, 0), yaw);
    const col = new THREE.Color();
    const put = (dx: number, dz: number, y: number, sx: number, sy: number, sz: number, color: number) => {
      const off = new THREE.Vector3(dx, 0, dz).applyQuaternion(q);
      m.compose(new THREE.Vector3(pos.x + off.x, y, pos.z + off.z), q, new THREE.Vector3(sx, sy, sz));
      this.furniture.setMatrixAt(fi, m);
      this.furniture.setColorAt(fi, col.setHex(color));
      fi++;
    };
    const y0 = pos.y;
    if (kind === 'Sleep') {
      put(0, 0, y0, 0.36, 0.14, 0.62, [0x90caf9, 0xf8bbd0, 0xc5e1a5, 0xffe082][household % 4]);
      put(0, 0.27, y0 + 0.14, 0.3, 0.05, 0.1, 0xffffff); // pillow
    } else if (kind === 'Relax') {
      put(0, -0.04, y0, 0.46, 0.14, 0.2, 0x8d6e63); // sofa
      put(0, 0.42, y0, 0.36, 0.26, 0.05, 0x212121); // TV
    } else if (kind === 'Work' || kind === 'School') {
      put(0, 0.2, y0, 0.44, 0.2, 0.24, 0xa1887f); // desk
      put(0, 0.27, y0 + 0.2, 0.2, 0.13, 0.03, 0x37474f); // screen
    } else {
      put(0, 0.22, y0, 0.32, 0.18, 0.32, 0xbcaaa4); // table
    }
    return fi;
  }

  pick(ray: THREE.Raycaster): Pick | null {
    const hits = ray.intersectObjects([this.bodies, this.heads, this.cars, this.cabins], false);
    for (const h of hits) {
      if (h.instanceId === undefined) continue;
      const owner = h.object === this.cars || h.object === this.cabins ? this.carOwner[h.instanceId] : this.bodyOwner[h.instanceId];
      return { region: this.regions[owner >>> 24].id, agent: owner & 0xffffff };
    }
    return null;
  }

  positionOf(regionIndex: number, agent: number): THREE.Vector3 | undefined {
    return this.lastPos.get((regionIndex << 24) | agent);
  }
}
