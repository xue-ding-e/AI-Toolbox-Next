<#
Reused from xue-ding-e/DSH-Next, commit 8cacae480cb63508f13e310d5bbfea3566620dda.
Original helper SHA256: 3bf5ae21ec9eb9942105a413459a5068b7b82ee7b8a1e5cddf2be1188073d719
MIT License

Copyright (c) 2026 DeepSeek

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

#>
<# Native-only negative controls run before dependency installation or an installer. #>
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted' -or $env:RUNNER_OS -ne 'Windows') { throw 'Disposable hosted Windows only.' }
. (Join-Path $PSScriptRoot 'windows-owned-process.ps1')
$root = Join-Path $env:RUNNER_TEMP ('next-process-controls-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $root | Out-Null
$node = (Get-Command node.exe).Source
$result = Invoke-NextOwnedProcess -FilePath $node -Arguments @('-e','process.stdout.write("owned-success")') -WorkingDirectory $root -TimeoutSeconds 10 -EvidencePrefix (Join-Path $root 'success')
if ($result.exitCode -ne 0 -or $result.timedOut) { throw 'Successful process was not observed.' }
$entry = Join-Path $root 'parent.mjs'
$marker = Join-Path $root 'descendant.json'
$heartbeat = Join-Path $root 'heartbeat.bin'
$child = 'const fs=require("fs");fs.writeFileSync(process.argv[1],JSON.stringify({pid:process.pid}));setInterval(()=>fs.appendFileSync(process.argv[2],"."),20)'
@'
import { spawn } from 'node:child_process';
const child=spawn(process.execPath,['-e',process.argv[2],process.argv[3],process.argv[4]],{stdio:'ignore',detached:true});
child.unref(); setInterval(()=>{},1000);
'@ | Set-Content -LiteralPath $entry -Encoding utf8
$failed = $false
try { $null = Invoke-NextOwnedProcess -FilePath $node -Arguments @($entry,$child,$marker,$heartbeat) -WorkingDirectory $root -TimeoutSeconds 3 -EvidencePrefix (Join-Path $root 'timeout') }
catch { $failed = $true }
if (-not $failed) { throw 'Timeout was reported as successful.' }
$record = Get-Content -LiteralPath (Join-Path $root 'timeout.result.json') -Raw | ConvertFrom-Json
if (-not $record.timedOut -or $record.exitCode -eq 0) { throw 'Timeout and process exit must be independently reported.' }
if (-not (Test-Path -LiteralPath $marker) -or -not (Test-Path -LiteralPath $heartbeat)) { throw 'Owned descendant never reached the timeout fixture.' }
$pidValue = (Get-Content -LiteralPath $marker -Raw | ConvertFrom-Json).pid
if ($null -ne (Get-Process -Id $pidValue -ErrorAction SilentlyContinue)) { throw 'Owned descendant survived job termination.' }
$length = (Get-Item -LiteralPath $heartbeat).Length
Start-Sleep -Milliseconds 150
if ((Get-Item -LiteralPath $heartbeat).Length -ne $length) { throw 'Owned descendant mutated data after cleanup.' }
if (-not [NextOwnedProcess]::SafeToStart) { throw 'Confirmed timeout cleanup unnecessarily poisoned this session.' }
$failed = $false
try {
    # A single synchronous write followed by exit 0 must still fail the final output-length gate.
    $null = Invoke-NextOwnedProcess -FilePath $node -Arguments @('-e','require("fs").writeSync(1,Buffer.alloc(33*1024*1024,120))') -WorkingDirectory $root -TimeoutSeconds 10 -EvidencePrefix (Join-Path $root 'fast-output')
} catch { $failed = $true }
$record = Get-Content -LiteralPath (Join-Path $root 'fast-output.result.json') -Raw | ConvertFrom-Json
if (-not $failed -or -not $record.outputExceeded -or (Get-Item -LiteralPath (Join-Path $root 'fast-output.stdout.log')).Length -le 32MB) { throw 'Fast successful oversized output bypassed the byte bound.' }
if (-not [NextOwnedProcess]::SafeToStart) { throw 'Confirmed output-limit cleanup unnecessarily poisoned this session.' }
# A successful parent cannot make its still-running owned child a successful command.
$exitEntry = Join-Path $root 'parent-exits.mjs'
$exitMarker = Join-Path $root 'exit-descendant.json'
$exitHeartbeat = Join-Path $root 'exit-heartbeat.bin'
@'
import { spawn } from 'node:child_process'; import { existsSync } from 'node:fs';
const child=spawn(process.execPath,['-e',process.argv[2],process.argv[3],process.argv[4]],{stdio:'ignore',detached:true});
child.unref(); const start=Date.now(); const timer=setInterval(()=>{
  if(existsSync(process.argv[3])){clearInterval(timer);process.exit(0)}
  if(Date.now()-start>5000)process.exit(2)
},20);
'@ | Set-Content -LiteralPath $exitEntry -Encoding utf8
$failed = $false
try { $null = Invoke-NextOwnedProcess -FilePath $node -Arguments @($exitEntry,$child,$exitMarker,$exitHeartbeat) -WorkingDirectory $root -TimeoutSeconds 25 -EvidencePrefix (Join-Path $root 'parent-exit') }
catch { $failed = $true }
$record = Get-Content -LiteralPath (Join-Path $root 'parent-exit.result.json') -Raw | ConvertFrom-Json
if (-not $failed -or -not $record.timedOut -or $record.exitCode -ne 0 -or $record.terminationReason -ne 'root-exit-descendants' -or -not $record.quiescenceConfirmed) { throw 'Parent exit 0 bypassed the descendant grace and cleanup gate.' }
if ($record.rootExitObservedMs -lt 0 -or ($record.elapsedMs - $record.rootExitObservedMs) -lt 15000 -or $record.elapsedMs -ge 25000) { throw 'Root-exit timeline lost its existing 15-second bound.' }
$pidValue = (Get-Content -LiteralPath $exitMarker -Raw | ConvertFrom-Json).pid
if ($record.snapshots.Count -ne 2 -or $record.snapshots[0].phase -ne 'root-exit' -or $record.snapshots[1].phase -ne 'before-owned-termination') { throw 'Owned lifecycle diagnostic phases were not retained.' }
foreach ($snapshot in $record.snapshots) {
    $observed = @($snapshot.processes | Where-Object { $_.pid -eq $pidValue })
    if (@($snapshot.processes | Where-Object { $_.pid -eq $PID }).Count -ne 0) { throw 'Observer included its unowned PowerShell caller.' }
    if ($snapshot.queryState -ne 'complete' -or $snapshot.truncated -or $observed.Count -ne 1) { throw 'Synthetic owned descendant was not observed completely.' }
    if ($snapshot.root.exitCode -ne 0 -or $observed[0].imageBasename -ne 'node.exe' -or $observed[0].state -ne 'running' -or $null -eq $observed[0].createdUtc) { throw 'Owned PID image, creation or root exit diagnostic was incorrect.' }
    if ($null -ne $observed[0].parentPid) {
        if ($observed[0].parentQueryStatus -ne 'reported' -or $observed[0].parentPid -ne $record.pid -or $observed[0].parentRelation -ne 'reported-parent-matches-observed-owned-lifetime') { throw 'A reported synthetic parent did not match its known owned lifetime.' }
    } elseif ($observed[0].parentRelation -ne 'unknown' -or $observed[0].parentQueryStatus -notmatch '^(unavailable|ntstatus-[0-9A-F]{8}|unexpected-size|pid-mismatch|unavailable-parent-id|query-exception-[A-Za-z0-9]+)$') {
        throw 'An unavailable optional parent query fabricated a relationship or omitted its status.'
    }
}
if ($null -ne (Get-Process -Id $pidValue -ErrorAction SilentlyContinue)) { throw 'Exited parent left its owned descendant running.' }
$length = (Get-Item -LiteralPath $exitHeartbeat).Length
Start-Sleep -Milliseconds 150
if ((Get-Item -LiteralPath $exitHeartbeat).Length -ne $length -or -not [NextOwnedProcess]::SafeToStart) { throw 'Descendant writes or uncertain cleanup survived the failed parent-exit case.' }
# Independent observer: hard-kill the owner at the pre-create barrier, suspended
# post-create barrier, and with a real detached grandchild already running.
$ownerEntry = Join-Path $root 'kill-owner.ps1'
$treeEntry = Join-Path $root 'owned-tree.mjs'
@'
import {spawn} from 'node:child_process'; import fs from 'node:fs';
const child = spawn(process.execPath, ['-e', 'require("fs").writeFileSync(process.argv[1],String(process.pid));setInterval(()=>require("fs").appendFileSync(process.argv[2],"."),20)', process.argv[2], process.argv[3]], {detached:true,stdio:'ignore'});
child.unref(); fs.writeFileSync(process.argv[4], String(process.pid)); setInterval(()=>{},1000);
'@ | Set-Content -LiteralPath $treeEntry -Encoding utf8
@'
$ErrorActionPreference='Stop'
. $env:AI_OWNER_HELPER
if ($env:AI_OWNER_MODE -eq 'before-create') {
    [NextOwnedProcess]::BeforeCreateForTest = [Action] { [IO.File]::WriteAllText($env:AI_OWNER_BARRIER,'ready'); [Threading.Thread]::Sleep(60000) }
}
if ($env:AI_OWNER_MODE -eq 'suspended') {
    [NextOwnedProcess]::BeforeResumeForTest = [Action[uint32]] { param($ownedPid) [IO.File]::WriteAllText($env:AI_OWNER_BARRIER,[string]$ownedPid); [Threading.Thread]::Sleep(60000) }
}
$null = Invoke-NextOwnedProcess -FilePath $env:AI_OWNER_NODE -Arguments @($env:AI_OWNER_TREE,$env:AI_OWNER_GRANDCHILD,$env:AI_OWNER_HEARTBEAT,$env:AI_OWNER_ROOTPID) -WorkingDirectory $env:AI_OWNER_ROOT -TimeoutSeconds 60 -EvidencePrefix $env:AI_OWNER_PREFIX
'@ | Set-Content -LiteralPath $ownerEntry -Encoding utf8
foreach ($mode in @('before-create','suspended','running')) {
    $prefix = Join-Path $root "hard-owner-$mode"
    $start = [Diagnostics.ProcessStartInfo]::new((Get-Process -Id $PID).Path)
    $start.UseShellExecute=$false
    foreach ($argument in @('-NoProfile','-NonInteractive','-File',$ownerEntry)) { $start.ArgumentList.Add($argument) }
    foreach ($pair in @{
        AI_OWNER_HELPER=(Join-Path $PSScriptRoot 'windows-owned-process.ps1'); AI_OWNER_MODE=$mode;
        AI_OWNER_NODE=$node; AI_OWNER_ROOT=$root; AI_OWNER_TREE=$treeEntry; AI_OWNER_PREFIX=$prefix;
        AI_OWNER_BARRIER="$prefix.barrier"; AI_OWNER_GRANDCHILD="$prefix.grandchild";
        AI_OWNER_HEARTBEAT="$prefix.heartbeat"; AI_OWNER_ROOTPID="$prefix.rootpid"
    }.GetEnumerator()) { $start.Environment[$pair.Key]=$pair.Value }
    $owner=[Diagnostics.Process]::Start($start)
    $null=$owner.Handle # Retain exact object identity; never select by process name.
    $owned=[Collections.Generic.List[Diagnostics.Process]]::new()
    try {
        $clock=[Diagnostics.Stopwatch]::StartNew()
        $ready=$false
        while ($clock.ElapsedMilliseconds -lt 20000 -and -not $owner.HasExited) {
            $ready = if ($mode -eq 'running') { (Test-Path -LiteralPath "$prefix.rootpid") -and (Test-Path -LiteralPath "$prefix.grandchild") -and (Test-Path -LiteralPath "$prefix.heartbeat") } else { Test-Path -LiteralPath "$prefix.barrier" }
            if ($ready) { break }
            Start-Sleep -Milliseconds 25
        }
        if (-not $ready) { throw 'Owner-kill fixture did not reach its exact barrier.' }
        $pidFiles = if ($mode -eq 'running') { @("$prefix.rootpid","$prefix.grandchild") } elseif ($mode -eq 'suspended') { @("$prefix.barrier") } else { @() }
        foreach ($pidFile in $pidFiles) {
            $observed=[Diagnostics.Process]::GetProcessById([int](Get-Content -LiteralPath $pidFile -Raw))
            $null=$observed.Handle
            if ($observed.StartTime -lt $owner.StartTime) { throw 'Owner-kill observed a different process identity.' }
            $owned.Add($observed)
        }
        $owner.Kill()
        if (-not $owner.WaitForExit(10000)) { throw 'Exact owner did not exit after hard kill.' }
        foreach ($observed in $owned) { if (-not $observed.WaitForExit(10000)) { throw 'Atomic Job member survived owner hard termination.' } }
        if (Test-Path -LiteralPath "$prefix.result.json") { throw 'Killed owner emitted a success receipt.' }
        if ($mode -eq 'running') {
            $length=(Get-Item -LiteralPath "$prefix.heartbeat").Length
            Start-Sleep -Milliseconds 150
            if ((Get-Item -LiteralPath "$prefix.heartbeat").Length -ne $length) { throw 'Killed-owner descendant still writes.' }
        } elseif ((Test-Path -LiteralPath "$prefix.rootpid") -or (Test-Path -LiteralPath "$prefix.grandchild")) { throw 'Suspended or uncreated test child executed.' }
    } finally {
        # Terminate only the directly-created owner; observation handles are wait/dispose only.
        if (-not $owner.HasExited) { $owner.Kill(); if (-not $owner.WaitForExit(10000)) { throw 'Owner fixture cleanup failed.' } }
        $owner.Dispose()
        $unconfirmed=$false
        foreach ($observed in $owned) {
            # Marker PID observations never authorize termination: a PID could have
            # been reused. Only the directly-created owner handle may be killed.
            if (-not $observed.WaitForExit(10000)) { $unconfirmed=$true }
            $observed.Dispose()
        }
        if ($unconfirmed) { throw 'Observed member exit unconfirmed; fail and discard the disposable worker.' }
    }
}

$failed = $false
$completedMarker = Join-Path $root 'injected-completed.txt'
try {
    $null = Invoke-NextOwnedProcess -FilePath $node -Arguments @('-e','require("fs").writeFileSync(process.argv[1],"owned")',$completedMarker) -WorkingDirectory $root -TimeoutSeconds 10 -EvidencePrefix (Join-Path $root 'uncertain-cleanup') -SimulateUncertainCleanup
} catch { $failed = $true }
if (-not $failed -or [NextOwnedProcess]::SafeToStart -or -not (Test-Path -LiteralPath $completedMarker)) { throw 'Injected unconfirmed cleanup did not poison the session.' }
$forbiddenMarker = Join-Path $root 'must-not-start.txt'
$failed = $false
try {
    $null = Invoke-NextOwnedProcess -FilePath $node -Arguments @('-e','require("fs").writeFileSync(process.argv[1],"unsafe")',$forbiddenMarker) -WorkingDirectory $root -TimeoutSeconds 10 -EvidencePrefix (Join-Path $root 'forbidden-start')
} catch { $failed = $true }
if (-not $failed -or (Test-Path -LiteralPath $forbiddenMarker) -or (Test-Path -LiteralPath (Join-Path $root 'forbidden-start.stdout.log'))) { throw 'A process started after ownership became uncertain.' }
# Leave the session poisoned deliberately. A later workflow step is a new disposable PowerShell process.
Write-Output 'Native owned-process success, timeout descendants, parent-exit diagnostics, fast output and fail-closed session controls passed (no compiler-tool exception enabled).'
