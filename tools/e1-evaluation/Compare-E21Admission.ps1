[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$rustOutput = @(& cargo test --quiet --manifest-path (Join-Path $root 'spikes\spike_e1_rust\Cargo.toml') --lib router_protocol::admission_tests::s8_matrix_is_deterministic_across_one_hundred_reconstructions_and_order_variants -- --exact --nocapture)
if ($LASTEXITCODE -ne 0) { $rustOutput | Write-Output; throw 'Rust admission probe failed.' }
$project = Join-Path $root 'spikes\spike_e1_csharp\tests\PacketLens.SpikeE1.Tests\PacketLens.SpikeE1.Tests.csproj'
$csharpOutput = @(& dotnet test $project --no-restore --filter 'FullyQualifiedName=PacketLens.SpikeE1.Tests.E21RouterAdmissionTests.S8MatrixIsDeterministicAcrossOneHundredReconstructionsAndOrderVariants' --logger 'console;verbosity=detailed')
if ($LASTEXITCODE -ne 0) { $csharpOutput | Write-Output; throw 'C# admission probe failed.' }
$rust = @($rustOutput | ForEach-Object { if ($_ -match '(ADM5\|.*)$') { $Matches[1] } })
$csharp = @($csharpOutput | ForEach-Object { if ($_ -match '(ADM5\|.*)$') { $Matches[1] } })
if ($rust.Count -eq 0 -or $rust.Count -ne $csharp.Count) { throw "Empty/different row counts: $($rust.Count) / $($csharp.Count)." }
for ($i=0; $i -lt $rust.Count; $i++) { if ($rust[$i] -cne $csharp[$i]) { throw "Admission difference at row $i : Rust=$($rust[$i]); C#=$($csharp[$i])" } }
$cases = @($rust | Where-Object { $_ -match '\|result\|' }).Count
[pscustomobject]@{ Observations=$rust.Count; Cases=$cases; ReconstructionsPerCasePerEngine=100; SemanticallyEqual=$true }
