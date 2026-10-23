[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$rust = @(& cargo run --quiet --manifest-path (Join-Path $root 'spikes\spike_e1_rust\Cargo.toml') --example e21_configuration_probe)
if ($LASTEXITCODE -ne 0) { throw 'Rust configuration probe failed.' }
$project = Join-Path $PSScriptRoot 'csharp\PacketLens.E1Evaluation.csproj'
& dotnet build $project --no-restore --verbosity quiet
if ($LASTEXITCODE -ne 0) { throw 'C# configuration probe build failed.' }
$csharp = @(& dotnet run --project $project --no-build --no-restore -- e21-configuration)
if ($LASTEXITCODE -ne 0) { throw 'C# configuration probe failed.' }
if ($rust.Count -ne 69 -or $csharp.Count -ne 69) { throw 'Expected 69 configuration observations per engine.' }
if (($rust -join "`n") -cne ($csharp -join "`n")) { throw 'Rust and C# configuration observations differ.' }
[pscustomobject]@{ Observations = $rust.Count; IndependentRunsPerEngine = 100; SemanticallyEqual = $true }
