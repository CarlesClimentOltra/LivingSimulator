# Living Simulator — Roadmap

> Status: **approved**. Phases 0–6 are implemented; Phase 7 is next.
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

## Phase 2 — Economy (+ ground and interiors polish)

🎯 *Money really circulates. Shops set their prices and run out of stock when wholesalers
can't keep up. Firms hire, fire and sometimes go bankrupt, and new owners reopen them. People
pay monthly rent to the municipal housing company, a private agency or a private landlord;
some own their home. The town hall collects taxes and pays teachers, police and doctors.
The stats panel charts prices, unemployment and the budget, and the newspaper reports on all
of it. Around the towns there is real land instead of sky, and zooming into a building
removes its roof and shows each floor, with families on their own floor and furniture.*

Decisions (approved):
- Middle-ground realism: a closed money loop, firms, dynamic prices, a public budget. No
  banks or loans yet.
- The currency is the US dollar ($), stored in integer cents.
- Housing is owned by a municipal housing company, private real-estate agencies, and private
  people (owner-occupiers, or landlords who receive the rent).
- Delivery trucks are deferred: shipments are invisible for now and become visible trips
  in a later phase.

Milestones:
1. **Money accounting.** Every transfer goes through one function. A test checks that money
   in a region is conserved except for explicit external flows (rest of the world, other
   regions).
2. **Firms.** One firm per commercial or industrial building (cash, stock, price, daily
   books). Public employers are paid by the treasury. Wages are paid per shift by the
   employer, with income tax withheld.
3. **Goods and supply.** Farms and factories produce per worked shift. Warehouses buy from
   producers and supply shops and restaurants. Shortfalls are imported at a premium. Food
   surplus is sold to neighbouring towns: `Trade` carries an invoice and `Payment` returns
   the money.
4. **Prices.** Daily integer rules based on stock against target, within bounds. Sales tax
   goes to the treasury.
5. **Housing and rent.** Ownership per household, rent by zone and building, paid on the
   1st of each month. Arrears are journaled; evictions come in Phase 3.
6. **Labour market.** Vacancies, daily matching of job seekers, layoffs when cash runs
   out, bankruptcy, and reopening with an investor's savings.
7. **Town hall.** Income and sales tax, pensions and benefits, public wages, maintenance,
   and a tax rate that adjusts to the budget.
8. **Snapshot format v2** with the first real migration (v1 worlds keep their people and
   gain an economy).
9. **UI.** Firm cards (price, stock, staff, profit), rent and ownership on person cards,
   an economy section with 60-day charts, economic news.
10. **Visual polish.** Real land around regions, roof-less cutaway interiors with floor
    slabs, furniture, and households on their own floor.

## Phase 3 — Life cycle

🎯 *People make friends, fall in love, move in together, marry and sometimes divorce.
Babies are born and named after the family, and couples who cannot have children (for
example same-sex couples) adopt. Children go to daycare, school and university, adults
retire at 66. People fall ill, are treated at the hospital and, eventually, die of old age
or illness; their families mourn and inherit. Villagers commute to jobs in the city, and
families move between towns or leave the region. Family trees appear in person cards, and
births, weddings and obituaries in the newspaper.*

Decisions (approved):
- Deaths only from old age or illness, always non-graphic.
- Couples can be of different or the same sex. Divorce and blended families exist. Only a
  woman of fertile age with a male partner can conceive; any couple may adopt.
- Housing stays fixed until Phase 4. People without a free home stay with their family or
  leave the region.
- Commuting between towns is part of this phase.

Milestones:
1. **Content.** `demography.ron` (mortality and illness by age, fertility, partnering,
   marriage, divorce, adoption, leaving home, migration rates), education levels for
   professions, daycare and cemetery buildings, and new activities (visit a friend,
   recover).
2. **Friends.** Up to four per person, made among coworkers, classmates and neighbours. A
   "visit a friend" activity.
3. **Couples.** Sexual orientation derived from the seed. Dating, moving in (into a free
   home or the partner's), marriage, divorce.
4. **Children.** Pregnancy, birth, names chosen from the family's names, parental leave,
   adoption (orphans first, otherwise children from outside the region).
5. **Stages of life.** Daycare (1–5), school (6–17), university (4 years, optional) or a
   first job, education requirements for jobs, retirement at 66.
6. **Health and death.** Illness (at home, or in hospital when severe), recovery, natural
   death. Inheritance through `transfer` (money is still conserved), firms and rented-out
   homes pass to heirs, mourning, funerals at the cemetery, orphans.
7. **Migration and commuting.** Children leaving home, emigration of long-term job seekers,
   immigration into free homes and jobs, families moving between towns (`Movers`), and
   villagers filling city vacancies (`JobOffers` / `JobAccept` / `RemoteWage` /
   `JobEnded`).
8. **Snapshot format v3** with migration from v2, keeping everyone (with education and
   orientation assigned deterministically).
9. **UI.** Family tree and education on person cards, a population pyramid and
   demographic rates, newspaper stories (births, weddings, obituaries, the oldest
   resident).

## Phase 4 — Construction and growth

🎯 *The town notices what it lacks — homes for couples living apart and young people still
with their parents, shops where shelves run empty, daycare, a cemetery, school places — and
builds it. Investors, real estate agencies and the town hall fund projects; the
construction company hires builders who go to the site every day. Sites are fenced, and
foundations, scaffolding and floors rise phase by phase (a house takes 2–4 real days).
When empty lots run out, new streets are laid out into the farmland around the town.*

Decisions (approved):
- Growth by infill of empty lots and outward expansion. No demolition or densification
  yet.
- Realistic pace: a house takes 1–2 simulated months (2–4 real days), an apartment block
  4–6 simulated months.
- Building materials are bought from outside the region.
- The phase also includes a review of the live world, the mobile check and the
  generator's age pyramid.

Milestones:
1. **Review.** Health projection of the live world (the `health` example), the generator's
   age pyramid, mobile settings.
2. **Room to grow.** Regions get an expansion ring of undeveloped blocks (`max_blocks` per
   template). Existing worlds are migrated by shifting coordinates, so their world
   position is unchanged.
3. **Demand.** Weekly planning from the simulation's own signals: couples living apart,
   adults stuck with their parents, empty shelves, full schools and daycares, missing
   cemetery, unemployment.
4. **Projects.** Buildings and roads go through planned → foundation → structure → roof
   → finished. Work is measured in builder-shifts. The funder (town hall, agency or
   investor) pays each phase to the construction company, and materials are imported.
   Firms and tenures are created on completion.
5. **Builders on site.** Construction workers go to their firm's active site and
   contribute their shifts. Regions without a builder use slower outside contractors.
6. **Expansion.** When no suitable lot is free, street projects open up a new block next to
   the town. New roads invalidate cached routes.
7. **Rendering.** Dirt road works, fenced lots, a structure growing by phase, scaffolding
   and cranes. The worker sends updated static data when the map changes.
8. **Snapshot format v5** with a migration from v4 (content ids kept). Tests for money
   conservation, project completion and expansion.
9. **UI.** A "Construction" panel with active projects and progress, building cards for
   sites, and newspaper stories (works start, inaugurations, new streets).

Notes from the implementation:
- Construction companies work in crews (`crew_size`, four builders per project); builders
  without a project wait at the yard, so an oversized company loses money and lays people
  off. Parks and cemeteries only need landscaping (the foundation phase).
- Homes in new buildings are let by whoever paid for them: the municipal housing company,
  the agency, or the investor (as a private landlord).
- A new daycare or school takes in the children who were waiting for a place.
- The generator builds more homes than the starting population needs, so a city first
  grows shops, services and workplaces; housing follows when its population grows.

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

Decisions (approved):
- Homicide is included but very rare (about one every few real months in the city), shown
  only as news, investigation, trial and the victim's death. Nothing graphic.
- Realistic frequency: a few shopliftings a day in the city, a burglary or a fight every
  few days, fraud about once a month; the village is almost quiet.
- The village has no police station or jail: city officers investigate there and its
  convicts serve time in the city's jail (a `Prisoner` message between regions).

Implemented as:
- `data/crimes.ron` (rate per 100 million adult-days, loss, evidence needed, clearance,
  hours) and `data/justice.ron` (police, courts, motive multipliers, safety).
- Motive = honesty × age × (money trouble or arrears for property crimes; temper and a
  recent layoff or break-up for fights) × criminal record.
- Offences are planned at 06:00 and happen on the hour. Witnesses add evidence at once,
  each police shift adds more (most serious case first), and only cases with leads (the
  clearance rate) can be solved. Minor offences end in a fine; the rest in custody, a trial
  ten days later and a sentence, or an acquittal. Convicts lose their job.
- Victims lose money or are hurt (a few days of recovery), feel less safe, and some
  burgled families move to a safer block. Unsafe blocks rent for less.
- Snapshot format 6 (frozen v5 layout in `snapshot_v5.rs`; crime ids added to the content
  ids).
- Map: a marker over recent incidents, police tape around serious ones, police cars on
  patrol (lights on). Panel: safety stats and a police log; cards show records and jail
  time; newspaper stories for reports, arrests and verdicts.

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

Implemented as:
- **Pressure.** Every morning each town adds points for long-term job seekers, people who
  left for lack of work or a home, and households waiting for a home beyond the free ones;
  four times as many once it has no room left to grow (`world.ron` → `expansion`).
- **Founding.** At 06:00 the world founds a town next to the town under most pressure
  (over the threshold, after the world is 30 days old, one founding per 120 days, at most
  8 towns). It takes a free side of the parent (north, south, east or west) where it does
  not overlap anyone. The new town (`new_town` template, ~250 people) sends `RoadBuilt` to
  its parent, which builds its own highway exit (so every town still changes only through
  its messages, and browsers replay it exactly); up to 15 families of job seekers move
  there as settlers.
- **Intercity graph.** Highways are the top level of routing (`IntercityGraph::route`).
  People visit, commute to and move to neighbouring towns.
- **Snapshot format 7** (expansion state; `snapshot_v6.rs`).
- **Runner.** Message logs for towns founded mid-run; per-town summaries in
  `manifest.json` and a static file per town (map, buildings, roads).
- **Browser.** Only the town being watched is downloaded and simulated; the others are
  drawn from their static files (no people) and start being simulated when the visitor
  goes there (`#town=ID` in the URL). Zoomed out, each town has a label with its figures;
  highways are drawn from the manifest.
- **Measured:** one runner job advances the whole world (10,500 people) by a day in
  ~0.5 s, so sharding is not needed yet; it becomes useful at roughly a hundred towns.
- **Deferred:** delta snapshots (checkpoints are small: ~1 MB for the city) and region
  impostors simpler than the full static meshes.

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
