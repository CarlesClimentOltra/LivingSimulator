# Contributing to Living Simulator

Thanks for wanting to help! This guide explains how to add content, the code style, and
the one rule that matters most: **determinism**.

## Ways to contribute

1. **Content (no Rust needed).** Add professions, buildings, activities, schedules, name
   styles or newspaper templates in [`/data`](data/).
2. **Simulation.** Work on the systems in `crates/livingsim-core` (see
   [docs/ROADMAP.md](docs/ROADMAP.md) for what comes next).
3. **Rendering and UI.** Work on `web/` (TypeScript and Three.js).
4. **Docs, tests and benchmarks.**

[docs/GOOD_FIRST_ISSUES.md](docs/GOOD_FIRST_ISSUES.md) has a list of small, well-scoped
tasks to start with.

## Adding content in `/data`

The content is in RON files. `livingsim-content` validates them at build time and reports
every problem, with the file and the id. To check your change, run:

```sh
cargo build -p livingsim-core
```

| File | What it defines |
|---|---|
| `needs.ron` | Decay and weight of the 7 needs (the set itself is fixed by the engine) |
| `activities.ron` | Things people do: kind, place (`Home`, `Workplace`, `Tag("…")`), effects per hour on needs, duration, cost, ages |
| `schedules.ron` | Daily agendas: obligation blocks (`days`, `time`) and preferred sleep |
| `professions.ron` | Wages per shift and the schedules that jobs use |
| `buildings.ron` | Lot size, footprint, floors, dwellings, jobs, tags, zones (generator weights), opening hours, capacity, colours, roof, name patterns |
| `names/style_*.ron` | Syllable sets for invented names. A new file is a new naming style |
| `names/common.ron` | Blacklist and street suffixes |
| `journal.ron` | Templates for diary entries and newspaper items |
| `economy.ron`, `world.ron` | Economy parameters and the initial regions |
| `goods.ron`, `crimes.ron`, `technologies.ron` | Content for later phases (schemas exist already) |

Examples:

- **A new leisure activity.** Add a building with a new tag (for example
  `tags: ["museum"]`, with `zones` so the generator places it), then add an activity
  with `place: Tag("museum")`.
- **A new profession.** Add it to `professions.ron` and reference it from a building's
  `jobs`.

Names must always be invented. Never add real people's names, brands or real places.

Changing data changes the simulation. Read "Updating the golden hash" below.

### Economy rules (Phase 2)

- Money is `i64` cents (US dollars). **Never** change a balance directly: use
  `Region::transfer(from, to, amount)`. The `money_is_conserved` test fails if money is
  created or destroyed.
- Money that enters or leaves the region goes through the external accounts
  `Account::World` (rest of the world) or `Account::OtherRegions` (other towns, via
  messages).
- Business behaviour is tuned in `data/economy.ron` and the `firm` field of
  `data/buildings.ron`. To check a change, run
  `cargo run --release -p livingsim-core --example economy -- 30`.

## Determinism: what you must not break

Every browser replays the same simulation that the runner computed. A single
non-deterministic operation splits the world. In `crates/livingsim-core`:

1. **No I/O, no system clock, no threads, and no global mutable state.** The only global
   allowed is the immutable embedded content.
2. **No floats** (`f32` or `f64`). Use integers and fixed point. CI greps for them.
3. **No iteration over `HashMap` or `HashSet`.** Use `BTreeMap`, `Vec`, or sorted keys.
   A clippy config bans these types in the core.
4. **No `usize` in serialized state**, and none in arithmetic that could overflow
   differently on 32-bit WASM and 64-bit native.
5. **Randomness only from `rng::Rng`** (seeded ChaCha8, consumed in a fixed order) or
   **`rng::keyed(seed, …)`** (a stateless hash). Give each new kind of decision its own
   `Purpose`.
6. **Total order everywhere.** Process events by agent id, and sort with keys that never
   tie in a meaningful way.
7. **Parallelism only across regions**, merged in region-id order.
8. **Regions never read another region's state.** They only send `messages::Body`.
9. **The level of detail must never depend on the camera.** Render-only details, such as
   where someone sits inside a building, are computed in the frontend.

### Updating the golden hash

`tests/determinism.rs::golden_hash` pins the exact output of the simulation. If you
change behaviour on purpose (code or data), run:

```sh
./scripts/update-golden.sh
```

Then commit `crates/livingsim-core/tests/golden.txt` and explain the change in the PR. If
the hash changes when you did not expect it to, you have found a bug.

### Changing the snapshot layout

If you add or change serialized fields, follow these steps:

1. Bump `snapshot::FORMAT_VERSION`.
2. Freeze the old types in a `snapshot::vN` module and add a step to `migrate`.
3. Add a small fixture of the old format under `crates/livingsim-core/tests/fixtures/`.

### Performance budget

- Do not add per-tick work for every agent. Schedule an event instead.
- Keep per-agent columns small. `memory_budget_per_agent` fails above 384 bytes.
- `cargo run --release -p livingsim-bench --bin bench-report` prints the numbers that CI
  tracks, and CI alerts on regressions above 25 %.

## Code style

- **English** for code, identifiers, comments and docs. `README.es.md` is the Spanish
  entry point.
- **Rust:** run `cargo fmt --all` and make sure
  `cargo clippy --workspace --all-targets -- -D warnings` passes. Prefer plain data plus
  functions over deep abstractions.
- **TypeScript:** `strict` mode, and `npm run typecheck` must pass. Keep allocations out
  of the per-frame loop.
- Keep commits small and descriptive, for example `feat(core): …`, `fix(web): …` or
  `content: add bakery`.

## Before opening a PR

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --release
./scripts/test-wasm.sh            # if you touched the core
cd web && npm run typecheck       # if you touched the web
```

Fill in the PR template. Screenshots help a lot with visual changes. By contributing you
agree that your work is dual-licensed under MIT or Apache-2.0, and you agree to follow the
[Code of Conduct](CODE_OF_CONDUCT.md).
