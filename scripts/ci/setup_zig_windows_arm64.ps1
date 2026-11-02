param(
  [string]$Version = $env:ZIG_VERSION
)

$ErrorActionPreference = "Stop"
if ($Version -ne "0.16.0") {
  throw "Update the pinned x86_64 Zig checksum for version $Version"
}
if (-not $env:RUNNER_TEMP -or -not $env:GITHUB_PATH -or -not $env:GITHUB_ENV) {
  throw "This helper requires the GitHub Actions runner environment."
}

# The native Windows ARM64 Zig host crashes while compiling Ghostty. Keep the
# known working x86_64 host under emulation; build.rs still targets ARM64.
$package = "zig-x86_64-windows-$Version"
$archive = Join-Path $env:RUNNER_TEMP "$package.zip"
$url = "https://ziglang.org/download/$Version/$package.zip"
Invoke-WebRequest -Uri $url -OutFile $archive -UseBasicParsing
$expectedSha256 = "68659eb5f1e4eb1437a722f1dd889c5a322c9954607f5edcf337bc3684a75a7e"
$actualSha256 = (Get-FileHash -Path $archive -Algorithm SHA256).Hash.ToLowerInvariant()
if ($actualSha256 -ne $expectedSha256) {
  throw "Zig archive checksum mismatch: expected $expectedSha256, got $actualSha256"
}

Expand-Archive -Path $archive -DestinationPath $env:RUNNER_TEMP -Force
$zigDir = Join-Path $env:RUNNER_TEMP $package
$zigExe = Join-Path $zigDir "zig.exe"
if (-not (Test-Path $zigExe)) {
  throw "Zig executable not found: $zigExe"
}

# The pinned standard library also needs the compiler diagnostic's alignment
# assertion when loading an ARM64 function pointer. Fail if that source changes.
$selfInfo = Join-Path $zigDir "lib\std\debug\SelfInfo\Windows.zig"
$source = [IO.File]::ReadAllText($selfInfo)
$before = '@ptrCast(try si.loadNtdllProc(@tagName(proc)))'
$after = '@ptrCast(@alignCast(try si.loadNtdllProc(@tagName(proc))))'
$matches = ([regex]::Matches($source, [regex]::Escape($before))).Count
if ($matches -ne 1) {
  throw "Expected one Zig SelfInfo alignment patch target, found $matches"
}
[IO.File]::WriteAllText(
  $selfInfo,
  $source.Replace($before, $after),
  [Text.UTF8Encoding]::new($false)
)

Add-Content -Path $env:GITHUB_PATH -Value $zigDir
Add-Content -Path $env:GITHUB_ENV -Value "NAUTERM_ZIG=$zigExe"
& $zigExe version
if ($LASTEXITCODE -ne 0) {
  throw "Zig version check failed with exit code $LASTEXITCODE"
}
