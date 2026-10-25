# Maintenance verification scripts

## Source of Truth

The registered desktop command paths come from `tauri/src/lib.rs`. The generated operation inventory records these lexical paths and their source hash, not a resolved business API or a security permission boundary.

## Design decisions

Inventory consistency and business completion are separate gates. `operations:check` detects source drift. `operations:complete` must fail while any applicable business path is unclassified, unimplemented, or unverified. A `NOT_SUPPORTED` response must never count as implemented coverage. Service, tray, and background entry points require additional semantic review and cannot silently disappear from the denominator.

The inventory lexer rejects unknown registration syntax rather than silently skipping entries. Do not equate the current handler count with the permanent operation count, infer public schemas from function names, or expose an arbitrary invoke dispatcher.

## Minimum verification

Run `pnpm run test:operation-inventory` and `pnpm run operations:check` for inventory changes. Verify the incomplete business gate still exits nonzero until the reviewed operation registry and its acceptance tests actually cover the applicable source paths.

## Grok supervised fixture lifecycle

The six-editor/four-catalog wrapper uses fresh sequential Node children to avoid
retaining both Vite builds in one Node lifetime. It requires a reviewed owning
supervisor and a fresh private control directory; it is not a standalone route
around the resource gate. A child `close` event proves neither descendant exit
nor test success. Only a matching, one-use supervisor acknowledgement of the
previously observed generation and quiescent owned tree permits the next batch.
Unknown identities, refused metadata reads and absent acknowledgements stop;
they do not trigger another route, a browser retry or a fresh time/RSS budget.
The private directory/session protects against stale runs, not hostile same-UID
code. Overall acceptance still requires exact real 6 + 4 reports, unchanged
source/dependency seals and final owned cleanup under the original shared limits.
Hardlink publication changes ctime when its temporary alias is removed. Readers
must open the final record before checking that fixed staging alias, and take
stable metadata only after the alias disappears; permission errors are not waits.

`node --test scripts/lib/grok-batch-lifecycle.test.mjs` covers only fake processes
and fake I/O, including the three former batch-selector controls. It is not a
real browser or resource observation. The separate editor IPC/router controls
remain in `fixtureBoundaries.test.ts`; neither mock suite establishes 6 + 4.
