param([Parameter(Mandatory)][string]$Product,[Parameter(Mandatory)][string]$Evidence)
$ErrorActionPreference='Stop'
$control=$PSScriptRoot
$productRoot=(Resolve-Path $Product).Path
New-Item -ItemType Directory -Force -Path $Evidence | Out-Null
$evidenceRoot=(Resolve-Path $Evidence).Path
$env:AI_TOOLBOX_DATA_DIR=Join-Path $env:RUNNER_TEMP 'oauth-synthetic-app-data'
$env:CARGO_TERM_COLOR='never'
$env:RUST_BACKTRACE='0'
Push-Location $productRoot
try {
  foreach($set in @(@{label='oauth'; filter='coding::codex::official_accounts::oauth_send_diagnostic_tests::'},@{label='proxy'; filter='http_client::tests::'})) {
    $log=Join-Path $evidenceRoot ($set.label+'.log')
    & cargo test --manifest-path tauri/Cargo.toml --locked --lib $set.filter -- --test-threads=1 2>&1 | Tee-Object -FilePath $log
    $code=$LASTEXITCODE
    & node (Join-Path $control 'test-results.mjs') (Join-Path $control 'manifest.json') $set.label $log $code (Join-Path $evidenceRoot ($set.label+'.json'))
    if($LASTEXITCODE -ne 0 -or $code -ne 0) { throw ('Native test acceptance failed: '+$set.label) }
  }
} finally { Pop-Location }
