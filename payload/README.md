# Dedicated capsule decoder: new, unintegrated candidate

This is newly authored experimental Go code, not a restoration of the missing
earlier experiment. It has not been compiled or executed in this environment.
It is independent of the three already tested history safety patches. No
production package, history file, message, or remote repository was modified.

## Public protocol provenance

Independently implemented from the wire format, without copying the AGPL Rust
implementation into Magpie's MIT code:

- AI-Toolbox-Next commit `8575ade9e18cfc43c3338f2516a0cd14da904f90`:
  [capsule protocol](https://github.com/xue-ding-e/AI-Toolbox-Next/blob/8575ade9e18cfc43c3338f2516a0cd14da904f90/tauri/src/coding/proxy_gateway/runtime/compat/codex_remote_compaction.rs).
  Git blob `e97445dc213729fee49451836101b95d4b38ac43`, unchanged from the
  previously inspected `861aa4f05c83d0927128292276bd59bfeb54a2da` version.
- Its [Cargo.lock](https://github.com/xue-ding-e/AI-Toolbox-Next/blob/8575ade9e18cfc43c3338f2516a0cd14da904f90/tauri/Cargo.lock)
  blob `2f2ebbce97a84fd3c43dd82d45fb51958edd39e1` pins serde 1.0.228 and
  serde_json 1.0.149.
- serde [struct deserializer generation](https://github.com/serde-rs/serde/blob/v1.0.228/serde_derive/src/de/struct_.rs),
  blob `6a5da232acda9c961c380fe78f675d94970ce1ce`: duplicate known fields
  fail, absent required fields fail, unknown fields use IgnoredAny; sequence
  input is also supported by the Rust derived struct.
- serde [field identifier matching](https://github.com/serde-rs/serde/blob/v1.0.228/serde_derive/src/de/identifier.rs),
  blob `8e67b96310ce6c60ffe34f6764b6c704d62cbdef`: exact string matching,
  not case-insensitive aliases.
- serde_json [string and escape readers](https://github.com/serde-rs/json/blob/v1.0.149/src/read.rs),
  blob `f90d9f74a396254cf8780bdbad5ac500da3eec6b`: decoded strings validate
  surrogate pairing; ignore_escape used when discarding unknown values does
  not require Unicode scalar validity.
- serde_json [numeric deserializer](https://github.com/serde-rs/json/blob/v1.0.149/src/de.rs),
  blob `942e1c5b263b6c7a49a2cf4b89d168ef88e390f4`: parse_number makes
  `-0`, decimal and exponent forms F64, and parse_integer converts values above
  u64's maximum to F64. The serde [i64 visitor](https://github.com/serde-rs/serde/blob/v1.0.228/serde_core/src/de/impls.rs),
  blob `ab1a893cc4dcde659409c4677a71715cc220e368`, accepts integer visits with
  range checks, not floats. The candidate explicitly rejects `-0`; relying on
  Go's int64 unmarshaller alone would accept it. These are source-derived
  expectations, not an executed Rust oracle result.

Wire format: exact prefix `ai-toolbox-compact-v1:`, canonical unpadded base64url
of nonce (12 bytes) followed by AES-256-GCM ciphertext and tag (16 bytes), empty
AAD. Key is SHA256 of the public product constant
`ai-toolbox-remote-compaction`; this is format compatibility, not a claim that
the capsule protects confidentiality or proves trusted authorship. Plaintext
writer output is a JSON object with summary (string), model (string), and
created_at (i64).

## Contract layers and deliberate differences

1. Envelope verification and exact known object-field rules follow the public
   format. Duplicate known fields, including escape-equivalent names, fail.
   Case variants are unknown fields, not known aliases. Valid unknown fields
   and duplicate unknown fields are ignored. Native ciphertext and other tools'
   prefixes are never claimed by this helper.
2. Only the writer's object representation is accepted. Rust's derived
   deserializer additionally accepts a positional array; this candidate rejects
   it and requires the caller to preserve the original.
3. Raw UTF-8 and all escaped string scalars must be valid. This deliberately
   rejects lone surrogates and raw invalid UTF-8 even in ignored fields, more
   conservatively than Rust's IgnoredAny, rather than permitting Go's
   replacement-character repair. Rust's SliceRead::ignore_str can skip invalid
   raw UTF-8 inside ignored values and their nested keys, unlike parse_str for
   top-level field names and known string fields.
   Thus this is not complete serde input-language equivalence.
4. Rust's consumer trims the returned summary. This candidate tests nonemptiness
   with TrimSpace but returns the exact decoded string, preserving whitespace.
   It does not preserve the original JSON escape spelling in the returned text.
5. The 16 MiB encoded-body ceiling is local resource policy, not a Rust wire
   limit. It is checked before base64 decode or authenticated plaintext
   allocation. Decoded ciphertext is at most 12 MiB, and plaintext is smaller.
   No input or decrypted data is included in errors or logs.
6. Unknown numeric values are syntax-checked but not converted to floating
   point, so `1e400` is intentionally ignored. Rust's ignore_value uses an
   explicit stack and does not apply the normal 128 recursion budget to that
   ignored subtree. The candidate's Go JSON validator has its own 10000-level
   ceiling, another local conservative restriction, not a Rust protocol limit.

## Synthetic regression design and current evidence

Prepared but not run: 46 object/payload cases, 8 malformed envelope cases,
4 foreign-prefix controls, and an independent Python-generated fixed vector.
An additional canonical-base64 test flips only unused trailing bits of a valid
authenticated fixture and checks that a lax decoder returns the same bytes;
this isolates strict canonical rejection from short-body/authentication errors.
Tests cover exact summary preservation, empty/Unicode-whitespace-only rejection,
known field case mismatches, duplicate known versus ignored unknown fields,
escaped field names, null/type/integer bounds, UTF-8, paired and lone surrogates,
authentication failure, canonical base64 and preallocation size rejection.
Fixed nonce sealing exists only in test fixtures, never as production writing.
The fixed vector was independently decrypted again with Python cryptography
in this round: it contains exactly the expected synthetic summary, model and
timestamp. This checks fixture provenance, not execution of the Go decoder.
The oversized-input case checks rejection only. Its eventual execution cannot
prove allocation ordering; that is a source-review claim. Nor can passing an
immutable Go string establish future history preservation: no history writer
is present or exercised here.

The relevant minimal failure hypothesis is that direct Go struct unmarshalling
would accept case aliases and silently replace duplicate known fields, unlike
the Rust object contract. This candidate does not use that approach. No newly
executed RED/GREEN claim is made. The earlier experiment's executions do not
validate these newly authored bytes.

The later fixed-version Rust oracle should deserialize the same synthetic
plaintexts into an independently declared struct using serde 1.0.228 and
serde_json 1.0.149 with preserve_order (the product's declared feature), and
compare known-object classification against this Go implementation. Numeric
fixtures must include zero, negative zero, i64 min/max and either-side overflow,
u64 max/overflow, `1.0`, `1e0`, null and missing created_at. Record the resolved
Cargo.lock and features rather than assuming dependency feature unification.
Separate expected local-policy divergences (array input, ignored malformed
surrogates, resource limit) from accidental mismatches, and compare decoded
summary bytes separately from Rust's later trim operation. No Rust build or
dependency download has been started for this plan.

Suggested next step after independent source review: a real, bounded std-only
Go run of this exact isolated directory, recording toolchain identity, file
hashes, output and exit status. No dependency download is needed. A real Go
compiler and gofmt were not found at the inspected supported paths in the
current environment; `/usr/bin/go` was previously established to be a
different program, not the Go language toolchain. Do not use that as test RED.

## Not implemented

No history scanning or migration, request item transformation, role changes,
native reasoning handling, call_id pairing, writer locking, backup publication,
byte-offset or ordinal index regeneration, descendant fork cutoff changes,
SQLite projection coordination, or native resume validation. Any future caller
must preserve nonmatching and unreadable original values and must not treat
recognition as permission to rewrite history. Issues #1 and #6 remain open.
