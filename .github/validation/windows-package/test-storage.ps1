$ErrorActionPreference = 'Stop'
$requiredSchemaTests = @(
  'standalone_state_initializes_schema_jsonb_and_bundled_pricing',
  'standalone_file_open_rejects_future_schema_without_modifying_bytes',
  'v29_file_upgrade_creates_journal_and_preserves_original_snapshot_and_records',
  'v29_upgrade_snapshot_failure_blocks_schema_changes',
  'v30_file_without_journal_upgrades_with_original_snapshot',
  'v30_file_with_journal_preserves_row_bytes_and_snapshot_on_reopen',
  'upstream_v31_file_admission_requires_journal_and_prechange_snapshot',
  'upstream_v31_snapshot_failure_blocks_journal_and_pricing_writes',
  'v31_existing_journal_preserves_rows_without_new_snapshot',
  'v31_unrecognized_journal_schema_is_not_overwritten'
)
# The three Linux-only lifetime tests are not compiled on Windows. Do not claim
# Linux's 37 cases here: Windows storage selects 11 + 13 + 10 = 34 real tests.
$targets = @(
  @{ name='storage-lib'; manifest='crates/ai-toolbox-storage/Cargo.toml'; target=@('--lib'); count=11; required=@() },
  @{ name='storage-journal'; manifest='crates/ai-toolbox-storage/Cargo.toml'; target=@('--test', 'codex_context_journal'); count=13; required=@() },
  @{ name='storage-schema'; manifest='crates/ai-toolbox-storage/Cargo.toml'; target=@('--test', 'sqlite_jsonb'); count=10; required=$requiredSchemaTests },
  @{ name='desktop-schema'; manifest='tauri/Cargo.toml'; target=@('--test', 'sqlite_jsonb'); count=31; required=@() },
  @{ name='desktop-storage-identity'; manifest='tauri/Cargo.toml'; target=@('--test', 'storage_reexports'); count=1; required=@('desktop_reexports_share_storage_types_and_connection') }
)
foreach ($set in $targets) {
  $list = Join-Path $env:RUNNER_TEMP ('storage-tests-' + $set.name + '-list.log')
  $log = Join-Path $env:RUNNER_TEMP ('storage-tests-' + $set.name + '-result.log')
  $manifest = $set.manifest
  $target = $set.target
  $required = $set.required
  $count = $set.count
  cargo test --locked --manifest-path $manifest @target -- --list 2>&1 | Tee-Object -FilePath $list
  $listExit = $LASTEXITCODE
  node .github/validation/windows-package/check-storage.mjs list $list $listExit $count @required
  if ($LASTEXITCODE -ne 0 -or $listExit -ne 0) { throw 'Storage target selection failed' }
  # No --nocapture: successful fixture diagnostics must not interrupt the test
  # identity lines consumed by the existing fail-closed native result validator.
  cargo test --locked --offline --manifest-path $manifest @target -- --test-threads=1 2>&1 | Tee-Object -FilePath $log
  $testExit = $LASTEXITCODE
  node .github/validation/windows-package/check-storage.mjs result $list $log $testExit $count @required
  if ($LASTEXITCODE -ne 0 -or $testExit -ne 0) { throw 'Storage target regression failed' }
}
