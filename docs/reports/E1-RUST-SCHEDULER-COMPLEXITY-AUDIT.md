# Auditoria acotada: scheduler Rust E1

Fecha de verificacion: 2026-10-19.

## Alcance y estado inicial

Se audito exclusivamente `spikes/spike_e1_rust`, comenzando por
`Engine::has_active_work()` y sin alterar protocolos, contratos publicos,
`ScenarioCanonicalReport`, `presentation/v1`, ADRs ni las especificaciones.

El arbol de trabajo ya tenia cambios locales sin commit antes de esta auditoria,
incluidos cambios previos en `src/lib.rs`, `presentation.rs`, los spikes C# y
documentacion. No se restauraron ni sobrescribieron. Esta auditoria solo agrega
el indice interno del scheduler, sus pruebas de caracterizacion y este reporte.

## Hallazgos y candidatos

| Punto | Complejidad antes | Coste observado / alcance | Decicion |
| --- | --- | --- | --- |
| `has_active_work()` | `O(Q)` por llamada, al recorrer `BinaryHeap`; `Q` es la cola pendiente. | Demostrado abajo cuando hay temporizadores pasivos anteriores a trabajo activo. El iterador puede detenerse pronto si el minimo ya es activo, por lo que no se asumio que afectara todos los escenarios. | Optimizado. |
| `switch_received_frame()` | Inundacion: `O(P)` para seleccionar puertos y `O(P log Q)` al encolar, con clonacion de la PDU por egreso. | E1 ejercita topologias pequenas; esta auditoria no introdujo una topologia de puertos masiva ni hallo evidencia de que sea el cuello de botella medido. Cambiarlo arriesgaria orden/duplicacion de tramas. | Sin cambio. |
| Tablas `BTreeMap` (interfaces, enlaces, ARP y CAM) | Busquedas/actualizaciones `O(log N)`; la actualizacion ARP/CAM tambien programa un temporizador. | En E1-S2 son tablas pequenas. El benchmark de 64 tramas activas mide parte de ese camino y no aislaba una ventaja segura de cambiar la estructura. | Sin cambio. |
| `get_trace_snapshot()` | `O(R)` tiempo y espacio adicional por clonacion de `R` trazas retenidas. | No se invoca dentro de `run_until_quiescent`; la proyeccion E1-S2 fria retenida tiene 27 eventos. Es relevante para exportacion/UI, no para el bucle auditado. | Sin cambio. |

La primera fila si tiene una ruta cuadratica: si `P` temporizadores pasivos con
fecha anterior quedan antes de un evento activo, el bucle procesa cada pasivo
para llegar al activo y vuelve a escanear aproximadamente la cola restante.
El coste adicional puede ser `O(P^2)`, aparte de los `pop`/`push` normales del
heap. No ocurre necesariamente cuando la raiz ya es trabajo activo.

## Benchmark reproducible

Comando, desde `spikes/spike_e1_rust`:

```powershell
& 'C:\Users\familia\.cargo\bin\cargo.exe' test --release tests::benchmark_run_until_quiescent_scheduler_scaling -- --ignored --exact --nocapture --test-threads=1
```

El benchmark es el test ignorado `benchmark_run_until_quiescent_scheduler_scaling`.
Para cada caso crea una topologia E1 valida, encola `P` temporizadores
`MacEntryExpireTimer` pasivos con fechas anteriores y despues 1 o 64 tramas
activas espaciadas. Ejecuta siete muestras consecutivas y muestra la mediana.
Los temporizadores pasivos usan un nodo ausente y no emiten trazas; permiten
aislar el coste de quiescencia sin anadir un protocolo ni un escenario
normativo. `processed_events` confirma que los dos programas comparados
procesaron el mismo trabajo.

Memoria es la diferencia de bytes de heap vivos, medida por el asignador de
prueba `#[cfg(test)]` inmediatamente despues de encolar respecto de la misma
topologia vacia. Incluye el heap y las cadenas de las entradas de prueba; no es
working set, RSS ni memoria del proceso. `ScheduledEvent` mide 112 bytes en
esta compilacion. El asignador no se compila en el crate de produccion.

Entorno observado: Windows 10.0.26200 x64, `PROCESSOR_IDENTIFIER=Intel64
Family 6 Model 151 Stepping 2`, `rustc 1.99.1`, `cargo 1.99.1`, perfil
`--release`. La consulta CIM de CPU/RAM fue denegada por el entorno, por lo que
no se atribuyen estos resultados a un modelo comercial ni a una cantidad de RAM.
No hubo afinidad de CPU, aislamiento de procesos ni perfilador de instrucciones.

| Carga | Eventos procesados | Heap vivo en cola (antes/despues) | Mediana antes ns | Mediana despues ns | Factor / reduccion |
| --- | ---: | ---: | ---: | ---: | ---: |
| 1 activa, 0 pasivos | 10 | 455 / 455 | 3.900 | 4.400 | 0,89x / -12,8 % |
| 1 activa, 512 pasivos | 522 | 117.767 / 117.767 | 85.700 | 32.400 | 2,65x / 62,2 % |
| 1 activa, 2.048 pasivos | 2.058 | 471.047 / 471.047 | 934.900 | 133.200 | 7,02x / 85,8 % |
| 1 activa, 8.192 pasivos | 8.202 | 1.884.167 / 1.884.167 | 13.562.700 | 953.600 | 14,22x / 93,0 % |
| 64 activas, 0 pasivos | 640 | 7.616 / 7.616 | 278.100 | 273.500 | 1,02x / 1,7 % |
| 64 activas, 8.192 pasivos | 8.832 | 1.884.608 / 1.884.608 | 9.643.500 | 1.178.200 | 8,18x / 87,8 % |

Las dos columnas temporales son medianas de siete muestras en el mismo host y
con el mismo comando. El caso sin pasivos muestra ruido normal de sub-5 us: no
se reclama mejora ahi. La mejora solo se atribuye a la forma previamente
medida de trabajo repetitivo.

## Cambio aplicado

`Engine` conserva ahora `active_scheduled_events`, un contador privado de los
eventos pendientes que pueden impedir `Quiescent`:

- `TransmitFrame` y `FrameArrived` se contabilizan al ser aceptados por el
  scheduler, incluso si al ejecutarse terminan en un descarte observable.
- Un `ArpResolveRetryTimer` se contabiliza solo si coincide con la resolucion
  ARP vigente (epoca e intento). Al resolver ARP, se retira la contribucion del
  retry que queda obsoleto, pero el evento permanece en el heap para conservar
  su semantica cuando una operacion posterior avance el reloj.
- Los temporizadores de expiracion ARP/CAM permanecen pasivos, igual que en el
  predicado anterior.
- Al extraer un evento, se retira su contribucion antes de manejarlo. Si un
  retry activo programa el siguiente, el contador se repone mediante el mismo
  camino de `schedule`.

Por tanto `has_active_work()` pasa de recorrer el heap a `O(1)`. El orden de
`BinaryHeap` por `(at, sequence)`, los `push`/`pop` `O(log Q)`, el reloj virtual,
las trazas y la cola fisica no cambian. El coste espacial adicional es un
`usize` por motor; el benchmark confirma que el tamaño de `ScheduledEvent` y la
memoria de cola no cambiaron.

## TDD: RED, GREEN y REFACTOR

1. **RED.** Se agrego `active_work_index_ignores_passive_and_obsolete_timers`
   antes del campo. El comando
   `cargo test tests::active_work_index_ignores_passive_and_obsolete_timers -- --exact`
   fallo en compilacion con tres `E0609`: no existia
   `Engine::active_scheduled_events`. Este fue un fallo de compilacion esperado,
   no una asercion funcional fallida.
2. **GREEN.** Se implemento el contador y se ejecuto el mismo comando: una
   prueba aprobada. La prueba exige `Quiescent` a 8.700 ns, deja temporizadores
   fisicamente pendientes y exige que no cuenten como trabajo activo.
3. **REFACTOR/verificacion.** Se centralizo la clasificacion previa en
   `event_is_active`, se mantuvo la resta con `checked_sub`, y se amplio la
   prueba de capacidad para demostrar que el rechazo de una trama activa cuando
   la cola esta llena no muta el indice. El test
   `active_work_index_matches_the_scan_during_arp_resolution` contrasta, antes
   de cada paso de una resolucion ARP con 32 temporizadores pasivos, el contador
   contra el predicado de escaneo anterior.

## Verificaciones ejecutadas

| Verificacion | Resultado observado |
| --- | --- |
| `cargo fmt --check` | Correcto. |
| `cargo test` en `spikes/spike_e1_rust` | 18 aprobadas, 0 fallidas, 1 benchmark ignorado; 3 pruebas de contrato y 2 de exportacion aprobadas; 0 doc-tests. Incluye 100 ejecuciones deterministas del reporte canonico. |
| Benchmark Release anterior y posterior | Ejecutado; siete muestras por caso, tabla anterior. |
| `dotnet test tests\PacketLens.SpikeE1.Tests\PacketLens.SpikeE1.Tests.csproj --no-restore` en `spikes/spike_e1_csharp` | 24 aprobadas, 0 fallidas, 0 omitidas. Incluye `E1S2_CanonicalReport_IsIdenticalAcrossOneHundredIndependentRuns` y `E1S2Cold_ExportIsDeterministic`. |
| `Export-PresentationFixture.ps1 -Mode Check` | Fixture generado por Rust valido: `presentation/v1`, E1-S2 frio, 27 eventos, 8.700 ns, 15.334 bytes. |
| `Compare-PresentationS2.ps1` | Rust y C# byte a byte identicos; fixture coincide con Rust. |

Se intento adicionalmente `cargo clippy --all-targets --all-features -- -D
warnings`. Fallo por tres advertencias convertidas en error fuera de esta
optimizacion: dos `collapsible_if` en `src/presentation.rs` y
`chunks_exact_to_as_chunks` en la implementacion SHA-256 ya existente de
`src/lib.rs`. No se modificaron por ser una auditoria de scheduler; no es un
fallo de las pruebas funcionales listadas arriba.

La herramienta estatica de emparejamiento de fuentes/pruebas tampoco pudo
ejecutarse porque falta el prerequisito Python `tree-sitter-language-pack`; no
se instalo dependencia alguna para esta auditoria. Las pruebas concretas se
agregaron en el modulo unitario que ya prueba los detalles privados del
scheduler.

## Riesgos pendientes

- El contador es una invariante de implementacion: todo futuro camino que
  elimine una resolucion ARP con un retry pendiente debe retirar una unidad.
  Los unicos sitios actuales de insercion/eliminacion fueron revisados y la
  prueba de escaneo los cubre para la resolucion exitosa; E1-S3A cubre los
  reintentos y el fallo final.
- El benchmark no representa una medicion de producto ni concluye nada sobre
  topologias mayores, UI, E2 o consumo de memoria del proceso.
- La medicion de tiempo no impone umbrales en CI para evitar pruebas fragiles;
  el test ignorado conserva el procedimiento reproducible y las pruebas
  funcionales protegen la correccion.

No se requiere ADR nuevo: el cambio es una optimizacion privada equivalente al
predicado existente y no altera una decision arquitectonica ni un contrato.
