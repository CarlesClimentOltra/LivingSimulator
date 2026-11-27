// Native vs WASM determinism + region catch-up benchmark, run under Node.
//
//   node tests/wasm/determinism.mjs <wasm-node-pkg-dir> <expected-hash> <seed> <ticks> [world-dir]
//
// 1. Computes the state hash of a fresh world after <ticks> ticks in WASM and compares it
//    with <expected-hash> (computed natively by `livingsim-runner hash`).
// 2. If [world-dir] (runner output) is given, replays every region from its first
//    checkpoint over one simulated day with the message log, checks the published hash and
//    reports the catch-up time (the browser does exactly this).
import { createRequire } from 'node:module';
import { readFileSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';

const require = createRequire(import.meta.url);
const [pkgDir, expected, seed, ticks, worldDir] = process.argv.slice(2);
const wasm = require(resolve(pkgDir, 'livingsim_wasm.js'));

let failed = false;
const t0 = performance.now();
const got = wasm.world_hash(Number(seed), Number(ticks));
const ms = performance.now() - t0;
if (got !== expected) {
  console.error(`MISMATCH native ${expected} vs wasm ${got}`);
  failed = true;
} else {
  console.log(`world hash after ${ticks} ticks matches native: ${got.slice(0, 16)} (${ms.toFixed(0)} ms in WASM)`);
}

const results = [];
if (worldDir) {
  const m = JSON.parse(readFileSync(join(worldDir, 'manifest.json'), 'utf8'));
  const cp = m.checkpoints[0];
  const next = m.checkpoints.find((c) => c.tick === cp.tick + m.checkpoint_ticks);
  if (!next) throw new Error('need two consecutive checkpoints');
  for (const r of cp.regions) {
    const sim = new wasm.Sim();
    sim.load_region(readFileSync(join(worldDir, `${m.ver}/cp/${cp.tick}/r${r.id}.bin.zst`)));
    for (const l of m.logs.filter((l) => l.region === r.id && l.from === cp.tick)) {
      sim.add_log(readFileSync(join(worldDir, `${m.ver}/msg/r${r.id}/${l.from}.bin.zst`)));
    }
    const s = performance.now();
    sim.step_to(r.id, next.tick, 1e9);
    const el = performance.now() - s;
    const want = next.regions.find((x) => x.id === r.id).hash;
    const h = sim.state_hash(r.id);
    const ok = h === want;
    if (!ok) failed = true;
    const pop = m.regions.find((x) => x.id === r.id).population;
    console.log(`region ${r.id} (${pop} inhabitants): 1 simulated day catch-up in ${el.toFixed(0)} ms — ${ok ? 'hash OK' : 'HASH MISMATCH'}`);
    results.push({ name: `WASM catch-up, 1 day, region ${r.id} (${pop} agents)`, unit: 'ms', value: Math.round(el) });
  }
  if (process.env.BENCH_OUT) writeFileSync(process.env.BENCH_OUT, JSON.stringify(results, null, 2));
}
process.exit(failed ? 1 : 0);
