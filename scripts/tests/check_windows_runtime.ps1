#requires -Version 7.0
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot '../ci/windows_runtime.ps1')
$fixture = Join-Path ([IO.Path]::GetTempPath()) ('nauterm-crt-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $fixture | Out-Null
function Write-PeFixture([string]$Directory, [string]$Name, [uint16]$Machine) {
  New-Item -ItemType Directory -Path $Directory -Force | Out-Null
  $bytes = [byte[]]::new(70)
  $bytes[0] = 0x4d; $bytes[1] = 0x5a; $bytes[60] = 64
  $bytes[64] = 0x50; $bytes[65] = 0x45
  [BitConverter]::GetBytes($Machine).CopyTo($bytes, 68)
  [IO.File]::WriteAllBytes((Join-Path $Directory $Name), $bytes)
}
try {
  $names = @('vcruntime140.dll', 'msvcp140.dll')
  $wrong = Join-Path $fixture 'host-x64'
  $partial = Join-Path $fixture 'incomplete-arm64'
  $matching = Join-Path $fixture 'target-arm64'
  foreach ($name in $names) {
    Write-PeFixture $wrong $name 0x8664
    Write-PeFixture $matching $name 0xaa64
  }
  Write-PeFixture $partial $names[0] 0xaa64
  $actual = Find-WindowsVcRuntime -Architecture arm64 -DllNames $names -Candidates @($wrong, $partial, $matching)
  if ($actual -ne $matching) { throw 'Selected incomplete or incorrect-architecture CRT.' }
  $rejected = $false
  try { Find-WindowsVcRuntime -Architecture arm64 -DllNames $names -Candidates @($wrong, $partial) }
  catch { $rejected = $true }
  if (-not $rejected) { throw 'Packaging accepted an incomplete ARM64 runtime.' }
  $invalid = Join-Path $fixture 'invalid.dll'
  [IO.File]::WriteAllBytes($invalid, [byte[]]@(0x4d, 0x5a))
  if ($null -ne (Get-WindowsPeArchitecture $invalid)) { throw 'Accepted truncated PE.' }
  Write-Host 'Windows runtime selection checks passed (host mismatch, incomplete CRT, malformed PE).'
} finally {
  # This path is a freshly created, explicitly named fixture under TEMP.
  $resolved = [IO.Path]::GetFullPath($fixture)
  $temporaryRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
  if (-not $resolved.StartsWith($temporaryRoot, [StringComparison]::OrdinalIgnoreCase) -or
      -not ([IO.Path]::GetFileName($resolved)).StartsWith('nauterm-crt-')) { throw 'Unexpected fixture path.' }
  Remove-Item -LiteralPath $resolved -Recurse -Force
}
