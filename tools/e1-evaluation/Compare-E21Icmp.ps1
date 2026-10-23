[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$rustOutput = @(& cargo test --quiet --manifest-path (Join-Path $root 'spikes\spike_e1_rust\Cargo.toml') --lib icmp_router_tests::icmp_cross_projection_is_deterministic -- --exact --nocapture)
if ($LASTEXITCODE -ne 0) { throw 'Rust ICMP test probe failed.' }
$project = Join-Path $root 'spikes\spike_e1_csharp\tests\PacketLens.SpikeE1.Tests\PacketLens.SpikeE1.Tests.csproj'
$csharpOutput = @(& dotnet test $project --no-restore --filter 'FullyQualifiedName=PacketLens.SpikeE1.Tests.E21RouterIcmpTests.IcmpCrossProjectionIsDeterministic' --logger 'console;verbosity=detailed')
if ($LASTEXITCODE -ne 0) { throw 'C# ICMP test probe failed.' }
$rust = @($rustOutput | ForEach-Object { if ($_ -match '(ICMP4\|.*)$') { $Matches[1] } })
$csharp = @($csharpOutput | ForEach-Object { if ($_ -match '(ICMP4\|.*)$') { $Matches[1] } })
if ($rust.Count -eq 0 -or $rust.Count -ne $csharp.Count) { throw "Empty or different observation counts: $($rust.Count) / $($csharp.Count)." }
for ($i=0; $i -lt $rust.Count; $i++) {
    if ($rust[$i] -cne $csharp[$i]) { throw "ICMP difference at row $i : Rust=$($rust[$i]); C#=$($csharp[$i])" }
}
[pscustomobject]@{ Observations=$rust.Count; Scenarios=7; ReconstructionsPerScenarioPerEngine=100; SemanticallyEqual=$true }
