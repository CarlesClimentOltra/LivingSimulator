## What does this change?

<!-- A short description. Link the issue it closes, if any: "Closes #123". -->

## Kind of change

- [ ] Content (`/data`)
- [ ] Simulation (`livingsim-core`)
- [ ] Runner / WASM bindings
- [ ] Frontend (`web/`)
- [ ] Docs / CI

## Checklist

- [ ] `cargo fmt --all` and `cargo clippy --workspace --all-targets -- -D warnings` pass
- [ ] `cargo test --workspace` passes (includes the determinism tests)
- [ ] If the simulation output changes on purpose, I updated the golden hash (`scripts/update-golden.sh`) and explained why below
- [ ] If the snapshot layout changed, I bumped `FORMAT_VERSION` and added a migration + fixture
- [ ] No floats, `HashMap` iteration, system time or I/O in `livingsim-core`
- [ ] Benchmarks did not regress noticeably (or the regression is explained)

## Notes for reviewers

<!-- Screenshots for visual changes are very welcome. -->
