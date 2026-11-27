# Good first issues

These are small, self-contained tasks for newcomers. Each one points at the files
involved. Comment on (or open) the matching GitHub issue before you start, so that two
people don't work on the same thing.

## Content (no Rust needed)

1. **Bakery.** Add a `bakery` building (small lot, Residential/Mixed zones, early opening
   hours) with a `baker` profession on an `early_shift`, and a "buy bread" `EatOut`
   activity. *Files: `data/buildings.ron`, `data/professions.ron`,
   `data/activities.ron`.*
2. **Library.** Add a `library` (tag `library`, opening hours) and a quiet "study"
   leisure activity for ages 8+. *Files: `data/buildings.ron`, `data/activities.ron`.*
3. **A third naming style.** Create `data/names/style_coastal.ron` with new invented
   syllables, then use it for a region in `data/world.ron`.
4. **More newspaper variety.** Add alternative wordings for the templates in
   `data/journal.ron`. This also needs a tiny engine change so the engine can pick one of
   several templates (see issue 12).
5. **Tune the needs.** People watch a lot of TV. Adjust `bias` and `effects` in
   `data/activities.ron` so evenings are more varied. Back your change with the output
   of `cargo run --release -p livingsim-core --example inspect -- 3`.

## Simulation (Rust, small)

6. **Lunch breaks.** Workers stay at work for their whole shift. Add an optional lunch
   block to schedules (`data/schedules.ron` + `livingsim-content`) and let agents eat at
   a nearby café or restaurant. *Files: `life.rs::decide`.*
7. **Weekend markets.** On Saturdays, plazas could hold a market that sells groceries.
   *Files: `life.rs::open_until`, `buildings.ron`.*
8. **Cars for families.** Only households with `has_car` drive. Make the share depend on
   income and region type. *Files: `gen.rs::populate`.*
9. **Birthdays in the newspaper.** Report the oldest inhabitant's birthday as a news item.
   This needs a new `NewsKind` and template. *Files: `region.rs::morning`, `journal.ron`.*
10. **Test: no one sleeps at work.** Write a test that runs two days and checks that every
    agent whose activity is `Sleep` is at home. *Files:
    `crates/livingsim-core/tests/`.*

## Frontend (TypeScript)

11. **Street lamps.** Add instanced lamp posts along avenues that glow at night (use
    `stage.night`). *Files: `web/src/render/city.ts`.*
12. **Search.** Add a box to find an inhabitant by name and follow them. This needs a new
    worker request that searches the name pool. *Files: `web/src/main.ts`,
    `crates/livingsim-wasm/src/lib.rs`.*
13. **Better cars.** Give cars wheels and headlights at night, and rotate them smoothly at
    corners. *Files: `web/src/render/people.ts`.*
14. **Clouds and birds.** Add a few slow low-poly clouds. They are purely decorative and
    must not depend on the simulation. *Files: `web/src/render/scene.ts`.*
15. **Accessibility.** Make the panels keyboard-navigable, add ARIA labels, and offer a
    colour-blind-safe palette for the need bars. *Files: `web/src/ui/panels.ts`,
    `web/src/style.css`.*
16. **Localisation.** Translate the UI strings into Spanish, with a language switch.
    Diary templates come from `data/journal.ron`, so plan how to localise those too.
17. **Region labels on the world map.** When zoomed out, show each town's name and
    population above it. *Files: `web/src/main.ts`.*

## Infrastructure and docs

18. **Bench dashboard link.** Publish the `bench-data` charts and link them from the
    README.
19. **Snapshot fixture.** Add a tiny committed region snapshot (village template) under
    `crates/livingsim-core/tests/fixtures/`, plus a test that it still decodes. This
    protects the migration system.
20. **Architecture diagram.** Turn the ASCII diagrams in `docs/ARCHITECTURE.md` into an
    SVG.
