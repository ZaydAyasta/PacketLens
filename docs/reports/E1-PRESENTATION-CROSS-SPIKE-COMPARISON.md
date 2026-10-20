# Comparación de `presentation/v1`: E1-S2 frío

Fecha de ejecución: 2026-10-18.

## Resultado medido

Se ejecutó:

```powershell
.\tools\e1-evaluation\Compare-PresentationS2.ps1
```

La salida fue `ByteIdentical = True`, `FixtureMatchesRust = True`,
`SchemaVersion = presentation/v1`, `Scenario = E1-S2 cold` y `Bytes = 15334`.

Los dos exportadores producen el mismo documento JSON, incluida la secuencia
de 27 eventos, `Quiescent`, `8700` ns, topología, retención, PDU ARP/IPv4/ICMP,
deltas ARP/CAM, descarte ARP de `pc3` y resultado ICMP.

## Diferencias permitidas frente al contrato canónico

No se observó diferencia de presentación para E1-S2 frío. Siguen permitidas
por A3 diferencias de métricas internas del scheduler, pero no se transportan
como condición de igualdad en este DTO.

## Conformidad A1 y fixture reproducible

La resolución A1 confirma que la transmisión ARP originada por `SendPing` es
normativa a `0 ns`. Las pruebas de ambos motores ejecutan el evento programado
en `AdvanceUntil(0)` y verifican la transmisión ARP por `link1` con llegada a
`1000 ns`; no se modificó el scheduler.

Las pruebas también verifican que una transmisión diferida de `sw1:port2`,
programada para `1100 ns`, no se emite si su interfaz queda administrativamente
caída y que usa `link2-new` si el enlace actual fue reemplazado antes de su
ejecución. E1-S3B conserva la cobertura independiente de una trama que ya
estaba en vuelo: se descarta con `LinkDisconnectedInFlight` a `1000 ns`.

El fixture Vue no contiene una traza escrita a mano. El procedimiento
`Export-PresentationFixture.ps1 -Mode Write` ejecuta el exportador Rust y
sobrescribe únicamente `e1-s2-cold.presentation.json`; `-Mode Check` exige que
el archivo sea idéntico byte a byte a una nueva corrida. La comparación cruzada
incluye esa misma comprobación antes de informar igualdad Rust/C#.

ADR-0004 cierra la diferencia de observabilidad de egreso sin modificar
SPEC-E1 ni A1. En los dos motores, una interfaz de salida caída produce
exactamente un `PacketDropped(EgressInterfaceDown)` y un enlace de salida
ausente produce exactamente un `PacketDropped(EgressLinkUnavailable)`, ambos
en `1100 ns` y con la PDU Ethernet semántica original. No hay transmisión ni
entrega al destino. Si el enlace se reemplaza antes de `1100 ns`, ambos motores
transmiten por `link2-new`; la regla de trama ya en vuelo sigue siendo
`LinkDisconnectedInFlight`.

C# ahora proyecta los dos códigos como `egress_interface_down` y
`egress_link_unavailable` en `presentation/v1`, igual que Rust. La comparación
E1-S2 frío continúa byte a byte idéntica; sus 27 eventos no incluyen estos
casos de error, por lo que las pruebas A1 específicas son la evidencia
adicional de equivalencia.
