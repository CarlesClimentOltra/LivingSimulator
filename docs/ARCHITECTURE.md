# Living Simulator — Architecture

> Status: **draft for review** (Phase 0). Nothing here is implemented yet.
> Sections marked **⚠ Decision** describe places where this document deviates from,
> or adds to, the original brief and need explicit approval.

Living Simulator is a small-scale life simulation of a modern city of ~10,000
inhabitants, rendered in the browser in a low-poly/voxel style. Inhabitants live on
their own; visitors only watch. The world is **shared and live**: every visitor sees the
same city at the same moment, and the whole thing costs **nothing** to run (GitHub
Actions + GitHub Pages, no servers, no paid APIs, no generative AI).

The architecture is driven by four hard constraints:

1. **Determinism.** The same seed and the same inputs must produce bit-identical state on
   native x86-64 and on `wasm32`, so a browser can reproduce what the runner computed.
2. **Zero cost.** All compute happens either in a free GitHub Actions job or in the
   visitor's browser. All storage is GitHub Pages (live data) and Release assets (backups).
3. **Scale.** Designed from day 1 for hundreds of towns and 500k–1M inhabitants.
   Cost must scale with *events*, not with *agents × ticks*.
4. **Observe only.** There are no inputs from visitors. The simulation is a pure function
   of `(seed, content data, simulation version, tick)`.

---

## 1. Big picture

```
                         ┌─────────────────────────────────────────────┐
  cron (every ~20 min)   │ GitHub Actions: "tick" workflow              │
  or push to main ──────▶│  1. build livingsim-runner + wasm + web      │
                         │  2. download frontier state from Pages       │
                         │     (fallback: latest Release backup)        │
                         │  3. simulate up to  now + LEAD               │
                         │  4. write checkpoints + message log          │
                         │  5. deploy web + world data to Pages         │
                         │  6. (daily) upload backup to a Release       │
                         └───────────────────────┬─────────────────────┘
                                                 │ static files
                                                 ▼
                     https://<user>.github.io/LivingSimulator/
                       ├── index.html, assets/*.js, livingsim_wasm_bg.wasm
                       └── world/manifest.json, world/<ver>/cp/<tick>/r<id>.bin.zst,
                           world/<ver>/msg/<window>.bin.zst

   Browser ───────────────────────────────────────────────────────────────────
   main thread (Three.js)                     Web Worker (livingsim-wasm)
   ┌──────────────────────────┐   messages    ┌──────────────────────────────┐
   │ camera, LOD, instancing, │◀────────────▶ │ load checkpoint of region(s) │
   │ UI panels (read-only)    │  (diffs, not  │ catch-up to "now" (progress) │
   │ analytic interpolation   │   positions)  │ then step 1 tick / 2.5 s     │
   └──────────────────────────┘               └──────────────────────────────┘
```

### Time

| Concept            | Value                                                   |
|--------------------|---------------------------------------------------------|
| 1 tick             | 1 simulated minute                                      |
| Real duration      | 2.5 s per tick → 1 simulated day = 1 real hour          |
| Current tick       | `tick(now) = floor((now_ms − epoch_ms) / 2500)`         |
| Epoch              | fixed Unix time in ms, stored in `manifest.json`        |
| Calendar           | Fictional: tick 0 = **Year 1, Monday Jan 1, 06:00**. 365-day years, 7-day weeks, no leap years. |

`epoch_ms` is the *only* link between the simulation and the real clock, and it is only
used *outside* `livingsim-core` (by the runner and the browser) to compute a target tick.

---

## 2. Repository layout

```
LivingSimulator/
├── Cargo.toml                 # workspace
├── rust-toolchain.toml        # pinned toolchain (determinism + reproducible builds)
├── crates/
│   ├── livingsim-core/        # pure simulation: no I/O, no clock, no threads, no floats in logic
│   ├── livingsim-content/     # content schemas (serde) + loader/validator for /data
│   ├── livingsim-runner/      # native CLI used by GitHub Actions (rayon, reqwest, fs)
│   ├── livingsim-wasm/        # wasm-bindgen bindings used inside the Web Worker
│   └── livingsim-bench/       # criterion benchmarks + synthetic world generators
├── data/                      # RON content: needs, professions, buildings, names, ...
├── web/                       # TypeScript + Vite + Three.js frontend
├── docs/
└── .github/workflows/         # ci.yml, tick.yml, bench.yml, keepalive.yml
```

`livingsim-core` depends on `livingsim-content` (types only). Content is **compiled into
the binary** (`include_bytes!` of a pre-validated, canonical bincode blob produced by a
build script from `/data/*.ron`). That keeps `livingsim-core` free of I/O and makes the
content hash part of the simulation version (see §9).

---

## 3. World model

### 3.1 Regions and chunks

```
World
├── seed, tick, sim_version
├── regions: Vec<Region>              # one per town/city, RegionId = u16 index
├── intercity: IntercityGraph         # highway graph between region gateway nodes
└── mailbox: Mailbox                  # in-flight inter-region messages
Region
├── id, rng_seed, bounds (in chunks)
├── tiles: chunked grid (64×64 tiles per chunk), land use + building refs
├── roads: RoadGraph (nodes, edges, travel times, version counter)
├── agents: AgentStore (SoA)
├── buildings, households, firms, ... (SoA stores)
├── queue: EventQueue
├── route_cache
├── stats: RegionStats (rolling aggregates for UI/newspaper)
└── outbox: Vec<Message>
```

* **A region is the unit of simulation, parallelism, persistence and download.**
  Regions never read each other's state. They only exchange **messages** (§5).
* Chunks (64×64 tiles, 1 tile ≈ 8 m) are the unit of *rendering* and spatial indexing.
  The initial city is a 512×512-tile region (≈ 4 km × 4 km) split into 8×8 chunks.

### 3.2 Entity storage (structure of arrays)

Hand-rolled SoA stores rather than a generic ECS crate:

* Deterministic by construction (no hash-based archetype maps, no system scheduling).
* Predictable memory layout we can measure against a byte budget.
* Smaller WASM binary.

```rust
pub struct AgentId(u32);          // dense index inside its region
pub struct AgentStore {
    // hot (touched on every decision)
    pub activity:      Vec<Activity>,     // u8 enum
    pub activity_until:Vec<Tick>,         // u32
    pub location:      Vec<PlaceRef>,     // u32 (building or road node)
    pub trip:          Vec<TripId>,       // u32, NONE when not travelling
    pub needs:         Vec<Needs>,        // 7 × u16 + last_update: Tick
    // warm
    pub home:          Vec<BuildingId>,
    pub workplace:     Vec<BuildingId>,   // job or school
    pub schedule:      Vec<ScheduleId>,   // u16 index into content schedules
    pub household:     Vec<HouseholdId>,
    pub money_cents:   Vec<i64>,
    pub traits:        Vec<Traits>,       // packed personality, 8 × u4
    // cold
    pub birth_tick:    Vec<Tick>,
    pub name:          Vec<NameRef>,      // (given: u32, family: u32) into region name pool
    pub sex:           Vec<Sex>,
    pub journal:       JournalStore,      // fixed-size ring buffers, see §3.4
    pub alive:         BitVec,
}
```

IDs are `u32`. Removed agents leave a tombstone; free slots are reused in ascending order
(deterministic). Strings never live per agent: names are indices into a per-region,
append-only name pool (`Vec<u8>` arena).

### 3.3 Fixed-point everything

| Quantity  | Representation                                  |
|-----------|-------------------------------------------------|
| Time      | `Tick(u32)` minutes (≈ 8,000 years of headroom) |
| Needs     | `u16`, 0..=10_000 (0.01 % resolution)           |
| Money     | `i64` cents                                     |
| Positions | tile coords `i32` + sub-tile `u8` (24.8 fixed)  |
| Distances / travel time | `u32` decimetres / seconds        |
| Utilities | `i32` scores; curves are integer piecewise-linear tables from `/data` |

`f32`/`f64` are forbidden in `livingsim-core` (enforced by a `clippy.toml` lint
`disallowed-types` and a CI grep). Floats are fine in the renderer.

### 3.4 Memory budget

Per agent, measured by a test that builds a synthetic region and divides heap usage
(`size_of` × capacity of every column) by agent count:

| Part                         | Budget |
|------------------------------|--------|
| Hot + warm columns           | ≤ 96 B |
| Cold columns                 | ≤ 32 B |
| Journal (16 entries × 12 B)  | 192 B  |
| Share of queue, trips, caches| ≤ 64 B |
| **Total**                    | **≤ 384 B / agent** (1M agents ≈ 384 MB) |

Journal entry: `{ tick: u32, kind: u16, arg: u16, subject: u32 }` (12 B). The UI renders
`kind` + args into text using localisable templates from `/data/journal.ron`.

---

## 4. Simulation core: event-driven, not tick-driven

### 4.1 Event queue

`world.step()` advances one tick, but the cost of a tick is proportional to the number
of **events scheduled for that minute**, not to the number of agents. Each agent has at
most one pending *decision event* ("re-evaluate at 17:00").

* `EventQueue` = a **timing wheel** with 1,440 one-minute buckets (one day) plus an
  overflow binary heap for events further away.
* Determinism: events inside a bucket are processed sorted by
  `(priority_class, entity_kind, entity_id, seq)`. No ordering ever depends on memory
  addresses or hash values.
* Cancelling an event = bumping a per-entity generation counter; stale events are skipped
  when popped (no O(n) removal).

### 4.2 Needs (lazy)

Needs decay linearly with a rate that depends on the current activity. We store the
value at `last_update` and compute `need(t) = clamp(v₀ + rate(activity)·(t − t₀))`
only when somebody looks: at a decision point, a UI query, or a serialization. A
sleeping agent costs nothing for 8 hours.

When an activity starts, the sim computes **analytically** the tick at which the next need
threshold will be crossed (e.g. hunger reaches "critical") and schedules the decision
event at `min(schedule_end, threshold_tick)`.

### 4.3 Decision making (classic AI, no LLM)

Three layers, all data-driven:

1. **Daily schedules** (`/data/schedules.ron`): templates such as *office worker*,
   *shift worker*, *student*, *retiree*, with weekday/weekend variants. They give hard
   blocks ("work 09:00–17:00, ±jitter from personality").
2. **Utility AI** for free time: each candidate action (eat at home, shop, go to the
   park, watch TV, visit friend, sleep…) has integer response curves over needs, distance,
   money and traits. Highest score wins; ties broken by action id; randomness comes from a
   keyed hash (§4.5), never from iteration order.
3. **Multi-step tasks** (Phase 4+): small hierarchical task plans (HTN/GOAP-lite) for
   things like *build a house* = fetch materials → go to site → work N shifts per phase.
   Defined as data, executed by a tiny interpreter.

### 4.4 Simulation level of detail (camera-independent)

LOD is a **simulation rule**, never a rendering decision:

* An agent inside a building with a long activity ("at the office until 17:00") has no
  events in between; the UI derives what they are doing from `(activity, until)`.
* Interiors show sub-activities (typing, meeting, coffee) chosen by a keyed hash of
  `(agent, tick / 15)`: deterministic, free, and never written back into state.
* Long trips are analytic: `position = f(route, depart_tick, t)` (§6.3).

### 4.5 Randomness

* `World::new(seed)` derives per-region seeds with ChaCha8 (`rand_chacha`).
* **Structural** randomness (city generation, migrations) uses a per-region `ChaCha8Rng`
  that advances in a fixed order.
* **Per-decision** randomness uses a stateless keyed hash
  `rand(region_seed, agent_id, tick, purpose) -> u64` (SplitMix64-style mixing). This makes
  results independent of processing order and lets us skip agents lazily without shifting
  any shared RNG stream.

### 4.6 Public API (`livingsim-core`)

```rust
let mut world = World::new(seed, &content);   // generates the initial city (tick 0)
world.step();                                 // +1 tick
world.step_to(target_tick);                   // loop of step(), early-exit friendly
let bytes = world.serialize();                // bincode (fixint) + zstd
let world = World::deserialize(&bytes)?;      // with version check & migrations
let h: StateHash = world.state_hash();        // blake3 over canonical encoding

// Region-level API, used by the browser and by sharded runners:
let mut r = Region::deserialize(&bytes)?;
r.step_window(&inbound_messages);             // advance one exchange window (§5)
let out: Vec<Message> = r.take_outbox();
```

---

## 5. Regions and inter-region messages

### 5.1 Exchange windows

Time is divided into **exchange windows** of `W = 60` ticks (1 simulated hour).

* During window `w`, every region runs independently, using only its own state and the
  inbound messages delivered at the start of `w`.
* Messages emitted during `w` go to the region's outbox. At the end of the window they
  are gathered, sorted by `(dest_region, src_region, seq)` and **delivered at the start
  of window `w + 1`**.
* Therefore regions are independent *within* a window → `rayon` can step them in parallel
  and merge outboxes in region-id order with no effect on determinism.

Message kinds (enum, `/data`-extensible payloads): `Trade { good, qty, price }`,
`Migration { household snapshot }`, `Traveller { agent snapshot, arrival_tick, purpose }`,
`MaterialShipment`, `NewsItem`, `TechDiffusion`, `RoadBuilt { gateway }`.

Agents travelling between cities leave their region as a `Traveller` message and are
re-created in the destination at `arrival_tick` (computed analytically from the intercity
route). Their identity is a `GlobalAgentId = (home_region, local_id, generation)`.

### 5.2 Why this lets the browser simulate a single region

To advance region `R` from tick `a` to tick `b`, a client needs exactly:

1. a checkpoint of `R` at tick `a`, and
2. all messages delivered to `R` in windows `(a, b]`.

The runner publishes (2) as the **message log**. The client therefore runs the identical
`Region::step_window` code on identical inputs and reaches a bit-identical state, without
simulating any other region. CI checks this property (§10).

### 5.3 ⚠ Decision: the runner runs *ahead* of real time

**Problem found in the brief.** If the runner only simulates up to "now", the message log
stops at "now" as well. A browser that wants to show region `R` *after* the last runner
run would need messages that other regions have not produced yet, so it would have to
simulate every region that can send messages to `R`, then their neighbours, and so on.
With a cron that can be hours late, single-region simulation does not work.

**Proposed solution.** Nothing in the simulation depends on the real world, so the
runner can compute the future. Each run continues from the **frontier** (the furthest
tick computed so far) up to `now + LEAD`, with `LEAD = 6 real hours` (= 6 simulated days),
and publishes:

* **checkpoints** of every region every `CP = 1 real hour` (= 1 simulated day) between
  `now − 1 h` and the frontier;
* the **message log** for that whole span;
* the **frontier state** (= the last checkpoint), which the next run resumes from.

The browser picks the latest checkpoint `≤ tick(now)` and catches up **at most one
simulated day** (the 3 s target from the brief), using the logged messages.

Consequences:

* The cron can be up to ~6 hours late (or fail several times) with *zero* visible effect.
  The steady-state compute is the same as without lead: every run simulates ~20 min of
  real time worth of ticks; it is just shifted 6 h into the future.
* If the frontier is ever reached (runner down > 6 h), the browser keeps going by
  simulating the region *closure* it needs (for the MVP with 1–2 regions that is simply
  "all regions"); past a configurable cost it shows a "the world is catching up" notice.
* **The future is public**: someone reading the raw files could know what happens in the
  next hours. For an observation-only world this seems harmless.

### 5.4 Hierarchical pathfinding across regions

* Inside a region: A* on the road graph (§6).
* Between regions: an `IntercityGraph` whose nodes are **gateways** (where a highway
  crosses a region border) and whose edges are highways with precomputed travel times
  (HPA* style). It is replicated in every region snapshot (it is small) and only changes
  through `RoadBuilt` messages, so every region sees the same version at the same window.

---

## 6. Map, roads and movement

### 6.1 Tiles

Each tile stores land use (`u8`), a building or road reference (`u32`) and a height
(`u8`, mostly flat for the MVP). Chunks are serialized independently so later phases can
ship incremental updates per chunk.

### 6.2 Road graph and routing

* Roads are a graph: intersections = nodes, road segments = edges with
  `length_dm`, `speed_class`, `lanes`. Buildings attach to the nearest edge via an
  *access point*.
* A* with an integer admissible heuristic (Manhattan / max-speed). Ties broken by node id.
* **Route cache**: `(from_node, to_node, mode) → RouteId`, an LRU with a fixed number of
  entries. The cache affects only speed, never results (a miss recomputes the same route).
  Every edit to the graph bumps `roads.version`; cached routes from an older version are
  invalidated lazily.
* Routes are interned (`RouteId`) and shared by all agents that use them.

### 6.3 Analytic trips

A trip is `{ route: RouteId, depart: Tick, mode: Walk | Car | Bus(later) }`. The route
stores cumulative travel times per node, so position at any `t` is a binary search +
interpolation. **No per-tick movement**: arrival is a single scheduled event. The renderer
uses the very same function (re-implemented in TS with floats) to animate smoothly at
60 fps between ticks.

Congestion is out of scope for the MVP (free-flow speeds). A later phase can add
deterministic congestion as a per-edge, per-hour travel-time multiplier computed from the
trips planned in the previous window, which keeps trips analytic.

Mode choice: walk if distance < threshold or no car in the household; car otherwise.
Cars park at the destination's access point (no parking simulation for now).

---

## 7. Procedural city generation

`World::new(seed)` builds a modern, already developed city at tick 0:

1. **Terrain & zoning**: a river or coast from noise (integer value noise), downtown near
   the centre, an industrial zone downwind/by the river, residential rings, parks.
2. **Road network**: arterial grid with jitter + local streets per block; one highway
   leaving the map (gateway to future cities).
3. **Lots & buildings**: lots per block, building types from `/data/buildings.ron` chosen by
   zone and density (houses, apartment blocks, offices, shops, factories, farms).
4. **Services**: town hall, police station, jail, hospital, schools (by catchment), fire
   station, university (as content allows).
5. **Firms & jobs**: each non-residential building gets a firm with job slots by profession.
6. **Population**: households generated to match housing capacity and job slots
   (~10,000 inhabitants): ages from a pyramid, couples, children, family names shared,
   jobs matched to age/education, children assigned to the nearest school.
7. **Names**: syllable-based generator (`/data/names/*.ron`), per-language "styles", with
   a blacklist filter so it never emits real-sounding famous names. Babies (Phase 3) get
   names from the same generator biased by the parents' and grandparents' names.

Generation is deterministic and validated by tests (every adult has a home, every
employed adult has a reachable workplace, every child a school, etc.).

---

## 8. Systems roadmap inside the core

Each system is a module under `livingsim-core/src/systems/`, with the same pattern:
data types + content schema + event handlers + stats. Phase 1 implements the first three;
the others exist as **types and stubs** (compiling, no-op or minimal) from Phase 1 on.

| Module          | Phase | Notes |
|-----------------|-------|-------|
| `needs`         | 1 | lazy decay, thresholds |
| `schedule`      | 1 | daily agendas, school, work, weekends |
| `movement`      | 1 | routing, analytic trips |
| `economy`       | 2 | firms, wages, prices, rents, shop stock, farms, taxes, public budget |
| `demography`    | 3 | couples, births, ageing, deaths, retirement, education |
| `construction`  | 4 | demand → projects → construction firms → visible phases |
| `crime`         | 5 | motive model (traits × desperation × conflict), offences |
| `justice`       | 5 | patrols, investigations, arrests, trials, sentences, jail |
| `expansion`     | 6 | new regions/cities founded, highways |
| `technology`    | 7 | tech tree, research points, unlocks |
| `news`          | 1→ | turns notable events into newspaper items (MVP: daily summary) |

Crime and justice are represented in a **non-graphic** way: icons, a police cordon,
newspaper headlines. No blood, no gore.

---

## 9. Persistence, versions and migrations

### 9.1 Snapshot format

```
[ magic "LSIM" | format_version: u16 | sim_version: [u8; 32] | kind: u8 (world/region)
  | region_id: u16 | tick: u32 | state_hash: [u8; 32] ] + zstd(bincode_fixint(state))
```

* `bincode` with **fixed-int encoding** and explicit integer widths (never `usize`, which
  is 64-bit on x86-64 and 32-bit on wasm32).
* `sim_version` = blake3 of `(engine semver, git commit of livingsim-core, content blob
  hash)`. Two builds with the same `sim_version` must produce identical states.

### 9.2 Migrations

* `format_version` changes when the *layout* changes. `livingsim-core::snapshot` keeps
  frozen copies of old layouts (`v1`, `v2`, …) and a chain of `migrate_vN_to_vN+1`.
  Migrations are deterministic and tested with fixture snapshots committed to the repo
  (small, synthetic regions only).
* `sim_version` changes on every engine/content change. A deploy with a new version
  **rebases** the world: the runner takes the latest checkpoint `≤ now`, migrates it, and
  recomputes the future (`now + LEAD`) with the new code. The world history "forks" at the
  deploy instant; open browsers notice the new `sim_version` in `manifest.json` and reload.

### 9.3 Published files (Pages)

```
world/manifest.json
{
  "format_version": 1, "sim_version": "…", "epoch_ms": 1798761600000,
  "frontier_tick": 123456, "window_ticks": 60,
  "regions": [{ "id": 0, "name": "…", "population": 10213, "bbox": […], "center": […] }],
  "checkpoints": [{ "tick": 120000, "hashes": { "0": "…", "1": "…" } }, …],
  "messages": { "from_window": 1990, "to_window": 2057 },
  "runner": { "finished_at": "…", "duration_s": 41 }
}
world/<sim_version>/cp/<tick>/r<region>.bin.zst
world/<sim_version>/msg/<from>-<to>.bin.zst      # grouped per destination region
```

Snapshots are **never committed** to git. They exist only inside the Pages artifact
(replaced on every deploy) and as backups.

### 9.4 Backups

Once per real day the workflow uploads the frontier state as an asset of a rolling Release
(`world-backups`), keeping the last 30 assets. The runner's recovery order is:
Pages frontier → latest Release asset → regenerate from seed (only valid near the epoch).

---

## 10. Determinism contract (what must never break)

1. No I/O, no system clock, no threads, no global state in `livingsim-core`.
2. No `HashMap`/`HashSet` whose iteration order can affect state (use `BTreeMap`,
   `IndexMap` with deterministic insertion, or `Vec`). Allowed only for lookups that never
   iterate (and even then prefer `BTreeMap`).
3. No floats in simulation logic.
4. No `usize` in serialized state or in arithmetic whose overflow could differ between
   32- and 64-bit targets.
5. All sorts have total keys (no reliance on stable/unstable sort details).
6. Parallelism only across regions, merged in region-id order.
7. Randomness only through the seeded RNGs/keyed hash in §4.5.
8. Toolchain pinned in `rust-toolchain.toml`; dependencies pinned via `Cargo.lock`.

**CI checks**

* `determinism-native-wasm`: generate world from a fixed seed, run N ticks natively and
  in `wasm32` (Node), compare `state_hash` at several points; also compare with a committed
  golden hash (updated explicitly by a script when the sim intentionally changes).
* `region-replay`: run a 2-region world for a few days with the full runner; then replay
  one region alone from a checkpoint + message log (native and WASM) and compare hashes.
* `roundtrip`: `deserialize(serialize(w))` has the same hash and continues identically.
* `no-floats` / `no-hashmap` lints on `livingsim-core`.

---

## 11. Runner (`livingsim-runner`) and GitHub Actions

### 11.1 CLI

```
livingsim-runner tick --base-url https://…/world --out dist/world --lead-hours 6
livingsim-runner genesis --seed 42 --epoch-ms … --out dist/world
livingsim-runner verify --checkpoint … --replay-region 0
```

Steps for `tick`: fetch manifest → fetch frontier (or backup) → migrate if needed →
`target = tick(now) + LEAD` → step windows in parallel with rayon → write checkpoints,
message log and manifest → print timings.

### 11.2 Workflows

* `ci.yml` (push/PR): fmt, clippy, tests, determinism tests, wasm build, web build.
* `tick.yml` (cron `*/20 * * * *` + `workflow_dispatch` + push to `main`):
  build everything, run the runner, upload the Pages artifact (web + world), deploy.
  `concurrency: group: pages, cancel-in-progress: false` so two runs never race.
  The web is rebuilt in the same job, so the WASM in the page always matches the data.
* `bench.yml` (push to main, PRs on demand): criterion benches, store results, alert.
* `keepalive.yml`: see below.

### 11.3 The 60-day inactivity problem

GitHub disables scheduled workflows in public repositories after 60 days without
repository activity. Mitigation, in layers:

1. Every `tick.yml` run calls the REST endpoint
   `PUT /repos/{owner}/{repo}/actions/workflows/tick.yml/enable` with `GITHUB_TOKEN`
   (`actions: write`). This is what keepalive actions use in "API mode".
2. A monthly `keepalive.yml` job does the same for all workflows and, as a last resort,
   pushes an empty commit to a dedicated `keepalive` branch if the last commit on `main`
   is older than 50 days.
3. Optional, documented in the README: a free external scheduler (e.g. cron-job.org)
   calling `workflow_dispatch` with a fine-grained token restricted to Actions.
4. Thanks to `LEAD` (§5.3) a disabled workflow is noticed by the client (frontier too
   close) and reported in the UI before visitors are affected.

### 11.4 Resource limits

* Public repos: unlimited minutes on standard runners (4 vCPU / 16 GB on `ubuntu-latest`;
  we plan for 2 cores / 7 GB to be safe). Job limit 6 h; we target < 5 min per run.
* Pages: site ≤ 1 GB, soft limit ~100 GB/month bandwidth and ~10 deploys/hour. With
  a 20-minute cron we deploy 3 times/hour. With ~2–3 MB per visit the bandwidth allows
  tens of thousands of visits per month; beyond that a free CDN mirror can be added.
* **Sharding**: since regions are independent within a window, a future runner can
  split regions across a job matrix that exchanges message logs between windows (or,
  simpler, between "super-windows" of several hours via lookahead). Not needed until the
  world is far beyond 1M agents.

---

## 12. Browser

### 12.1 Stack

TypeScript + Vite + Three.js (WebGL2; WebGPU via `three/webgpu` optional later), the
simulation in WASM inside a **Web Worker**. Agreed with the brief: Bevy-WASM would mean a
much larger download (tens of MB vs ~1–2 MB), a slower first paint, and fewer potential
web contributors; nothing in the rendering needs an ECS on the client.

### 12.2 Boot sequence

1. Load `manifest.json` (no cache) and the region list.
2. Worker loads `livingsim_wasm_bg.wasm`, downloads the latest checkpoint `≤ tick(now)` for
   the region(s) in view, plus the message log for that span.
3. **Catch-up** in the worker with a progress bar (target < 3 s for 1 simulated day of
   10k agents on a mid-range laptop). The rendering starts with the static city meanwhile.
4. Real-time loop: every 2.5 s the worker does `step()` and posts a **diff** (agents whose
   activity/trip changed, buildings changed, new journal entries, stats). It also
   re-checks `tick(now)` so a backgrounded tab catches up on return.
5. Every few minutes the main thread re-reads `manifest.json`; a different `sim_version`
   triggers a reload.

### 12.3 Main-thread ⇄ worker protocol

No per-frame positions over `postMessage`. The worker sends *trip descriptors*
(`agent, route polyline, depart, arrive`) and *activity changes*; the main thread computes
positions analytically per frame. Routes are sent once and cached. All payloads are typed
arrays sent as transferables (GitHub Pages cannot set COOP/COEP headers, so we do not rely
on `SharedArrayBuffer`).

### 12.4 Rendering

* Crossy-Road-like look: flat-shaded low-poly/voxel meshes, a small palette, orthographic
  isometric camera with zoom/pan, soft fake AO, sun/moon cycle driven by the sim clock,
  emissive windows at night for occupied, lit rooms.
* **Instancing** (`InstancedMesh`) per mesh type: people, cars, building parts, trees.
* **Chunk streaming** + frustum culling per chunk.
* **LOD**: far → coloured points/billboards; mid → simple silhouettes; near → animated
  voxel characters (vertex-shader walk cycle). Close zoom makes roofs fade out to show
  interiors (sleeping, TV, desks) derived from activities.
* **World map** zoom level: each region is an icon with stats; distant regions are
  impostors/simplified meshes built from their checkpoint summaries.
* Visitor actions: pan, zoom, follow a person, click to open their card
  (name, age, family, job, needs, money, journal), city stats panel, newspaper, clock and
  calendar. **Nothing modifies the world.**

---

## 13. Performance targets and benchmarks

| Benchmark (CI, `livingsim-bench`)                         | Target |
|-----------------------------------------------------------|--------|
| 1 simulated day, 10k agents, native, 1 core               | < 0.5 s |
| 1 simulated day, 100k agents (10 regions), native, 2 cores | < 5 s |
| 1 simulated day, 1M agents (100 regions), native, 2 cores | < 60 s |
| Memory per agent (all columns + journal + share of queue) | ≤ 384 B |
| Region catch-up, 10k agents, 1 simulated day, WASM (Node)  | < 1.5 s in CI (≈ < 3 s on a mid laptop browser) |
| Region checkpoint size, 10k agents, compressed            | < 2 MB |
| Runner wall time per cron run (20 min real ≈ 8 sim h), 10k agents | < 1 min incl. build cache |

Synthetic worlds for 100k/1M are made by replicating generated regions with different
seeds (no rendering involved). CI stores results with `github-action-benchmark` in a small
`bench-data` branch (JSON only) and comments/alerts when a metric regresses by more than
25 % (wall time, noisy) or 5 % (deterministic counters such as events/day and bytes/agent,
which are also reported).

---

## 14. Content (`/data`)

All content is RON, validated at build time by `livingsim-content` (schemas with
`serde(deny_unknown_fields)`, cross-reference checks, readable error messages):

```
data/
├── needs.ron            # hunger, energy, hygiene, fun, social, money, safety
├── activities.ron       # actions, effects on needs, durations, utility curves
├── schedules.ron        # daily agendas by role
├── professions.ron      # wage ranges, education, workplace types
├── buildings.ron        # footprint, capacity, jobs, zone, mesh id, construction phases
├── goods.ron            # food, consumer goods, materials (Phase 2)
├── crimes.ron           # offences, motives, sentences (Phase 5)
├── technologies.ron     # tech tree (Phase 7)
├── journal.ron          # journal/newspaper text templates
└── names/
    ├── syllables_*.ron  # syllable sets per naming style
    └── blacklist.ron
```

Changing data changes `sim_version` (it is part of the hash), so it is as safe as
changing code.

---

## 15. Open questions

1. ⚠ **Runner runs ahead** (`LEAD = 6 h`) and the consequence that the near future is
   public (§5.3). Alternative: no lead, and clients simulate the neighbour closure.
2. ⚠ **Inter-region latency of one window** (1 sim hour) between emitting and delivering a
   message (§5.1). Intercity trips are not affected (arrival is computed analytically), but
   effects such as prices propagate with a 1-hour delay.
3. Hand-rolled SoA instead of an ECS crate (`bevy_ecs`/`hecs`) — for determinism, memory
   control and WASM size.
4. Free-flow traffic (no congestion) in the MVP.
5. Fictional calendar starting at Year 1.
