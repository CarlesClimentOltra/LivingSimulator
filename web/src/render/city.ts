// Static city meshes: ground (tile texture), buildings (instanced boxes with procedural
// windows that light up at night), roofs, trees and the highway between towns.

import * as THREE from 'three';
import { BUILDING_STRIDE, INCIDENT_STRIDE, SITE_STRIDE, type RegionStatic } from '../sim/protocol';

export const FLOOR_H = 0.42;
export const BASE_H = 0.15;

export interface BuildingDef {
  id: string;
  name: string;
  floors: number;
  dwellings: number;
  jobs: { profession: number; count: number }[];
  tags: number[];
  color: number;
  roof_color: number;
  roof: 'Flat' | 'Gabled' | 'None';
}

export interface Content {
  crimes?: { id: string; name: string; kind: string; severity: number }[];
  buildings: BuildingDef[];
  tags: string[];
  activities: { id: string; name: string; kind: string; doing: string }[];
}

/** Per region: building data the people renderer needs. */
export interface CityRegion {
  id: number;
  origin: [number, number];
  width: number;
  height: number;
  buildings: Uint32Array;
  /** Building index -> instance index in the body mesh, or -1. */
  bodyIndex: Int32Array;
  outdoor: Uint8Array;
}

function hash(x: number, y: number, s = 0): number {
  let h = (x * 374761393 + y * 668265263 + s * 2147483647) | 0;
  h = Math.imul(h ^ (h >>> 13), 1274126177);
  return ((h ^ (h >>> 16)) >>> 0) / 4294967296;
}

const TILE_COLORS: Record<number, [number, number, number]> = {
  0: [140, 207, 106], // grass
  1: [96, 103, 117], // street
  2: [84, 90, 104], // avenue
  3: [70, 76, 90], // highway
  4: [86, 182, 226], // water
  5: [150, 132, 112], // bridge
  6: [166, 214, 136], // lot / garden
  7: [112, 190, 80], // park
  8: [214, 190, 118], // field
  9: [216, 207, 192], // plaza
  10: [158, 126, 88], // road works (dirt)
  11: [178, 152, 114], // building site
};
const PX = 4;

function isRoad(k: number) {
  return k === 1 || k === 2 || k === 3 || k === 5;
}

function groundTexture(tiles: Uint8Array, w: number, h: number): THREE.DataTexture {
  const data = new Uint8Array(w * PX * h * PX * 4);
  const W = w * PX;
  const at = (x: number, y: number) => (x < 0 || y < 0 || x >= w || y >= h ? 0 : tiles[y * w + x]);
  for (let ty = 0; ty < h; ty++) {
    for (let tx = 0; tx < w; tx++) {
      const k = tiles[ty * w + tx];
      const base = TILE_COLORS[k] ?? TILE_COLORS[0];
      const v = k === 0 || k === 7 ? (hash(tx, ty) - 0.5) * 14 : k === 4 ? (hash(tx, ty, 3) - 0.5) * 8 : 0;
      const road = isRoad(k);
      const horiz = road && isRoad(at(tx - 1, ty)) && isRoad(at(tx + 1, ty));
      const vert = road && isRoad(at(tx, ty - 1)) && isRoad(at(tx, ty + 1));
      for (let py = 0; py < PX; py++) {
        for (let px = 0; px < PX; px++) {
          let [r, g, b] = base;
          if (road) {
            // Sidewalks along edges that border non-road tiles.
            const edge =
              (px === 0 && !isRoad(at(tx - 1, ty))) ||
              (px === PX - 1 && !isRoad(at(tx + 1, ty))) ||
              (py === 0 && !isRoad(at(tx, ty - 1))) ||
              (py === PX - 1 && !isRoad(at(tx, ty + 1)));
            if (edge && k !== 3) [r, g, b] = [201, 195, 184];
            // Dashed centre line on avenues and highways.
            if ((k === 2 || k === 3) && horiz && !vert && (py === 1 || py === 2) && px < 2 && tx % 2 === 0) [r, g, b] = [236, 220, 140];
            if ((k === 2 || k === 3) && vert && !horiz && (px === 1 || px === 2) && py < 2 && ty % 2 === 0) [r, g, b] = [236, 220, 140];
          } else if (k === 8) {
            if (py % 2 === 0) [r, g, b] = [196, 168, 98];
          } else if (k === 10 || k === 11) {
            // Dirt with tyre tracks; sites are fenced (orange site fence on their border).
            if ((px + py + tx) % 3 === 0) [r, g, b] = [r - 14, g - 12, b - 10];
            const fence =
              k === 11 &&
              ((px === 0 && at(tx - 1, ty) !== 11) ||
                (px === PX - 1 && at(tx + 1, ty) !== 11) ||
                (py === 0 && at(tx, ty - 1) !== 11) ||
                (py === PX - 1 && at(tx, ty + 1) !== 11));
            if (fence) [r, g, b] = (px + py) % 2 === 0 ? [236, 120, 40] : [245, 245, 240];
          }
          const i = ((ty * PX + py) * W + tx * PX + px) * 4;
          data[i] = Math.max(0, Math.min(255, r + v));
          data[i + 1] = Math.max(0, Math.min(255, g + v));
          data[i + 2] = Math.max(0, Math.min(255, b + v));
          data[i + 3] = 255;
        }
      }
    }
  }
  const tex = new THREE.DataTexture(data, W, h * PX, THREE.RGBAFormat);
  tex.magFilter = THREE.NearestFilter;
  tex.minFilter = THREE.LinearMipmapLinearFilter;
  tex.generateMipmaps = true;
  tex.colorSpace = THREE.SRGBColorSpace;
  tex.flipY = false;
  tex.anisotropy = 4;
  tex.needsUpdate = true;
  return tex;
}

function groundQuad(w: number, h: number): THREE.BufferGeometry {
  const g = new THREE.BufferGeometry();
  g.setAttribute('position', new THREE.Float32BufferAttribute([0, 0, 0, w, 0, 0, w, 0, h, 0, 0, h], 3));
  g.setAttribute('uv', new THREE.Float32BufferAttribute([0, 0, 1, 0, 1, 1, 0, 1], 2));
  g.setAttribute('normal', new THREE.Float32BufferAttribute([0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0], 3));
  g.setIndex([0, 2, 1, 0, 3, 2]);
  return g;
}

function prism(): THREE.BufferGeometry {
  // Gabled roof: ridge along x, base 1x1 at y=0, height 1.
  const p = [
    // front & back triangles
    -0.5, 0, 0.5, 0.5, 0, 0.5, 0, 1, 0.5,
    0.5, 0, -0.5, -0.5, 0, -0.5, 0, 1, -0.5,
  ];
  const g = new THREE.BufferGeometry();
  const v: number[] = [];
  const tri = (a: number[], b: number[], c: number[]) => v.push(...a, ...b, ...c);
  const A = [-0.5, 0, 0.5], B = [0.5, 0, 0.5], C = [0, 1, 0.5];
  const D = [-0.5, 0, -0.5], E = [0.5, 0, -0.5], F = [0, 1, -0.5];
  void p;
  // Ridge runs along z (front/back are the gables).
  tri(A, B, C);
  tri(E, D, F);
  tri(B, E, F); tri(B, F, C);
  tri(D, A, C); tri(D, C, F);
  g.setAttribute('position', new THREE.Float32BufferAttribute(v, 3));
  g.computeVertexNormals();
  return g;
}

/** Lambert material with procedural windows lit by `aLit` × night. */
function windowMaterial(night: { value: number }): THREE.MeshLambertMaterial {
  const m = new THREE.MeshLambertMaterial({ color: 0xffffff });
  m.onBeforeCompile = (shader) => {
    shader.uniforms.uNight = night;
    shader.vertexShader = shader.vertexShader
      .replace(
        '#include <common>',
        `#include <common>
         attribute float aLit;
         varying float vLit;
         varying vec3 vWPos;
         varying vec3 vWNormal;
         varying float vSeed;`,
      )
      .replace(
        '#include <project_vertex>',
        `#include <project_vertex>
         mat4 lsIM = modelMatrix * instanceMatrix;
         vWPos = (lsIM * vec4(transformed, 1.0)).xyz;
         vWNormal = normalize(mat3(lsIM) * objectNormal);
         vLit = aLit;
         vSeed = float(gl_InstanceID);`,
      );
    shader.fragmentShader = shader.fragmentShader
      .replace(
        '#include <common>',
        `#include <common>
         uniform float uNight;
         varying float vLit;
         varying vec3 vWPos;
         varying vec3 vWNormal;
         varying float vSeed;
         float h21(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }`,
      )
      .replace(
        '#include <opaque_fragment>',
        `#include <opaque_fragment>
         if (vWNormal.y < 0.5 && vWPos.y > ${BASE_H.toFixed(2)} + 0.05) {
           float along = abs(vWNormal.x) > 0.5 ? vWPos.z : vWPos.x;
           float fy = fract((vWPos.y - ${BASE_H.toFixed(2)}) / ${FLOOR_H.toFixed(2)});
           float fx = fract(along * 1.25);
           float win = step(0.3, fy) * step(fy, 0.78) * step(0.22, fx) * step(fx, 0.78);
           vec2 cell = vec2(floor(along * 1.25), floor((vWPos.y - ${BASE_H.toFixed(2)}) / ${FLOOR_H.toFixed(2)}));
           float on = step(h21(cell + vSeed * 0.123), vLit) * uNight;
           vec3 glass = mix(vec3(0.42, 0.58, 0.7), vec3(0.08, 0.1, 0.16), uNight);
           vec3 lit = vec3(1.0, 0.82, 0.45) * (0.85 + 0.3 * h21(cell * 1.7 + vSeed));
           vec3 c = mix(glass, lit, on);
           gl_FragColor.rgb = mix(gl_FragColor.rgb, c, win * 0.9);
         }`,
      );
  };
  return m;
}

export class City {
  readonly group = new THREE.Group();
  readonly regions: CityRegion[] = [];
  bodies!: THREE.InstancedMesh;
  /** body instance -> (region index << 20 | building) */
  bodyOwner: Int32Array = new Int32Array(0);
  private litAttr!: THREE.InstancedBufferAttribute;
  private bodyMat: THREE.MeshLambertMaterial;
  private roofs: THREE.InstancedMesh[] = [];
  /** Floor slabs shown in the cutaway view. */
  private slabs!: THREE.InstancedMesh;
  private slabCentre = new THREE.Vector3(1e9, 0, 1e9);
  private slabRadius = 0;
  private interiors = false;
  private statics: RegionStatic[] = [];
  private expected: Float32Array = new Float32Array(0);
  /** Everything rebuilt when the map changes. */
  private layer = new THREE.Group();
  // Building sites.
  private siteData: Uint32Array[] = [];
  // Incidents: a floating marker for a few hours, police tape around serious ones.
  private incidentData: { arr: Uint32Array; tick: number }[] = [];
  private incidentKey = '';
  private markers!: THREE.InstancedMesh;
  private tape!: THREE.InstancedMesh;
  private siteKey = '';
  siteBodies: THREE.InstancedMesh;
  /** site instance -> (region index << 20 | building) */
  siteOwner: Int32Array;
  private siteRoofsG: THREE.InstancedMesh;
  private siteRoofsF: THREE.InstancedMesh;
  private scaffolds: THREE.InstancedMesh;
  private cranes: THREE.InstancedMesh;

  constructor(private content: Content, night: { value: number }) {
    this.bodyMat = windowMaterial(night);
    this.group.add(this.layer);
    this.markers = new THREE.InstancedMesh(new THREE.OctahedronGeometry(0.45, 0), new THREE.MeshBasicMaterial({ color: 0xffffff }), 256);
    this.tape = new THREE.InstancedMesh(new THREE.BoxGeometry(1, 1, 1), new THREE.MeshBasicMaterial({ color: 0xffd600 }), 1024);
    this.markers.setColorAt(0, new THREE.Color(0xffffff));
    for (const im of [this.markers, this.tape]) {
      im.count = 0;
      im.frustumCulled = false;
      this.group.add(im);
    }
    const N = 512;
    const box = () => new THREE.BoxGeometry(1, 1, 1).translate(0, 0.5, 0);
    const lambert = () => new THREE.MeshLambertMaterial({ color: 0xffffff });
    this.siteBodies = new THREE.InstancedMesh(box(), lambert(), N);
    this.siteOwner = new Int32Array(N);
    this.siteRoofsG = new THREE.InstancedMesh(prism(), lambert(), N);
    this.siteRoofsF = new THREE.InstancedMesh(box(), lambert(), N);
    // Scaffolding: a wire lattice around the structure.
    this.scaffolds = new THREE.InstancedMesh(
      new THREE.BoxGeometry(1, 1, 1, 4, 4, 4).translate(0, 0.5, 0),
      new THREE.MeshBasicMaterial({ color: 0x8d6e63, wireframe: true }),
      N,
    );
    // Tower cranes: mast + jib + counterweight, as unit boxes placed per crane.
    this.cranes = new THREE.InstancedMesh(box(), new THREE.MeshLambertMaterial({ color: 0xffc107 }), N * 3);
    for (const im of [this.siteBodies, this.siteRoofsG, this.siteRoofsF]) im.setColorAt(0, new THREE.Color(0xffffff));
    for (const im of [this.siteBodies, this.siteRoofsG, this.siteRoofsF, this.scaffolds, this.cranes]) {
      im.count = 0;
      im.frustumCulled = false;
      this.group.add(im);
    }
  }

  /** Throws the static meshes away and builds them again from new data. */
  rebuild(statics: RegionStatic[]) {
    this.layer.traverse((o) => {
      const mesh = o as THREE.Mesh;
      if (mesh.geometry) mesh.geometry.dispose();
      const mat = mesh.material as THREE.Material | THREE.Material[] | undefined;
      if (mat && mat !== this.bodyMat) {
        for (const m of Array.isArray(mat) ? mat : [mat]) {
          (m as THREE.MeshLambertMaterial).map?.dispose();
          m.dispose();
        }
      }
    });
    this.layer.clear();
    this.regions.length = 0;
    this.slabCentre.set(1e9, 0, 1e9);
    this.build(statics);
    const interiors = this.interiors;
    this.interiors = !interiors;
    this.setInteriors(interiors);
    this.siteKey = '';
    this.incidentKey = '';
    this.drawIncidents();
    this.drawSites();
  }

  /**
   * Recent incidents (from every tick): a marker floats over the place for six hours, and
   * serious offences are taped off while the case is open (up to a day).
   */
  setIncidents(region: number, arr: Uint32Array, tick: number) {
    if (region < 0) return;
    this.incidentData[region] = { arr, tick };
    // Redraw when the list changes or every simulated hour (markers expire).
    const key = this.incidentData.map((d) => (d ? `${d.arr.join(',')}@${Math.floor(d.tick / 60)}` : '')).join('|');
    if (key === this.incidentKey) return;
    this.incidentKey = key;
    this.drawIncidents();
  }

  private drawIncidents() {
    const m = new THREE.Matrix4();
    const q = new THREE.Quaternion();
    const v = new THREE.Vector3();
    const sc = new THREE.Vector3();
    const col = new THREE.Color();
    const crimes = this.content.crimes ?? [];
    const colours: Record<string, number> = { Shoplifting: 0xffd54f, Burglary: 0xff9800, Assault: 0xf4511e, Fraud: 0xab47bc, Homicide: 0xb71c1c };
    let mi = 0, ti = 0;
    const mCap = 256, tCap = 1024;
    this.incidentData.forEach((d, ri) => {
      const s = this.statics[ri];
      if (!d || !s) return;
      const [ox, oz] = s.info.origin;
      for (let o = 0; o + INCIDENT_STRIDE <= d.arr.length; o += INCIDENT_STRIDE) {
        const [b, crime, tick, status] = [d.arr[o], d.arr[o + 1], d.arr[o + 2], d.arr[o + 3]];
        const age = d.tick - tick;
        const bo = b * BUILDING_STRIDE;
        if (bo + 8 >= s.buildings.length) continue;
        const cd = crimes[crime];
        const def = this.content.buildings[s.buildings[bo]];
        const [x, y, bw, bd] = [s.buildings[bo + 1], s.buildings[bo + 2], s.buildings[bo + 3], s.buildings[bo + 4]];
        const top = def && def.roof !== 'None' ? BASE_H + def.floors * FLOOR_H + 1.2 : 1.4;
        if (age < 360 && mi < mCap) {
          m.compose(v.set(ox + x + bw / 2, top, oz + y + bd / 2), q, sc.set(1, 1.5, 1));
          this.markers.setMatrixAt(mi, m);
          this.markers.setColorAt(mi++, col.setHex(colours[cd?.kind ?? ''] ?? 0xffffff));
        }
        // Tape around the lot of serious offences while the police are on the case.
        if ((cd?.severity ?? 0) >= 4 && age < 1440 && status <= 2 && ti + 4 <= tCap) {
          const [lx, ly, lw, lh] = [s.buildings[bo + 5], s.buildings[bo + 6], s.buildings[bo + 7], s.buildings[bo + 8]];
          const x0 = ox + lx - 0.2, z0 = oz + ly - 0.2, x1 = ox + lx + lw + 0.2, z1 = oz + ly + lh + 0.2;
          const h = 0.32;
          for (const [cx, cz, sx, sz] of [
            [(x0 + x1) / 2, z0, x1 - x0, 0.04],
            [(x0 + x1) / 2, z1, x1 - x0, 0.04],
            [x0, (z0 + z1) / 2, 0.04, z1 - z0],
            [x1, (z0 + z1) / 2, 0.04, z1 - z0],
          ]) {
            m.compose(v.set(cx, h, cz), q, sc.set(sx, 0.06, sz));
            this.tape.setMatrixAt(ti++, m);
          }
        }
      }
    });
    this.markers.count = mi;
    this.tape.count = ti;
    for (const im of [this.markers, this.tape]) {
      im.instanceMatrix.needsUpdate = true;
      if (im.instanceColor) im.instanceColor.needsUpdate = true;
    }
  }

  /** Building sites of a region (from every tick); redrawn only when they change. */
  setSites(region: number, sites: Uint32Array) {
    if (region < 0) return;
    this.siteData[region] = sites;
    const key = this.siteData.map((s) => (s ? s.join(',') : '')).join('|');
    if (key === this.siteKey) return;
    this.siteKey = key;
    this.drawSites();
  }

  private drawSites() {
    const m = new THREE.Matrix4();
    const q = new THREE.Quaternion();
    const col = new THREE.Color();
    const v = new THREE.Vector3();
    const sc = new THREE.Vector3();
    let bi = 0, gi = 0, fi = 0, si = 0, ci = 0;
    const cap = this.siteOwner.length;
    this.siteData.forEach((arr, ri) => {
      const s = this.statics[ri];
      if (!arr || !s) return;
      const [ox, oz] = s.info.origin;
      for (let o = 0; o + SITE_STRIDE <= arr.length && bi < cap; o += SITE_STRIDE) {
        const [b, target, phase, local] = [arr[o], arr[o + 1], arr[o + 2], arr[o + 3] / 1000];
        const bo = b * BUILDING_STRIDE;
        if (bo + 4 >= s.buildings.length) continue;
        const d = this.content.buildings[target];
        if (!d || d.roof === 'None') continue; // parks: only the dirt and the fence
        const [x, y, bw, bd] = [s.buildings[bo + 1], s.buildings[bo + 2], s.buildings[bo + 3], s.buildings[bo + 4]];
        const cx = ox + x + bw / 2;
        const cz = oz + y + bd / 2;
        const full = BASE_H + d.floors * FLOOR_H;
        let h: number;
        if (phase === 1) {
          h = 0.04 + BASE_H * local;
          col.setHex(0x9e9e9e);
        } else if (phase === 2) {
          h = BASE_H + d.floors * FLOOR_H * Math.max(0.12, local);
          col.setHex(0xbdb5a6);
        } else {
          h = full;
          col.setHex(0xbdb5a6).lerp(new THREE.Color(d.color), Math.min(1, local * 1.4));
        }
        m.compose(v.set(cx, 0, cz), q, sc.set(bw * 0.94, h, bd * 0.94));
        this.siteBodies.setMatrixAt(bi, m);
        this.siteBodies.setColorAt(bi, col);
        this.siteOwner[bi++] = (ri << 20) | b;
        if (phase >= 2 && !(phase === 3 && local > 0.85)) {
          m.compose(v.set(cx, 0, cz), q, sc.set(bw * 1.08, h + 0.25, bd * 1.08));
          this.scaffolds.setMatrixAt(si++, m);
        }
        if (phase === 3 && local > 0.5) {
          col.setHex(d.roof_color);
          if (d.roof === 'Gabled') {
            m.compose(v.set(cx, full, cz), q, sc.set(bw * 1.02, Math.min(bw, bd) * 0.45, bd * 1.02));
            this.siteRoofsG.setMatrixAt(gi, m);
            this.siteRoofsG.setColorAt(gi++, col);
          } else {
            m.compose(v.set(cx, full, cz), q, sc.set(bw * 0.97, 0.08, bd * 0.97));
            this.siteRoofsF.setMatrixAt(fi, m);
            this.siteRoofsF.setColorAt(fi++, col);
          }
        }
        // A tower crane next to tall buildings while the structure goes up.
        if (d.floors >= 3 && phase >= 2 && ci + 3 <= cap * 3) {
          const mx = cx + bw / 2 + 0.7;
          const mz = cz - bd / 2 + 0.5;
          const top = full + 1.6;
          const jib = Math.max(bw, bd) + 1.5;
          m.compose(v.set(mx, 0, mz), q, sc.set(0.22, top, 0.22));
          this.cranes.setMatrixAt(ci++, m);
          m.compose(v.set(mx - jib / 2 + 0.6, top, mz), q, sc.set(jib, 0.16, 0.18));
          this.cranes.setMatrixAt(ci++, m);
          m.compose(v.set(mx + 1.0, top - 0.25, mz), q, sc.set(0.5, 0.3, 0.32));
          this.cranes.setMatrixAt(ci++, m);
        }
      }
    });
    const counts: [THREE.InstancedMesh, number][] = [[this.siteBodies, bi], [this.siteRoofsG, gi], [this.siteRoofsF, fi], [this.scaffolds, si], [this.cranes, ci]];
    for (const [im, n] of counts) {
      im.count = n;
      im.instanceMatrix.needsUpdate = true;
      if (im.instanceColor) im.instanceColor.needsUpdate = true;
    }
  }

  build(statics: RegionStatic[]) {
    const c = this.content;
    const tagId = (t: string) => c.tags.indexOf(t);
    const outdoorTags = [tagId('park'), tagId('farm')].filter((x) => x >= 0);
    const siteTag = tagId('site');
    let totalBodies = 0;
    let gabled = 0;
    let flat = 0;
    for (const s of statics) {
      for (let i = 0; i < s.buildings.length; i += BUILDING_STRIDE) {
        const d = c.buildings[s.buildings[i]];
        if (d.roof === 'None') continue;
        totalBodies++;
        if (d.roof === 'Gabled') gabled++;
        else flat++;
      }
    }
    const box = new THREE.BoxGeometry(1, 1, 1).translate(0, 0.5, 0);
    this.bodies = new THREE.InstancedMesh(box, this.bodyMat, totalBodies);
    this.litAttr = new THREE.InstancedBufferAttribute(new Float32Array(totalBodies), 1);
    this.bodies.geometry.setAttribute('aLit', this.litAttr);
    this.bodyOwner = new Int32Array(totalBodies);
    this.expected = new Float32Array(totalBodies);
    const roofsG = new THREE.InstancedMesh(prism(), new THREE.MeshLambertMaterial({ color: 0xffffff }), Math.max(1, gabled));
    const roofsF = new THREE.InstancedMesh(new THREE.BoxGeometry(1, 1, 1).translate(0, 0.5, 0), new THREE.MeshLambertMaterial({ color: 0xffffff }), Math.max(1, flat));
    const m = new THREE.Matrix4();
    const q = new THREE.Quaternion();
    const col = new THREE.Color();
    let bi = 0, gi = 0, fi = 0;
    const trees: [number, number, number][] = [];

    statics.forEach((s, ri) => {
      const { width: w, height: h, origin } = s.info;
      const [ox, oz] = origin;
      // Ground.
      const ground = new THREE.Mesh(groundQuad(w, h), new THREE.MeshLambertMaterial({ map: groundTexture(s.tiles, w, h) }));
      ground.position.set(ox, 0, oz);
      this.layer.add(ground);
      const n = s.buildings.length / BUILDING_STRIDE;
      const region: CityRegion = { id: s.info.id, origin, width: w, height: h, buildings: s.buildings, bodyIndex: new Int32Array(n).fill(-1), outdoor: new Uint8Array(n) };
      for (let b = 0; b < n; b++) {
        const o = b * BUILDING_STRIDE;
        const d = c.buildings[s.buildings[o]];
        const [x, y, bw, bd] = [s.buildings[o + 1], s.buildings[o + 2], s.buildings[o + 3], s.buildings[o + 4]];
        if (d.tags.some((t) => outdoorTags.includes(t))) region.outdoor[b] = 1;
        if (siteTag >= 0 && d.tags.includes(siteTag)) region.outdoor[b] = 2;
        if (d.roof === 'None') continue;
        const height = BASE_H + d.floors * FLOOR_H;
        const cx = ox + x + bw / 2;
        const cz = oz + y + bd / 2;
        m.compose(new THREE.Vector3(cx, 0, cz), q, new THREE.Vector3(bw * 0.94, height, bd * 0.94));
        this.bodies.setMatrixAt(bi, m);
        col.setHex(d.color).offsetHSL(0, 0, (hash(b, ri, 9) - 0.5) * 0.06);
        this.bodies.setColorAt(bi, col);
        this.bodyOwner[bi] = (ri << 20) | b;
        const jobs = d.jobs.reduce((a, j) => a + j.count, 0);
        this.expected[bi] = Math.max(1, d.dwellings * 1.6 + jobs * 0.5);
        region.bodyIndex[b] = bi++;
        col.setHex(d.roof_color);
        if (d.roof === 'Gabled') {
          m.compose(new THREE.Vector3(cx, height, cz), q, new THREE.Vector3(bw * 1.02, Math.min(bw, bd) * 0.45, bd * 1.02));
          roofsG.setMatrixAt(gi, m);
          roofsG.setColorAt(gi++, col);
        } else {
          m.compose(new THREE.Vector3(cx, height, cz), q, new THREE.Vector3(bw * 0.97, 0.08, bd * 0.97));
          roofsF.setMatrixAt(fi, m);
          roofsF.setColorAt(fi++, col);
        }
      }
      // Trees: parks, gardens and countryside.
      for (let ty = 0; ty < h; ty++) {
        for (let tx = 0; tx < w; tx++) {
          const k = s.tiles[ty * w + tx];
          const r = hash(tx, ty, ri + 77);
          const p = k === 7 ? 0.16 : k === 0 ? 0.035 : 0;
          if (r < p) trees.push([ox + tx + 0.5 + (hash(tx, ty, 5) - 0.5) * 0.6, oz + ty + 0.5 + (hash(tx, ty, 6) - 0.5) * 0.6, hash(tx, ty, 8)]);
        }
      }
      this.regions.push(region);
    });
    this.bodies.count = bi;
    roofsG.count = gi;
    roofsF.count = fi;
    for (const im of [this.bodies, roofsG, roofsF]) {
      im.instanceMatrix.needsUpdate = true;
      if (im.instanceColor) im.instanceColor.needsUpdate = true;
      im.computeBoundingSphere();
      this.layer.add(im);
    }
    this.roofs = [roofsG, roofsF];
    this.statics = statics;
    this.slabs = new THREE.InstancedMesh(
      new THREE.BoxGeometry(1, 1, 1),
      new THREE.MeshLambertMaterial({ color: 0xd7c4a3, transparent: true, opacity: 0.38, depthWrite: false }),
      6000,
    );
    this.slabs.count = 0;
    this.slabs.frustumCulled = false;
    this.layer.add(this.slabs);
    this.addTrees(trees);
    this.addHighways(statics);
    this.addLand(statics);
  }

  private addTrees(trees: [number, number, number][]) {
    const crowns = new THREE.InstancedMesh(new THREE.ConeGeometry(0.42, 1.0, 6).translate(0, 0.85, 0), new THREE.MeshLambertMaterial({ color: 0xffffff, flatShading: true }), trees.length);
    const trunks = new THREE.InstancedMesh(new THREE.BoxGeometry(0.12, 0.4, 0.12).translate(0, 0.2, 0), new THREE.MeshLambertMaterial({ color: 0x7a5236 }), trees.length);
    const m = new THREE.Matrix4();
    const col = new THREE.Color();
    const q = new THREE.Quaternion();
    trees.forEach(([x, z, r], i) => {
      const s = 0.75 + r * 0.6;
      m.compose(new THREE.Vector3(x, 0, z), q, new THREE.Vector3(s, s, s));
      crowns.setMatrixAt(i, m);
      trunks.setMatrixAt(i, m);
      col.setHSL(0.27 + r * 0.08, 0.5, 0.32 + r * 0.12);
      crowns.setColorAt(i, col);
    });
    for (const im of [crowns, trunks]) {
      im.instanceMatrix.needsUpdate = true;
      im.computeBoundingSphere();
      this.layer.add(im);
    }
    if (crowns.instanceColor) crowns.instanceColor.needsUpdate = true;
  }

  /** Straight highway strips between neighbouring regions' map edges. */
  private addHighways(statics: RegionStatic[]) {
    const ends = statics.map((s) => {
      const out: { x: number; z: number }[] = [];
      for (let i = 0; i < s.roads.length; i += 5) {
        if (s.roads[i + 4] !== 2) continue;
        for (const [x, y] of [[s.roads[i], s.roads[i + 1]], [s.roads[i + 2], s.roads[i + 3]]]) {
          if (x === 0 || x === s.info.width - 1) out.push({ x: s.info.origin[0] + x + 0.5, z: s.info.origin[1] + y + 0.5 });
        }
      }
      return out;
    });
    const mat = new THREE.MeshLambertMaterial({ color: 0x4a505c });
    for (let i = 0; i + 1 < statics.length; i++) {
      const a = ends[i].sort((p, q) => q.x - p.x)[0];
      const b = ends[i + 1].sort((p, q) => p.x - q.x)[0];
      if (!a || !b) continue;
      const len = Math.hypot(b.x - a.x, b.z - a.z);
      const strip = new THREE.Mesh(new THREE.BoxGeometry(len, 0.02, 1.4), mat);
      strip.position.set((a.x + b.x) / 2, 0.01, (a.z + b.z) / 2);
      strip.rotation.y = -Math.atan2(b.z - a.z, b.x - a.x);
      this.layer.add(strip);
    }
  }

  /**
   * Land around the towns: a large field of farmland, meadows and bare earth with
   * scattered woods, so that the world never looks like it floats in the sky. This land is
   * where towns will expand (Phase 4).
   */
  private addLand(statics: RegionStatic[]) {
    let minX = Infinity, minZ = Infinity, maxX = -Infinity, maxZ = -Infinity;
    for (const s of statics) {
      minX = Math.min(minX, s.info.origin[0]);
      minZ = Math.min(minZ, s.info.origin[1]);
      maxX = Math.max(maxX, s.info.origin[0] + s.info.width);
      maxZ = Math.max(maxZ, s.info.origin[1] + s.info.height);
    }
    const margin = 2500;
    const x0 = minX - margin, z0 = minZ - margin;
    const w = maxX - minX + 2 * margin, d = maxZ - minZ + 2 * margin;
    // A tiling texture of 8x8-tile parcels: meadow, crops, ploughed earth, darker grass.
    const P = 64; // parcels per texture side
    const px = 8; // pixels per parcel
    const data = new Uint8Array(P * px * P * px * 4);
    const palette: [number, number, number][] = [
      [134, 196, 98], [140, 200, 102], [128, 190, 94], [136, 194, 100], [122, 184, 90], [130, 192, 96], // meadows
      [200, 184, 112], [190, 172, 100], // crops
      [160, 128, 88], // ploughed earth
      [112, 170, 82], // rough grass
    ];
    for (let py = 0; py < P; py++) {
      for (let pxl = 0; pxl < P; pxl++) {
        const c = palette[Math.floor(hash(pxl, py, 41) * palette.length)];
        const stripes = c[0] > 140 && c[1] < 190 && hash(pxl, py, 42) < 0.7;
        for (let y = 0; y < px; y++) {
          for (let x = 0; x < px; x++) {
            const i = ((py * px + y) * P * px + pxl * px + x) * 4;
            const edge = x === 0 || y === 0;
            const k = stripes && y % 2 === 0 ? -12 : 0;
            const n = (hash(pxl * px + x, py * px + y, 43) - 0.5) * 5;
            data[i] = Math.max(0, Math.min(255, c[0] + k + n - (edge ? 18 : 0)));
            data[i + 1] = Math.max(0, Math.min(255, c[1] + k + n - (edge ? 10 : 0)));
            data[i + 2] = Math.max(0, Math.min(255, c[2] + k + n - (edge ? 14 : 0)));
            data[i + 3] = 255;
          }
        }
      }
    }
    const tex = new THREE.DataTexture(data, P * px, P * px, THREE.RGBAFormat);
    tex.wrapS = tex.wrapT = THREE.RepeatWrapping;
    tex.magFilter = THREE.NearestFilter;
    tex.minFilter = THREE.LinearMipmapLinearFilter;
    tex.generateMipmaps = true;
    tex.colorSpace = THREE.SRGBColorSpace;
    tex.repeat.set(w / (P * 28), d / (P * 28));
    tex.needsUpdate = true;
    const g = new THREE.PlaneGeometry(w, d).rotateX(-Math.PI / 2);
    const land = new THREE.Mesh(g, new THREE.MeshLambertMaterial({ map: tex }));
    land.position.set(x0 + w / 2, -0.04, z0 + d / 2);
    this.layer.add(land);
    // Woods: clusters of trees in a band around the towns.
    const trees: [number, number, number][] = [];
    for (let i = 0; i < 260; i++) {
      const cx = minX - 400 + hash(i, 1, 91) * (maxX - minX + 800);
      const cz = minZ - 400 + hash(i, 2, 91) * (maxZ - minZ + 800);
      const inside = statics.some((s) => cx > s.info.origin[0] - 6 && cx < s.info.origin[0] + s.info.width + 6 && cz > s.info.origin[1] - 6 && cz < s.info.origin[1] + s.info.height + 6);
      if (inside) continue;
      const n = 8 + Math.floor(hash(i, 3, 91) * 30);
      const r = 4 + hash(i, 4, 91) * 10;
      for (let k = 0; k < n; k++) {
        const a = hash(i, k, 92) * Math.PI * 2;
        const rr = Math.sqrt(hash(i, k, 93)) * r;
        trees.push([cx + Math.cos(a) * rr, cz + Math.sin(a) * rr, hash(i, k, 94)]);
      }
    }
    this.addTrees(trees);
  }

  /** Updates lit windows from occupancy (awake occupants per building). */
  setOccupancy(region: number, occ: Uint32Array) {
    const r = this.regions[region];
    if (!r) return;
    const arr = this.litAttr.array as Float32Array;
    for (let b = 0; b < occ.length; b++) {
      const bi = r.bodyIndex[b];
      if (bi < 0) continue;
      const awake = occ[b] >>> 16;
      arr[bi] = awake === 0 ? 0 : Math.min(1, 0.25 + awake / this.expected[bi]);
    }
    this.litAttr.needsUpdate = true;
  }

  /**
   * Cutaway view when zoomed in: roofs disappear, walls become glass-like and each storey
   * gets a floor slab, so people can be seen on their own floor from above.
   */
  setInteriors(on: boolean) {
    if (on === this.interiors) return;
    this.interiors = on;
    this.bodyMat.transparent = on;
    this.bodyMat.opacity = on ? 0.2 : 1;
    this.bodyMat.depthWrite = !on;
    this.bodyMat.needsUpdate = true;
    for (const r of this.roofs) r.visible = !on;
    this.slabs.visible = on;
    if (!on) this.slabCentre.set(1e9, 0, 1e9);
  }

  /** Rebuilds floor slabs around the camera target when it has moved enough. */
  updateInteriors(centre: THREE.Vector3, radius: number) {
    if (!this.interiors) return;
    if (this.slabCentre.distanceTo(centre) < radius * 0.25 && Math.abs(this.slabRadius - radius) < radius * 0.2) return;
    this.slabCentre.copy(centre);
    this.slabRadius = radius;
    const m = new THREE.Matrix4();
    const q = new THREE.Quaternion();
    const pos = new THREE.Vector3();
    const scale = new THREE.Vector3();
    let n = 0;
    const cap = (this.slabs.instanceMatrix.array as Float32Array).length / 16;
    for (const s of this.statics) {
      const [ox, oz] = s.info.origin;
      for (let o = 0; o < s.buildings.length && n < cap; o += BUILDING_STRIDE) {
        const def = this.content.buildings[s.buildings[o]];
        if (def.roof === 'None') continue;
        const [x, y, bw, bd] = [s.buildings[o + 1], s.buildings[o + 2], s.buildings[o + 3], s.buildings[o + 4]];
        const cx = ox + x + bw / 2;
        const cz = oz + y + bd / 2;
        if (Math.abs(cx - centre.x) > radius || Math.abs(cz - centre.z) > radius) continue;
        for (let f = 0; f < def.floors && n < cap; f++) {
          pos.set(cx, BASE_H + f * FLOOR_H, cz);
          m.compose(pos, q, scale.set(bw * 0.92, 0.035, bd * 0.92));
          this.slabs.setMatrixAt(n++, m);
        }
      }
    }
    this.slabs.count = n;
    this.slabs.instanceMatrix.needsUpdate = true;
  }
}
