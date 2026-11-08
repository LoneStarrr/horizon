# Generated attachment batch execution proof

Build `tests/attachments/mod` with `cargo horizon-mod build --wasm`, then build the
std member with `cargo horizon-mod wasm-std <output.wasm>`. These use the pinned
Rust toolchain and need its `rust-src` component. Run the one focused test:

```sh
HORIZON_WASM_ATTACHMENTS=<fixture.hmod> HORIZON_WASM_STD=<output.wasm> \
  cargo test -p horizon-mod-wasm --test attachments \
  generated_batch_crosses_shared_and_mutable_frames -- --ignored --nocapture
```

The shared `entity_mob_griefing` hook has only `&dyn Host`, exercising the shared
call frame rather than borrowing a mutable frame for a read. Its guest query
lazily decodes a raw saved entry and leaves the host's saved data unchanged.
The server chat hook then exercises `query_mut`, set, remove, snapshot isolation,
fresh-query visibility and a retained batch after unload. The native test checks
stored data and index membership independently; a trapped or skipped guest hook
cannot satisfy its expected outputs. The main/dispatch `wasm-mods` workflow builds
and executes this fixture.

The accompanying Game idle proof uses 10,000 real painting holders with saved
attachments and calls an unfrozen server `tick`. An opt-in runtime test feature
counts lookup, validation and mutation entry points. The counter must not move;
no counter field, atomic operation or allocation exists in normal runtime builds.
