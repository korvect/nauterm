# Isolated protocol fixture: no profile, history file, application database,
# credential store, real external command or network connection is used.
$ErrorActionPreference = 'Stop'
$global:__nautermOriginalReadLine = { "Write-Output '中文'" }
$originalOutput = [Console]::Out
$capture = [IO.StringWriter]::new()
[Console]::SetOut($capture)
try {
    $initialPrompt = prompt
    if (!$initialPrompt.StartsWith('fixture> ')) { throw 'Original prompt was replaced' }
    $line = PSConsoleHostReadLine
    if ($line -ne "Write-Output '中文'") { throw 'The existing reader was not preserved' }
    $global:__nautermAiArmed = $true
    $global:LASTEXITCODE = 23
    Write-Error 'synthetic command failure' -ErrorAction SilentlyContinue
    $failedPrompt = prompt
    if ($global:LASTEXITCODE -ne 23) { throw 'Prompt changed LASTEXITCODE' }
    $line = PSConsoleHostReadLine
    $global:__nautermAiArmed = $true
    $null = 1 + 1
    $successfulPrompt = prompt
    $data = $capture.ToString()
    $escape = [string][char]27
    $bell = [string][char]7
    $encoded = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes("Write-Output '中文'"))
    foreach ($expected in @(
        ($escape + ']4545;CommandStarted;' + $PID + ';' + $encoded + $bell),
        ($escape + ']4545;CommandExited;' + $PID + ';23' + $bell),
        ($escape + ']4545;CommandExited;' + $PID + ';0' + $bell),
        ($escape + ']777;nauterm-command-end=fixture-token;23' + $bell),
        ($escape + ']777;nauterm-command-end=fixture-token;0' + $bell),
        ($escape + ']7;' + [Uri]::new($PWD.ProviderPath, [UriKind]::Absolute).AbsoluteUri + $bell)
    )) {
        if (!$data.Contains($expected)) { throw ('Missing protocol event: ' + $expected) }
    }
    if (!$failedPrompt.Contains(']133;B;aid=')) { throw 'Missing input boundary' }
    $key = Get-PSReadLineKeyHandler -Chord 'Ctrl+x,Ctrl+u'
    if ($key.Function -ne 'Nauterm command') { throw 'Missing AI clean-line binding' }
} finally {
    [Console]::SetOut($originalOutput)
}
[Console]::WriteLine('NAUTERM_POWERSHELL_PROTOCOL_OK')
