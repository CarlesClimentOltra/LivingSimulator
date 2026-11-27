// People and cars: instanced voxel figures, positions interpolated analytically along
// their trip routes (position = f(route, departure, arrival, now)).

import * as THREE from 'three';
import {
  AGENT_STRIDE,
  A_ACT,
  A_DEPART,
  A_FROM,
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
import { FLOOR_H } from './city';

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

  private buildingSpot(reg: CityRegion, b: number, id: number, out: THREE.Vector3, floors: number): number {
    const o = b * BUILDING_STRIDE;
    const outdoor = reg.outdoor[b] === 1;
    const [x, y, w, d] = outdoor
      ? [reg.buildings[o + 5], reg.buildings[o + 6], reg.buildings[o + 7], reg.buildings[o + 8]]
      : [reg.buildings[o + 1], reg.buildings[o + 2], reg.buildings[o + 3], reg.buildings[o + 4]];
    const fx = 0.15 + hash(id, 1) * 0.7;
    const fz = 0.15 + hash(id, 2) * 0.7;
    const floor = outdoor ? 0 : Math.floor(hash(id, 3) * floors);
    out.set(reg.origin[0] + x + fx * w, 0.15 + floor * FLOOR_H, reg.origin[1] + y + fz * d);
    return hash(id, 4) * Math.PI * 2;
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
          const outdoor = reg.outdoor[place] === 1;
          if (!outdoor) {
            if (!this.interiors || far) continue;
            const bx = reg.origin[0] + reg.buildings[place * BUILDING_STRIDE + 1];
            const bz = reg.origin[1] + reg.buildings[place * BUILDING_STRIDE + 2];
            if (Math.abs(bx - this.interiorCentre.x) > this.interiorRadius || Math.abs(bz - this.interiorCentre.z) > this.interiorRadius) continue;
            const kind = this.kindOf[act] ?? '';
            pose = kind === 'Sleep' ? 2 : kind === 'Work' || kind === 'School' || kind === 'Relax' || kind === 'EatHome' || kind === 'EatOut' ? 1 : 0;
          }
          yaw = this.buildingSpot(reg, place, id, pos, def.floors);
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
          pos.y += 0.08;
        } else {
          q.setFromAxisAngle(up, yaw);
        }
        m.compose(pos, q, s.set(scale, pose === 1 ? scale * 0.78 : scale, scale));
        this.bodies.setMatrixAt(ni, m);
        this.heads.setMatrixAt(ni, m);
        this.legs.setMatrixAt(ni, m);
        this.bodies.setColorAt(ni, col.setHex(SHIRTS[seed % SHIRTS.length]));
        this.heads.setColorAt(ni, col.setHex(SKINS[(seed >> 4) % SKINS.length]));
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
    this.points.visible = far;
    this.points.geometry.setDrawRange(0, pi);
    (this.points.geometry.getAttribute('position') as THREE.BufferAttribute).needsUpdate = true;
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
