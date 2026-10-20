[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
$env:Path = "$cargoBin;$env:Path"

Push-Location (Join-Path $PSScriptRoot 'rust')
try {
    $rust = @(& cargo run --quiet -- presentation-s2-cold)
    if ($LASTEXITCODE -ne 0) { throw "El exportador Rust terminÃ³ con cÃ³digo $LASTEXITCODE." }
}
finally {
    Pop-Location
}

$csharpProject = Join-Path $PSScriptRoot 'csharp\PacketLens.E1Evaluation.csproj'
$csharp = @(& dotnet run --project $csharpProject --no-restore -- presentation-s2-cold)
if ($LASTEXITCODE -ne 0) { throw "El exportador C# terminÃ³ con cÃ³digo $LASTEXITCODE." }

if ($rust.Count -ne 1 -or $csharp.Count -ne 1) {
    throw 'Cada exportador debe emitir exactamente un documento JSON.'
}
if ($rust[0] -cne $csharp[0]) {
    throw 'Las proyecciones presentation/v1 de Rust y C# difieren.'
}

$fixturePath = Join-Path $repositoryRoot 'apps\packetlens-e1-vertical\src\fixtures\e1-s2-cold.presentation.json'
$expectedFixtureText = "$($rust[0])`n"
if (-not (Test-Path -LiteralPath $fixturePath)) {
    throw "No existe el fixture esperado: $fixturePath"
}
if ([System.IO.File]::ReadAllText($fixturePath) -cne $expectedFixtureText) {
    throw 'El fixture no coincide byte a byte con la salida del exportador Rust.'
}

[pscustomobject]@{
    SchemaVersion = 'presentation/v1'
    Scenario = 'E1-S2 cold'
    ByteIdentical = $true
    FixtureMatchesRust = $true
    Bytes = [Text.Encoding]::UTF8.GetByteCount($rust[0])
}
