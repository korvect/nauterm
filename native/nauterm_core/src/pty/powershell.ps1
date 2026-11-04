# Loaded only in the terminal's own process, after normal PowerShell profiles.
# Compatible with Windows PowerShell 5.1 and PowerShell 7.
if (Get-Variable __nautermIntegration -Scope Global -ErrorAction SilentlyContinue) {
    if ($env:NAUTERM_SHELL_INTEGRATION_TOKEN) {
        $global:__nautermToken = $env:NAUTERM_SHELL_INTEGRATION_TOKEN
    }
    return
}

$global:__nautermToken = $env:NAUTERM_SHELL_INTEGRATION_TOKEN
if (!$global:__nautermToken) { $global:__nautermToken = [string]$PID }
$global:__nautermCommandActive = $false
$global:__nautermAiArmed = $false
$global:__nautermOriginalPrompt = $function:prompt

function global:__nautermOsc([string]$Payload) {
    [Console]::Write(([string][char]27) + ']' + $Payload + ([string][char]7))
}

# Use the existing reader so PSReadLine history, completion and custom key
# bindings continue to work. No history file is polled or rewritten.
Import-Module PSReadLine -ErrorAction SilentlyContinue
$__nautermReader = Get-Command PSConsoleHostReadLine -CommandType Function -ErrorAction SilentlyContinue
$global:__nautermOriginalReadLine = if ($__nautermReader) { $__nautermReader.ScriptBlock } else { $null }
if ($global:__nautermOriginalReadLine) {
    function global:PSConsoleHostReadLine {
        $line = & $global:__nautermOriginalReadLine
        if (![string]::IsNullOrWhiteSpace($line)) {
            $encoded = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($line))
            __nautermOsc ('4545;CommandStarted;' + $PID + ';' + $encoded)
            __nautermOsc ('133;C;aid=' + $PID)
            $global:__nautermCommandActive = $true
        }
        return $line
    }
}

function global:prompt {
    $succeeded = $?
    $nativeStatus = Get-Variable LASTEXITCODE -Scope Global -ValueOnly -ErrorAction SilentlyContinue
    if ($global:__nautermCommandActive) {
        $status = 0
        if (!$succeeded) {
            $status = 1
            if ($null -ne $nativeStatus -and $nativeStatus -ne 0) { $status = $nativeStatus }
        }
        __nautermOsc ('4545;CommandExited;' + $PID + ';' + $status)
        __nautermOsc ('133;D;' + $status + ';aid=' + $PID)
        $global:__nautermCommandActive = $false
        if ($global:__nautermAiArmed) {
            __nautermOsc ('777;nauterm-command-end=' + $global:__nautermToken + ';' + $status)
            $global:__nautermAiArmed = $false
        }
    }
    if ($PWD.Provider.Name -eq 'FileSystem') {
        $directory = [Uri]::new($PWD.ProviderPath, [UriKind]::Absolute).AbsoluteUri
        __nautermOsc ('7;' + $directory)
    }
    __nautermOsc ('133;A;cl=line;aid=' + $PID)
    $text = if ($global:__nautermOriginalPrompt) { & $global:__nautermOriginalPrompt } else { 'PS ' + $PWD + '> ' }
    $global:LASTEXITCODE = $nativeStatus
    return ($text + ([string][char]27) + ']133;B;aid=' + $PID + ([string][char]7))
}

if (Get-Command Set-PSReadLineKeyHandler -ErrorAction SilentlyContinue) {
    Set-PSReadLineKeyHandler -Chord 'Ctrl+x,Ctrl+u' -BriefDescription 'Nauterm command' -ScriptBlock {
        [Microsoft.PowerShell.PSConsoleReadLine]::RevertLine()
        $global:__nautermAiArmed = $true
        __nautermOsc ('777;nauterm-line-ready=' + $global:__nautermToken)
    }
}
Remove-Variable __nautermReader -ErrorAction SilentlyContinue
$global:__nautermIntegration = $true
