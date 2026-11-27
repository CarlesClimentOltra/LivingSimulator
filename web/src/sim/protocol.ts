// Messages between the main thread and the simulation worker.

export interface Manifest {
  manifest_version: number;
  format_version: number;
  sim_version: string;
  ver: string;
  seed: number;
  epoch_ms: number;
  ms_per_tick: number;
  window_ticks: number;
  checkpoint_ticks: number;
  frontier_tick: number;
  checkpoints: { tick: number; regions: { id: number; hash: string; bytes: number }[] }[];
  logs: { region: number; from: number; to: number; bytes: number }[];
  regions: { id: number; name: string; template: string; population: number; origin: [number, number]; size: number }[];
  highways: { a: number; b: number; length_tiles: number }[];
  runner: { finished_at_ms: number; duration_ms: number; simulated_ticks: number; rebased: boolean };
}

export interface RegionInfo {
  id: number;
  name: string;
  width: number;
  height: number;
  origin: [number, number];
  population: number;
  gateways: { building: number; to_region: number; to_name: string; node: number }[];
}

export interface RegionStatic {
  info: RegionInfo;
  tiles: Uint8Array;
  buildings: Uint32Array;
  roads: Uint32Array;
}

export interface RegionDynamic {
  id: number;
  tick: number;
  agents: Uint32Array;
  occupancy: Uint32Array;
}

export type ToWorker =
  | { type: 'init'; base: string }
  | { type: 'card'; region: number; agent: number; req: number }
  | { type: 'building'; region: number; building: number; req: number }
  | { type: 'stats'; region: number; req: number }
  | { type: 'news'; region: number; req: number };

export type FromWorker =
  | { type: 'progress'; label: string; done: number; total: number }
  | { type: 'ready'; manifest: Manifest; content: string; regions: RegionStatic[] }
  | { type: 'tick'; regions: RegionDynamic[]; routes: { key: string; pts: Int32Array }[]; catchingUp: boolean }
  | { type: 'reply'; req: number; json: string }
  | { type: 'status'; stalled: boolean; message: string }
  | { type: 'reload'; reason: string }
  | { type: 'error'; message: string };

/** Agent array layout (see livingsim-core/src/render.rs). */
export const AGENT_STRIDE = 8;
export const A_ACT = 0;
export const A_PLACE = 1;
export const A_FROM = 2;
export const A_TO = 3;
export const A_DEPART = 4;
export const A_UNTIL = 5;
export const A_INFO = 6;
export const A_HOUSEHOLD = 7;
export const R_TRAVEL = 0xfffe;
export const R_AWAY = 0xfffd;
export const R_NONE = 0xffff;
export const F_ALIVE = 1;
export const F_VISITOR = 2;
export const F_FEMALE = 4;
export const F_CAR = 8;
export const BUILDING_STRIDE = 9;
export const NONE = 0xffffffff;

export function routeKey(from: number, to: number, car: boolean): string {
  return `${from}:${to}:${car ? 1 : 0}`;
}
