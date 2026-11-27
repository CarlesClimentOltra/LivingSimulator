// Static city meshes: ground (tile texture), buildings (instanced boxes with procedural
// windows that light up at night), roofs, trees and the highway between towns.

import * as THREE from 'three';
import { BUILDING_STRIDE, type RegionStatic } from '../sim/protocol';

export const FLOOR_H = 0.42;
const BASE_H = 0.15;

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
  private expected: Float32Array = new Float32Array(0);

  constructor(private content: Content, night: { value: number }) {
    this.bodyMat = windowMaterial(night);
  }

  build(statics: RegionStatic[]) {
    const c = this.content;
    const tagId = (t: string) => c.tags.indexOf(t);
    const outdoorTags = [tagId('park'), tagId('farm')].filter((x) => x >= 0);
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
      this.group.add(ground);
      const n = s.buildings.length / BUILDING_STRIDE;
      const region: CityRegion = { id: s.info.id, origin, width: w, height: h, buildings: s.buildings, bodyIndex: new Int32Array(n).fill(-1), outdoor: new Uint8Array(n) };
      for (let b = 0; b < n; b++) {
        const o = b * BUILDING_STRIDE;
        const d = c.buildings[s.buildings[o]];
        const [x, y, bw, bd] = [s.buildings[o + 1], s.buildings[o + 2], s.buildings[o + 3], s.buildings[o + 4]];
        if (d.tags.some((t) => outdoorTags.includes(t))) region.outdoor[b] = 1;
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
      this.group.add(im);
    }
    this.addTrees(trees);
    this.addHighways(statics);
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
      this.group.add(im);
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
      this.group.add(strip);
      // Countryside under the highway.
      const grass = new THREE.Mesh(new THREE.BoxGeometry(len, 0.01, 60), new THREE.MeshLambertMaterial({ color: 0x86c463 }));
      grass.position.set(strip.position.x, -0.02, strip.position.z);
      grass.rotation.y = strip.rotation.y;
      this.group.add(grass);
    }
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

  setInteriors(on: boolean) {
    this.bodyMat.transparent = on;
    this.bodyMat.opacity = on ? 0.28 : 1;
    this.bodyMat.depthWrite = !on;
    this.bodyMat.needsUpdate = true;
  }
}
