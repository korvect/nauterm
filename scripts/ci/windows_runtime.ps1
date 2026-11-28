# Shared by packaging and its local fixture check. Never package host-architecture
# CRT DLLs merely because System32 happens to contain a similarly named file.
function Get-WindowsPeArchitecture {
  param([Parameter(Mandatory)][string]$Path)
  $stream = [IO.File]::OpenRead($Path)
  $reader = [IO.BinaryReader]::new($stream)
  try {
    if ($reader.ReadUInt16() -ne 0x5a4d -or $stream.Length -lt 64) { return $null }
    $stream.Position = 60
    $offset = $reader.ReadUInt32()
    if ($offset -gt $stream.Length - 6) { return $null }
    $stream.Position = $offset
    if ($reader.ReadUInt32() -ne 0x4550) { return $null }
    switch ($reader.ReadUInt16()) {
      0x8664 { return 'x64' }
      0xaa64 { return 'arm64' }
      default { return $null }
    }
  } catch [IO.EndOfStreamException] {
    return $null
  } finally {
    $reader.Dispose()
    $stream.Dispose()
  }
}

function Find-WindowsVcRuntime {
  param(
    [Parameter(Mandatory)][ValidateSet('x64', 'arm64')][string]$Architecture,
    [Parameter(Mandatory)][string[]]$DllNames,
    [string[]]$Candidates
  )
  if (-not $PSBoundParameters.ContainsKey('Candidates')) {
    $roots = [Collections.Generic.List[string]]::new()
    if ($env:VCToolsRedistDir) { $roots.Add($env:VCToolsRedistDir) }
    if ($env:VSINSTALLDIR) { $roots.Add((Join-Path $env:VSINSTALLDIR 'VC\Redist\MSVC')) }
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (Test-Path -LiteralPath $vswhere) {
      foreach ($installation in (& $vswhere -all -products '*' -property installationPath)) {
        if ($installation) { $roots.Add((Join-Path $installation 'VC\Redist\MSVC')) }
      }
    }
    $Candidates = @(
      foreach ($root in $roots) {
        foreach ($pattern in @("$Architecture\Microsoft.VC*.CRT", "*\$Architecture\Microsoft.VC*.CRT")) {
          Get-ChildItem -Path (Join-Path $root $pattern) -Directory -ErrorAction SilentlyContinue |
            Sort-Object FullName -Descending | Select-Object -ExpandProperty FullName
        }
      }
      Join-Path $env:SystemRoot 'System32'
    )
  }
  foreach ($directory in ($Candidates | Select-Object -Unique)) {
    $valid = $true
    foreach ($dll in $DllNames) {
      $path = Join-Path $directory $dll
      if (-not (Test-Path -LiteralPath $path -PathType Leaf) -or
          (Get-WindowsPeArchitecture -Path $path) -ne $Architecture) {
        $valid = $false
        break
      }
    }
    if ($valid) { return $directory }
  }
  throw "A complete $Architecture Visual C++ runtime was not found. Install the matching MSVC redist or configure VCToolsRedistDir."
}
