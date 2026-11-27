# Living Simulator — Roadmap

> Status: **approved**. Phase 0 is done, and Phase 1 is implemented (milestones 1–10 below).
> Every phase ends with something **visible at the public URL**. Phases are ordered so
> that each one builds on systems that already run live, and scalability work is spread
> across all of them instead of being left for the end.

Legend: 🎯 = what a visitor can see at the end of the phase.

---

## Phase 0 — Design (this document)

- `docs/ARCHITECTURE.md`, `docs/ROADMAP.md`, review and approval.

---

## Phase 1 — MVP: a living city you can watch

🎯 *Open the URL and see a procedurally generated city of ~10,000 people going from home
to work or school, shopping, going out and home to sleep, with a day/night cycle and lit
windows. Click anybody to read their card and journal. The city is the same for everybody
and keeps living when nobody is watching.*

Milestones (each one is a handful of small commits):

1. **Workspace & project files**
   - Cargo workspace (`livingsim-core`, `-content`, `-runner`, `-wasm`, `-bench`),
     pinned toolchain, lints (no floats / no `HashMap` in core).
   - `README.md`, `README.es.md`, `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`,
     `LICENSE-MIT`, `LICENSE-APACHE`, issue & PR templates, `docs/GOOD_FIRST_ISSUES.md`.
   - `ci.yml`: fmt, clippy, test, wasm build.
2. **Core foundations**
   - Fixed-point types, `Tick`, keyed hash RNG, ChaCha8 seeding.
   - SoA stores, timing-wheel event queue, `World`/`Region` skeletons.
   - Serialization (bincode fixint + zstd), snapshot header, `state_hash`, migration
     scaffolding (`v1`).
   - Content pipeline: RON → validation → canonical blob embedded in core.
3. **City generation**
   - Terrain/zoning, road network, lots and buildings, services, firms and job slots.
   - Households and population (~10k) with coherent homes, jobs, schools, families.
   - Syllable name generator + blacklist.
   - Invariant tests (everyone housed, jobs reachable, etc.).
4. **Life loop**
   - Lazy needs, daily schedules (weekday/weekend, work, school, retirees),
     utility AI for free time (eat, shop, leisure, TV, social, sleep).
   - Road graph, A* with route cache, analytic walk/car trips.
   - Journal ring buffer + templates; daily news summary (stub newspaper).
5. **Regions & messages (from day 1)**
   - Exchange windows, outbox/mailbox, message log.
   - A second small region (a village connected by the highway) exchanging
     `Traveller` and simple `Trade` messages, so the mechanism runs for real.
   - Intercity graph with gateways (2 nodes for now).
6. **Determinism tests**
   - Native vs WASM hash (Node), golden hash, serialize round-trip,
     single-region replay from checkpoint + message log.
7. **Runner & live pipeline**
   - `livingsim-runner genesis | tick | verify`, rayon over regions, `LEAD` ahead of now.
   - `tick.yml`: cron + deploy to Pages with web + world data; daily Release backup;
     keepalive (API re-enable + fallback).
8. **Frontend**
   - Vite + TS + Three.js, worker + WASM, boot/catch-up progress bar.
   - Voxel/low-poly meshes, instanced people/cars/buildings, chunk culling, basic LOD
     (points far, simple figures near), day/night with lit windows, roofs fade on zoom.
   - Person card (name, age, family, job, needs, money, journal), follow camera, clock &
     calendar, city stats panel, newspaper stub.
   - Works with WebGL2 on laptop and mobile (touch pan/zoom).
9. **Benchmarks**
   - criterion: cost per simulated day for 10k / 100k / 1M synthetic agents, memory per
     agent, WASM region catch-up time; `bench.yml` with regression alerts.
10. **Stubs for later phases** — compiling types/modules and `/data` schemas for economy,
    demography, construction, crime & justice, expansion, technology.

**Exit criteria:** live URL works on desktop and mobile; catch-up of 1 simulated day
< 3 s on a mid laptop; determinism CI green; benchmarks published; cron ran unattended
for 48 h.

---

## Phase 2 — Economy

🎯 *Shops visibly run out of stock and get restocked by trucks, prices and rents show up
in the stats panel, farms and allotments produce food, the town budget pays for services,
the newspaper reports on prices and unemployment.*

- Firms with payroll, revenue, inventory; wages by profession.
- Goods and simple supply chains (farm → warehouse → shop); trucks as analytic trips.
- Household budgets: rent, food, leisure; the `money` need gets meaning.
- Prices adjusting to stock/demand with integer rules; rent by location/demand.
- Municipal taxes and public spending (police, schools, hospital, maintenance).
- Unemployment, job search, firing/hiring; bankruptcy and new firms.
- Inter-region `Trade` with real goods and prices.
- Stats panel: GDP-like indicators, prices, unemployment, budget charts.

## Phase 3 — Life cycle

🎯 *Couples form and move in together, babies are born (and you can read why their name
was chosen), children grow up, go to school and later university, people retire and
eventually die. Family trees appear in person cards. Births and deaths are in the paper.*

- Relationships graph (friends, partners) with compact storage.
- Partnering, moving in, household splitting/merging.
- Pregnancy and births; naming influenced by family names.
- Ageing, age-dependent schedules, education levels, graduation, retirement.
- Natural deaths with an age-based hazard model; funerals; inheritance.
- Grief/mourning effects on needs.
- Migration between regions (`Migration` messages) driven by housing/jobs.

## Phase 4 — Construction and growth

🎯 *Building sites appear where housing, jobs or shops are lacking; workers with helmets
build them phase by phase (foundation → structure → roof → finished), new roads and
allotments get built. The city visibly grows over days.*

- Demand model (housing shortage, unemployment, lack of shops/services, food).
- Planning: zoning decisions, lot selection, project queue.
- Construction firms, worker crews, material deliveries, multi-step task plans.
- Visible construction phases (data-defined meshes per phase).
- Road building with route-cache invalidation and road-graph versioning.
- Incremental per-chunk snapshot updates.

## Phase 5 — Crime and justice

🎯 *Police cars patrol, occasional incidents are shown with icons and police tape,
arrests happen, trials are reported in the newspaper and convicts serve time in the
jail. Everything non-graphic.*

- Personality traits + economic desperation + conflicts → motive scores.
- Offences from `/data/crimes.ron`: theft, burglary, fights, rare homicide.
- Police patrols (analytic routes), witnesses, investigations with evidence points.
- Arrests, trials (simple deterministic verdict model), sentences, jail population,
  release and recidivism.
- Consequences for victims and families (needs, money, safety, moving away).
- Safety perception per neighbourhood affecting decisions and rents.

## Phase 6 — New cities and the world map

🎯 *When land runs out or the population grows, a new town is founded down the highway.
Zoom out to see the whole world: every town is an icon with its stats; distant towns
render as simplified meshes. People commute and move between towns.*

- Expansion rules: founding new regions, highway construction between gateways.
- Intercity graph growth and HPA*-style routing.
- Client: region streaming (download only the regions in view), impostors for far
  regions, world-map view with per-region stats.
- Manifest per region with summaries; incremental/delta snapshots.
- Runner: measure and, if needed, prepare sharding across a job matrix.

## Phase 7 — Technology

🎯 *A university and tech firms invest in research; the tech tree in the stats panel
fills up over weeks; new buildings, vehicles and improvements appear in the city.*

- Research points from university/firms; tech tree from `/data/technologies.ron`.
- Unlocks: building types, productivity modifiers, new vehicles/transport (e.g. buses,
  trams), energy upgrades.
- `TechDiffusion` messages between regions.

## Phase 8 — Big world (500k–1M inhabitants)

🎯 *Dozens to hundreds of towns, a million inhabitants, and the page still loads fast.*

- Profiling and optimisation guided by the benchmarks maintained since Phase 1.
- Runner sharding by regions across parallel jobs if a single job no longer fits.
- Chunk-level delta snapshots, smarter caching (ETags, immutable versioned paths).
- Optional free CDN mirror if Pages bandwidth becomes a limit.
- WebGPU renderer path (optional).

## Continuous tracks (all phases)

- Content: more professions, buildings, activities, name styles (good first issues).
- Art: voxel meshes, animations, palette, accessibility (colour-blind safe UI).
- i18n of UI and journal templates (English + Spanish first).
- Docs and onboarding for contributors.
