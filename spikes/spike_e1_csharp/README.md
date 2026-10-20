# Spike E1 en C#

Implementación headless independiente de E1, organizada en `Domain` y
`Application`; no reutiliza el motor Rust. Ejecutar desde esta carpeta:

```powershell
dotnet test tests/PacketLens.SpikeE1.Tests/PacketLens.SpikeE1.Tests.csproj
dotnet build tests/PacketLens.SpikeE1.Tests/PacketLens.SpikeE1.Tests.csproj --no-incremental
```

La bitácora del ciclo RED → GREEN → REFACTOR está en
[`docs/reports/E1-CSHARP-TDD-LOG.md`](../../docs/reports/E1-CSHARP-TDD-LOG.md).
La comparacion ejecutada con Rust esta en
[`docs/reports/E1-CROSS-SPIKE-COMPARISON.md`](../../docs/reports/E1-CROSS-SPIKE-COMPARISON.md).
