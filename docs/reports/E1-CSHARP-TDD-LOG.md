# Bitácora TDD — Spike E1 C#

Fecha: 2026-10-18. Proyecto: `spikes/spike_e1_csharp`.

Esta bitácora separa de forma explícita pruebas que llegaron a ejecutarse en
rojo de los RED de compilación producidos porque la API normativa aún no existía.
Un error de compilación demuestra que el contrato probado no estaba
implementado, pero **no** se contabiliza como una prueba ejecutada y fallida.

| Incremento | RED ejecutado antes de producción | GREEN ejecutado | Refactor sin cambio observable |
| --- | --- | --- | --- |
| Reloj | `VirtualClockTests.AdvanceUntil_WithoutEvents_MovesClockToExactTarget` no compiló: faltaba `SimulationEngine`. | 1/1 aprobado. | `ExecutionOutcome` y `ExecutionReport` se movieron a `Domain`; 1/1 aprobado. |
| Pasado | `VirtualClockTests.AdvanceUntil_TargetInPast_ThrowsExplicitDomainErrorWithoutMovingClock` no compiló: faltaban el error y el estado observable. | 2/2 aprobadas. | No hubo simplificación adicional útil. |
| Enlace | `LinkPropagationTests.SendPing_ArpRequestCrossesDirectLinkAfterExactlyOneThousandNanoseconds` no compiló: faltaban topología/PDU/traza. | 3/3 aprobadas. | Se encapsuló la cola de trazas dentro del motor. |
| S1 | `E1S1_ArpResolutionThroughSwitch_IsReadyAt4300Nanoseconds` no compiló: faltaban switch e introspección de tablas. | 4/4 aprobadas. | Colecciones emisoras quedaron ordenadas con `SortedDictionary`. |
| S2 | `E1S2_ColdAndWarmPing_CompleteAtNormativeTimes` no compiló: faltaba `RunUntilQuiescent`. | 5/5 aprobadas. | Separación de recepción ARP e IPv4 en métodos privados; 5/5 aprobadas. |
| S3 | Pruebas S3A/S3B no compilaron: faltaban cambio administrativo y desconexión. | 7/7 aprobadas. | Reintentos y descartes permanecieron encapsulados en eventos internos. |
| S4 | `E1S4_MigratedHost_RelearnsMacOnPort4AndPingCompletesAt24300Nanoseconds` no compiló: faltaba `MacPort`. | 8/8 aprobadas. | Sin cambio de comportamiento. |
| S5 | `E1S5_ArpUseDoesNotRenewTtlAndEntryExpiresAtExactTimestamp` no compiló: faltaba `ArpEntryExpired`. | 9/9 aprobadas. | Invalidación por época conservada al extraer timers obsoletos. |
| S6 | `E1S6_L2Loop_ExhaustsBudgetWithoutSafetyHaltAndTruncatesTrace` no compiló: faltaban capacidad configurable y `StepEvents`. | 10/10 aprobadas. | Buffer circular quedó como componente privado. |
| Cancelación | `EngineControlTests.RunUntilQuiescent_WhenCancellationIsRequested_ThrowsExplicitCancellationErrorBeforeProcessing` no compiló: faltaba control cooperativo. | 11/11 aprobadas. | Sin cambio adicional. |
| Epoch CAM | `MacExpiryTimer_WithCurrentEpoch_RemovesOnlyTheExpiredCamEntry` **ejecutó y falló**: esperaba 1 entrada CAM y observó 2. | Se implementó expiración por época; 12/12 aprobadas. | Comparadores de MAC/IP se reescribieron sin arreglos temporales; 12/12 aprobadas. |

El RED de expiración CAM es evidencia directa de prueba ejecutada y fallida por
comportamiento incompleto. Los RED previos son evidencia de pruebas escritas
antes de la API, pero no se presentan como fallos de ejecución.

## Ejecuciones finales observadas

- `dotnet test tests/PacketLens.SpikeE1.Tests/PacketLens.SpikeE1.Tests.csproj`:
  12 aprobadas, 0 fallidas, 0 omitidas.
- `dotnet format ... --verify-no-changes`: no pudo iniciar BuildHost por
  `UnauthorizedAccessException` al conectar una named pipe. No modifica ni
  invalida el resultado de `dotnet test`, pero impide declarar una validación de
  formato limpia.

## Actualizacion posterior de TDD

Los siguientes ciclos se ejecutaron despues de la primera bitacora. Los RED de
compilacion indican una API ausente; los RED de ejecucion registran el fallo
exacto observado antes de la implementacion.

| Incremento | RED | GREEN y refactor |
| --- | --- | --- |
| TraceSeq | Compilacion: `TraceEvent.Sequence` ausente. | S6 aprobo con `TraceSequence`; se evito la colision con la secuencia ICMP. |
| Tiempo cero | Compilacion: faltaban contrato, limite y sonda interna. | El evento 1.001 devuelve `HaltedBySafetyGuard(ZeroDelayLoop)`; se concentro la extraccion en `ProcessEvent`. |
| Capacidad | Ejecucion: esperaba `HaltedBySafetyGuard`, recibio `BudgetExhausted` al solicitar el evento 100.001. | Devuelve `QueueCapacityExceeded { pending_count: 100001, limit: 100000 }`; se conserva el conteo procesado. |
| Overflow temporal | Ejecucion: se recibio `System.OverflowException`. | `TimeOverflow` antes de mutar la cola ARP o la traza; las sumas usan `AddTime`. |
| Overflow de secuencia | Ejecucion: se recibio `System.OverflowException`. | `SequenceOverflow` sin encolar; incremento centralizado en `NextSequence`. |
| ARP no dirigido | Ejecucion: S1 no encontro el descarte de `pc3` a 2.100 ns. | Emite `ArpTargetIpMismatch` con la trama completa. |
| Reporte canonico | Compilacion: `ScenarioCanonicalReport` no existia. Luego ejecucion: no encontro el formato plano de registro. | 100 ejecuciones independientes de E1-S2 dieron un unico JSON y SHA-256; ADR-0002 fija el formato. |

Resultado final ejecutado despues de estos ciclos:

- `dotnet test tests/PacketLens.SpikeE1.Tests/PacketLens.SpikeE1.Tests.csproj`:
  17 aprobadas, 0 fallidas, 0 omitidas.
- Se construyeron temporalmente E1-S2 frio con las APIs publicas de ambos
  spikes. Los JSON canonicos fueron identicos byte a byte (7.101 bytes).
