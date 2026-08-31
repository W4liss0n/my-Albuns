# Native UI gate operating policy

Use headless checks during interactive work. They include focused Vitest, Rust,
contract, typecheck, build, and structural gate tests and may be repeated while
the implementation is changing.

Run the focused native gate `npm run test:native-owned-dialogs` once after GREEN.
It owns exactly two visible scenarios: the Global opening owner deciding a
read-only external copy, and the Project owner reporting a late graphics
failure. A failed visible run is diagnosed from its retained log and headless
seams before another visible run is authorized.

The focused gate builds its own debug Tauri `custom-protocol` application and
records that executable's canonical path and SHA-256. The path follows the
repository's `CARGO_TARGET_DIR` resolver for default, absolute, and relative
values. A plain `cargo build` is a development-server artifact and cannot
satisfy the gate's source provenance.

The external-copy scenario completes only after one public terminal correlates
the dialog attempt to the exact pending Host process and one subsequent Global
activation terminal closes that batch. Missing, additional, or mismatched
records fail closed.

Run the full productive journey only for integration, release, or with explicit
permission. The focused gate neither imports nor invokes that journey and is not
part of any standard headless command.

Validate dialog appearance separately with filtered UI acceptance. Set
`MYALBUNS_UI_SCENARIO_IDS` to the complete affected ID list before invoking
`npm run ui:acceptance`; do not capture unrelated surfaces.
