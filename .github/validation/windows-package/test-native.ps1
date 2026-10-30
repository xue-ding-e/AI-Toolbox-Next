$ErrorActionPreference = 'Stop'
foreach ($set in @(
  @{ filter='coding::codex::official_accounts::tests::auth_export_'; count=3 },
  @{ filter='coding::codex::official_accounts::oauth_send_diagnostic_tests::'; count=9 },
  @{ filter='http_client::tests::'; count=5 }
)) {
  $log = Join-Path $env:RUNNER_TEMP ('native-tests-' + $set.count + '.log')
  cargo test --manifest-path tauri/Cargo.toml --locked --lib $set.filter -- --test-threads=1 2>&1 | Tee-Object -FilePath $log
  $exit = $LASTEXITCODE
  node .github/validation/windows-package/check-native.mjs $log $exit $set.filter $set.count
  if ($LASTEXITCODE -ne 0 -or $exit -ne 0) { throw 'Native regression gate failed' }
}
