[CmdletBinding()]
param(
    [ValidateSet('Check', 'Write')]
    [string]$Mode = 'Check'
)

$ErrorActionPreference = 'Stop'
$repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$fixturePath = Join-Path $repositoryRoot 'apps\packetlens-e1-vertical\src\fixtures\e1-s2-cold.presentation.json'
$cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
$env:Path = "$cargoBin;$env:Path"

Push-Location (Join-Path $PSScriptRoot 'rust')
try {
    $exported = @(& cargo run --quiet -- presentation-s2-cold)
    if ($LASTEXITCODE -ne 0) {
        throw "El exportador Rust termino con codigo $LASTEXITCODE."
    }
}
finally {
    Pop-Location
}

if ($exported.Count -ne 1) {
    throw 'El exportador Rust debe emitir exactamente un documento JSON.'
}

$json = [string]$exported[0]
try {
    $snapshot = $json | ConvertFrom-Json
}
catch {
    throw "El exportador Rust no produjo JSON valido: $($_.Exception.Message)"
}

if ($snapshot.schema_version -cne 'presentation/v1' -or
    $snapshot.scenario_id -cne 'E1-S2' -or
    $snapshot.run_label -cne 'cold' -or
    $snapshot.execution.outcome -cne 'Quiescent' -or
    $snapshot.execution.final_virtual_time_ns -cne '8700' -or
    @($snapshot.events).Count -ne 27 -or
    $snapshot.events[0].trace_seq -cne '1' -or
    $snapshot.events[26].trace_seq -cne '27') {
    throw 'El exportador Rust no produjo el fixture normativo E1-S2 frio esperado.'
}

$expectedFileText = "$json`n"
if ($Mode -eq 'Write') {
    [System.IO.File]::WriteAllText(
        $fixturePath,
        $expectedFileText,
        [System.Text.UTF8Encoding]::new($false)
    )
}
else {
    if (-not (Test-Path -LiteralPath $fixturePath)) {
        throw "No existe el fixture esperado: $fixturePath"
    }
    $actualFileText = [System.IO.File]::ReadAllText($fixturePath)
    if ($actualFileText -cne $expectedFileText) {
        throw 'El fixture no coincide byte a byte con la salida actual del exportador Rust. Ejecute Export-PresentationFixture.ps1 -Mode Write y revise el cambio.'
    }
}

[pscustomobject]@{
    Mode = $Mode
    Fixture = $fixturePath
    SchemaVersion = $snapshot.schema_version
    Scenario = "$($snapshot.scenario_id) $($snapshot.run_label)"
    EventCount = @($snapshot.events).Count
    FinalVirtualTimeNs = $snapshot.execution.final_virtual_time_ns
    Bytes = [System.Text.Encoding]::UTF8.GetByteCount($json)
}
