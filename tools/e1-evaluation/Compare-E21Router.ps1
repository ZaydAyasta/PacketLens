[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$rust = @(& cargo run --quiet --manifest-path (Join-Path $root 'spikes\spike_e1_rust\Cargo.toml') --example e21_router_probe)
if ($LASTEXITCODE -ne 0) { throw 'Rust router probe failed.' }
$project = Join-Path $PSScriptRoot 'csharp\PacketLens.E1Evaluation.csproj'
& dotnet build $project --no-restore --verbosity quiet
if ($LASTEXITCODE -ne 0) { throw 'C# router probe build failed.' }
$csharp = @(& dotnet run --project $project --no-build --no-restore -- e21-router)
if ($LASTEXITCODE -ne 0) { throw 'C# router probe failed.' }
if ($rust.Count -eq 0 -or $rust.Count -ne $csharp.Count) { throw 'Router observation counts differ or are empty.' }
for ($i=0; $i -lt $rust.Count; $i++) {
    if ($rust[$i] -cne $csharp[$i]) { throw "Router semantic difference at row $i : Rust=$($rust[$i]); C#=$($csharp[$i])" }
}
[pscustomobject]@{ Observations = $rust.Count; IndependentRunsPerEngine = 100; SemanticallyEqual = $true }
