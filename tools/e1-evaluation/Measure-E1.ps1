Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
$rustExe = (Resolve-Path (Join-Path $PSScriptRoot 'rust\target\release\packetlens_e1_evaluation_rust.exe')).Path
$csharpDll = (Resolve-Path (Join-Path $PSScriptRoot 'csharp\bin\Release\net10.0\PacketLens.E1Evaluation.dll')).Path
$dotnet = (Get-Command dotnet -ErrorAction Stop).Source

function Invoke-SampledWorkload {
    param(
        [Parameter(Mandatory)][ValidateSet('Rust', 'CSharp')][string]$Engine,
        [Parameter(Mandatory)][ValidateSet('s2-cold', 's6')][string]$Workload,
        [Parameter(Mandatory)][int]$Iterations,
        [Parameter(Mandatory)][int]$Run
    )

    $outputFile = Join-Path ([IO.Path]::GetTempPath()) "packetlens-e1-$Engine-$Workload-$Run-$([guid]::NewGuid().ToString('N')).txt"
    if ($Engine -eq 'Rust') {
        $filePath = $rustExe
        $arguments = @('workload', $Workload, $Iterations)
    }
    else {
        $filePath = $dotnet
        $arguments = @($csharpDll, 'workload', $Workload, $Iterations)
    }

    $wallClock = [Diagnostics.Stopwatch]::StartNew()
    $process = Start-Process -FilePath $filePath -ArgumentList $arguments -PassThru -NoNewWindow -RedirectStandardOutput $outputFile
    $peakWorkingSet = 0L
    $peakSampledPrivate = 0L
    while (-not $process.HasExited) {
        try {
            $process.Refresh()
            $peakWorkingSet = [Math]::Max($peakWorkingSet, $process.WorkingSet64)
            $peakSampledPrivate = [Math]::Max($peakSampledPrivate, $process.PrivateMemorySize64)
        }
        catch {
            # The process can exit between HasExited and Refresh.
        }
        Start-Sleep -Milliseconds 10
    }
    $process.WaitForExit()
    $wallClock.Stop()

    $output = (Get-Content -Path $outputFile -Raw).Trim()
    if ($output -notmatch '^checksum=(\d+);elapsed_ns=(\d+)$') {
        throw "Unexpected $Engine/$Workload output: $output"
    }

    [pscustomobject]@{
        Engine = $Engine
        Workload = $Workload
        Iterations = $Iterations
        Run = $Run
        Checksum = [uint64]$Matches[1]
        EngineElapsedMs = [math]::Round(([double]$Matches[2]) / 1000000, 3)
        ProcessElapsedMs = [math]::Round($wallClock.Elapsed.TotalMilliseconds, 3)
        PeakWorkingSetMiB = [math]::Round($peakWorkingSet / 1MB, 3)
        PeakSampledPrivateMiB = [math]::Round($peakSampledPrivate / 1MB, 3)
    }
}

$cases = @(
    [pscustomobject]@{ Workload = 's2-cold'; Iterations = 100000 },
    [pscustomobject]@{ Workload = 's6'; Iterations = 5000 }
)
$results = foreach ($case in $cases) {
    foreach ($engine in 'Rust', 'CSharp') {
        foreach ($run in 1..3) {
            Invoke-SampledWorkload -Engine $engine -Workload $case.Workload -Iterations $case.Iterations -Run $run
        }
    }
}

$results | Format-Table -AutoSize
$results |
    Group-Object Engine, Workload, Iterations |
    ForEach-Object {
        $group = $_.Group
        [pscustomobject]@{
            Engine = $group[0].Engine
            Workload = $group[0].Workload
            Iterations = $group[0].Iterations
            MedianEngineMs = ($group.EngineElapsedMs | Sort-Object)[1]
            MedianProcessMs = ($group.ProcessElapsedMs | Sort-Object)[1]
            MedianPeakWorkingSetMiB = ($group.PeakWorkingSetMiB | Sort-Object)[1]
            MedianPeakSampledPrivateMiB = ($group.PeakSampledPrivateMiB | Sort-Object)[1]
        }
    } |
    Sort-Object Workload, Engine |
    Format-Table -AutoSize
