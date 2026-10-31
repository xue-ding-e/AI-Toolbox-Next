param([Parameter(Mandatory=$true)][string]$SyntheticHome)
$ErrorActionPreference = 'Stop'
$phaseClock = [Diagnostics.Stopwatch]::StartNew()
function Write-FixturePhase([string]$Name) {
  [Console]::Error.WriteLine("shell-fixture:$Name elapsed_ms=$($phaseClock.ElapsedMilliseconds)")
}
Set-StrictMode -Version Latest
# CI fixture only. NSIS expands shell variables with SHGetFolderPath and may
# initialize desktop.ini even under /NS. Prepare that OS state BEFORE snapshots.
# Source: NSIS v3.11 Source/exehead/util.c GetNSISString, lines 825-904.
if (-not [IO.Path]::IsPathRooted($SyntheticHome) -or $SyntheticHome.StartsWith('\\') -or
    $env:HOME -cne $SyntheticHome -or $env:USERPROFILE -cne $SyntheticHome) { throw 'Not the explicit synthetic profile' }
function Assert-NoLinks([string]$Path) {
  $cursor = $Path
  while ($cursor) {
    # GetFileAttributesW reports link attributes, including dangling links.
    if (([FixtureShellFolders]::Attributes($cursor) -band 0x400) -ne 0) { throw 'Synthetic profile contains a reparse point' }
    $cursor = [IO.Path]::GetDirectoryName($cursor)
  }
}
Write-FixturePhase 'add-type:start'
Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class FixtureShellFolders {
  [DllImport("kernel32.dll", CharSet=CharSet.Unicode, ExactSpelling=true, SetLastError=true)]
  private static extern uint GetFileAttributesW(string path);
  public static uint Attributes(string path) {
    uint value = GetFileAttributesW(path);
    if (value != 0xffffffff) return value;
    int error = Marshal.GetLastWin32Error();
    if (error == 2 || error == 3) return 0;
    throw new System.ComponentModel.Win32Exception(error);
  }
  [DllImport("shell32.dll", CharSet=CharSet.Unicode, ExactSpelling=true)]
  private static extern int SHGetFolderPathW(IntPtr hwnd, int csidl, IntPtr token, uint flags, StringBuilder path);
  public static string Get(int id, int flags) {
    var value = new StringBuilder(260);
    int result = SHGetFolderPathW(IntPtr.Zero, id | flags, IntPtr.Zero, 0, value);
    if (result != 0) throw new InvalidOperationException("SHGetFolderPath non-S_OK HRESULT: " + result);
    if (value.Length == 0) throw new InvalidOperationException("Empty shell folder");
    return value.ToString();
  }
}
'@
Write-FixturePhase 'add-type:complete'
Write-FixturePhase 'initial-links:start'
Assert-NoLinks $SyntheticHome
Write-FixturePhase 'initial-links:complete'
Write-FixturePhase 'initial-absence:start'
if (Test-Path -LiteralPath $SyntheticHome) { throw 'Shell preparation requires a fresh absent HOME' }
Write-FixturePhase 'initial-absence:complete'
$folders = @(
  @{ id=0x10; relative='Desktop' },
  @{ id=0x0b; relative='AppData\Roaming\Microsoft\Windows\Start Menu' },
  @{ id=0x02; relative='AppData\Roaming\Microsoft\Windows\Start Menu\Programs' }
)
# DONT_VERIFY neither creates nor initializes. Validate ALL destinations before
# any CREATE call; no registry redirection, default-user token or fallback path.
Write-FixturePhase 'probe:start'
foreach ($folder in $folders) {
  $expected = [IO.Path]::Combine($SyntheticHome, $folder.relative)
  $actual = [FixtureShellFolders]::Get($folder.id, 0x4000)
  if (-not [string]::Equals($actual, $expected, [StringComparison]::OrdinalIgnoreCase)) { throw "Shell destination outside exact synthetic path: $($folder.id)" }
  Assert-NoLinks $actual
}
if (Test-Path -LiteralPath $SyntheticHome) { throw 'Read-only shell probe initialized HOME' }
Write-FixturePhase 'probe:complete'
Write-FixturePhase 'create:start'
foreach ($folder in $folders) {
  $expected = [IO.Path]::Combine($SyntheticHome, $folder.relative)
  $actual = [FixtureShellFolders]::Get($folder.id, 0x8000)
  if (-not [string]::Equals($actual, $expected, [StringComparison]::OrdinalIgnoreCase)) { throw 'Shell destination changed during creation' }
  Assert-NoLinks $actual
}
Write-FixturePhase 'create:complete'
@{ initialized=$true; home=$SyntheticHome; csidl=@(16,11,2) } | ConvertTo-Json -Compress
