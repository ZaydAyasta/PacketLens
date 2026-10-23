[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$rust = @(& cargo run --quiet --manifest-path (Join-Path $root 'spikes\spike_e1_rust\Cargo.toml') --example e21_s1_probe)
if ($LASTEXITCODE -ne 0) { throw 'Rust S1 probe failed.' }
$project = Join-Path $PSScriptRoot 'csharp\PacketLens.E1Evaluation.csproj'
& dotnet build $project --no-restore --verbosity quiet
if ($LASTEXITCODE -ne 0) { throw 'C# S1 probe build failed.' }
$csharp = @(& dotnet run --project $project --no-build --no-restore -- e21-s1)
if ($LASTEXITCODE -ne 0) { throw 'C# S1 probe failed.' }
if ($rust.Count -eq 0 -or $rust.Count -ne $csharp.Count) { throw 'S1 observation counts differ or are empty.' }
for ($i=0; $i -lt $rust.Count; $i++) {
    if ($rust[$i] -cne $csharp[$i]) { throw "S1 semantic difference at row $i : Rust=$($rust[$i]); C#=$($csharp[$i])" }
}
[pscustomobject]@{ Observations = $rust.Count; Scenarios = 'cold,warm-reverse,concurrent'; IndependentRunsPerEngine = 100; SemanticallyEqual = $true }
