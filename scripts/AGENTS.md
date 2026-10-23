# Maintenance verification scripts

## Source of Truth

The registered desktop command paths come from `tauri/src/lib.rs`. The generated operation inventory records these lexical paths and their source hash, not a resolved business API or a security permission boundary.

## Design decisions

Inventory consistency and business completion are separate gates. `operations:check` detects source drift. `operations:complete` must fail while any applicable business path is unclassified, unimplemented, or unverified. A `NOT_SUPPORTED` response must never count as implemented coverage. Service, tray, and background entry points require additional semantic review and cannot silently disappear from the denominator.

The inventory lexer rejects unknown registration syntax rather than silently skipping entries. Do not equate the current handler count with the permanent operation count, infer public schemas from function names, or expose an arbitrary invoke dispatcher.

## Minimum verification

Run `pnpm run test:operation-inventory` and `pnpm run operations:check` for inventory changes. Verify the incomplete business gate still exits nonzero until the reviewed operation registry and its acceptance tests actually cover the applicable source paths.
