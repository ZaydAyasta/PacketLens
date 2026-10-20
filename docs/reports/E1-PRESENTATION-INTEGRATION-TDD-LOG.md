# Evidencia TDD e integración de presentación E1

Fecha: 2026-10-18. Alcance: contrato `presentation/v1`, exportadores E1-S2
frío, consumidor Vue aislado y bridge Tauri experimental.

## Incremento 0 — línea base

| Comando | Resultado |
| --- | --- |
| `cargo test` en `spikes/spike_e1_rust` | 12 pasadas, 0 fallidas. |
| `dotnet test tests\PacketLens.SpikeE1.Tests\PacketLens.SpikeE1.Tests.csproj --no-restore` | 17 pasadas, 0 fallidas. |

## Incremento 1 — contrato Rust

| Ciclo | RED observado | GREEN observado | Refactor/nota |
| --- | --- | --- | --- |
| Topología | `cargo test --test presentation_contract`: import `presentation` no resuelto. | 2/2 al introducir DTO y validación. | El primer GREEN reveló un error de compilación local (`Option` sin tipo), corregido antes de la ejecución verde. |
| JSON con decimales | método `to_json` inexistente. | 3/3. | Se centralizó el escape JSON y el orden de claves. |

## Incremento 2 — exportador Rust

| Ciclo | RED observado | GREEN observado | Refactor/nota |
| --- | --- | --- | --- |
| E1-S2 frío | `export_e1_s2_cold` no existía. | 2/2: 27 eventos, 8700 ns, retención, PDU y deltas. | El exportador solo lee `ExecutionReport` y `TraceBufferSnapshot`. |
| IPv4 encapsulado | patrón de prueba no encontró `src_ip`, `dst_ip`, `ttl` en IPv4 bajo Ethernet. | 2/2 tras conservar toda la cabecera semántica IPv4. | Corrige una pérdida de datos de presentación, no el motor. |

La primera versión de una aserción de presentación esperaba una transmisión
ARP a 100 ns y falló contra la traza medida a 0 ns. Se corrigió **la
expectativa** para conservar el hecho del motor.

## Seguimiento A1 — transmisión inicial y diferida

La resolución posterior de A1 confirmó que `SendPing` puede originar ARP a
`0 ns`; no autorizó ni requirió retrasarlo a `100 ns`.

| Motor | Primera ejecución | Resultado | Nota |
| --- | --- | --- | --- |
| Rust | La nueva aserción consultó la traza justo después de `SendPing` y falló porque `TransmitFrame(at=0)` todavía estaba en el scheduler. | GREEN: 3/3 tras ejecutar `AdvanceUntil(0)`. | Se corrigió el momento de observación de la prueba; no se modificó el motor. |
| C# | Las 3 pruebas nuevas pasaron en su primera ejecución. | GREEN: 3/3. | Son pruebas de caracterización de comportamiento ya implementado, no evidencia retrospectiva de un ciclo RED/GREEN original. |

Las pruebas verifican: ARP inicial por `link1` a `0 ns` con llegada a `1000 ns`;
supresión de la transmisión diferida si `sw1:port2` queda administrativamente
caída; y utilización del nuevo enlace activo si `link2` se sustituye antes de
la transmisión programada para `1100 ns`. E1-S3B ya verifica la regla distinta
de A1 para una trama en vuelo: `LinkDisconnectedInFlight` a `1000 ns`.

## Incremento 6 — cierre de descartes de egreso A1

ADR-0004 fija que una transmisión diferida sin salida produce una observación
`PacketDropped`, no un silenciamiento. La decisión no modifica SPEC-E1, A1,
el scheduler ni los tiempos.

| Ciclo | RED observado | GREEN observado | Refactor/nota |
| --- | --- | --- | --- |
| `EgressInterfaceDown` y `EgressLinkUnavailable` en C# | Las pruebas no compilaron: `CS0117`, ambos valores no existían en `DropReason`. | 3/3 pruebas A1 diferidas pasan tras emitir el descarte en `Transmit`. | Una sola PDU Ethernet semántica se conserva en cada descarte; el exportador C# proyecta ambos códigos. |
| Paridad Rust | No aplica RED: Rust ya emitía los dos descartes. | Las pruebas de caracterización Rust pasan para interfaz caída, enlace ausente y enlace reconectado. | Se fortalecieron a exactamente un descarte, no solo presencia. |
| Forma de aserción C# | `xUnit2031` rechazó `Where(...).Assert.Single()`. | 3/3 tras usar la sobrecarga de `Assert.Single` con predicado. | Refactor exclusivo de prueba, sin cambio funcional. |

Las pruebas concretas son `A1_DeferredTransmit_DoesNotUseAnAdminDownEgressInterface`,
`A1_DeferredTransmit_RecordsEgressLinkUnavailableWhenCurrentLinkWasRemoved`,
`a1_deferred_transmit_does_not_use_an_admin_down_egress_interface` y
`a1_deferred_transmit_records_unavailable_current_egress_link`.

## Incremento 3 — exportador C# independiente

| Ciclo | RED observado | GREEN observado | Refactor/nota |
| --- | --- | --- | --- |
| Contrato | espacio de nombres `Presentation` inexistente. | 1/1. | Se usa validación que acumula errores, diseño independiente del `Result` Rust. |
| E1-S2 frío | `E1S2ColdPresentationExporter` inexistente. | 2/2. | Una aserción `Assert.Single` falló porque hay cinco transmisiones ARP; se seleccionó la primera, que es la intención comprobable. |
| IPv4 encapsulado | propiedades IPv4 ausentes en el payload Ethernet. | 2/2 tras proyectar IP origen/destino/TTL. | No se portó código Rust. |

La comparación ejecutable posterior fue byte a byte idéntica: `15334` bytes.

## Incremento 4 — Vue

Se escribieron primero pruebas Vitest para validación, filtro, selección por
`trace_seq`, cursor visual, PDU, retención y versionado del bridge. El fixture
`src/fixtures/e1-s2-cold.presentation.json` contiene los 27 eventos, no los 10
hitos del PoC. `Export-PresentationFixture.ps1 -Mode Write` lo genera desde el
exportador Rust; `-Mode Check` y el golden test cruzado exigen igualdad byte a
byte frente a una nueva ejecución del motor.

| Comando | Resultado verificable |
| --- | --- |
| `npm test` con Vite 8 | No descubrió tests: `spawn EPERM` al cargar configuración. |
| `npm test` tras cambiar a Vite 6/Vitest 3 | Tampoco descubrió tests: `esbuild` devolvió `spawn EPERM`. |
| `npx vue-tsc -b` | Pasó después de corregir los errores de tipado iniciales. |
| `npm run build` | Pasó: Vite generó `dist/` (19 módulos transformados). |
| `npm run dev -- --host 127.0.0.1` | Bloqueado antes de iniciar el servidor por el mismo `spawn EPERM` de esbuild; no hubo verificación manual en navegador. |

Por tanto, **no se afirma que las pruebas Vitest hayan pasado**. La limitación
es de infraestructura de creación de subprocesos de Node/esbuild; ejecutar
`esbuild.exe --version` directamente sí funcionó.

En la verificación posterior de esta fase, `npx vue-tsc -b` volvió a pasar,
pero `npm run build` volvió a detenerse en el mismo `spawn EPERM` al cargar la
configuración Vite. El resultado histórico de build anterior no se usa como
validación del estado actual.

## Incremento 5 — bridge Tauri

Se añadieron comandos `contract_version` y `run_e1_s2_cold`, más una prueba de
comando y una prueba unitaria de mapeo de error de ejecución.

| Comando | Resultado verificable |
| --- | --- |
| `cargo test --test bridge_commands` inicial | RED: faltaba `src/lib.rs`. |
| `cargo test --test bridge_commands` tras implementar | Bloqueado: crate `tauri` no existe en la caché offline. |
| `cargo test --config net.offline=false --test bridge_commands` | Bloqueado: proxy local no pudo conectar con `index.crates.io:443`; se detuvieron dos procesos Cargo tras los reintentos. |
| `npx tauri --version` | Pasó: `tauri-cli 2.12.3`. |
| `npm run tauri -- dev` | Con Cargo ausente de `PATH`, la CLI se detuvo antes de iniciar. Con `C:\Users\familia\.cargo\bin` agregado temporalmente a `PATH`, llegó a `beforeDevCommand` y Vite falló con `spawn EPERM`. |
| `cargo metadata --config net.offline=false --config net.retry=0` | Bloqueado antes de compilación: el proxy configurado `127.0.0.1:9` no conecta con `index.crates.io:443`. |
| `cargo metadata` sin variables de proxy | Bloqueado antes de compilación: Schannel devuelve `SEC_E_NO_CREDENTIALS`; retirar el proxy no proporciona una salida de red válida. |
| `node` con un `spawnSync` de sí mismo | Bloqueado: `EPERM` al crear cualquier hijo de Node, incluso antes de involucrar esbuild. |
| WSL | Bloqueado: `Wsl/EnumerateDistros/Service/E_ACCESSDENIED`; no hay entorno alternativo utilizable desde esta sesión. |

En consecuencia no se ejecutó una ventana Tauri, una invocación Vue→Rust ni la
prueba de comando; la vertical real queda pendiente de permisos para spawn de
Node y de dependencias Cargo accesibles. La prueba Vue del bridge con doble de
`invoke` tampoco se ejecutó por el bloqueo Vitest.

## Calidad y límites

- `ScenarioCanonicalReport` y ADR-0002 no fueron modificados.
- No se añadieron protocolos, ECS, plugins, MCP ni GUI definitiva.
- El `PATH` de esta sesión no incluye Cargo, aunque existe en
  `C:\Users\familia\.cargo\bin\cargo.exe`; los scripts de evaluación lo agregan
  de forma local. No es una falla de compilación del proyecto.
- No hay fuentes cacheadas de `tauri` y no se creó `src-tauri/Cargo.lock` porque
  Cargo no pudo resolver el registro. Por tanto no se alcanzó compilación Rust
  de Tauri y no se infiere ningún fallo de código host.
- El análisis de emparejamiento estático solicitado por la guía de pruebas no
  pudo ejecutarse: falta `tree-sitter-language-pack`; no se instaló por no ser
  una dependencia del proyecto.
- La revisión de aserciones confirma que las nuevas pruebas cubren igualdad,
  estructura/colección, tipos de PDU, excepción/rechazo, estado de retención y
  llamadas del bridge. No hay aserciones tautológicas conocidas; Vitest queda
  sin ejecución, como se indica arriba.

## Verificación final ejecutada

| Comando | Resultado |
| --- | --- |
| `cargo fmt --check` y `cargo test` en Rust | Formato conforme; 21 pruebas pasadas, 0 fallidas. |
| `dotnet test tests\PacketLens.SpikeE1.Tests\PacketLens.SpikeE1.Tests.csproj --no-restore` | 24 pruebas pasadas, 0 fallidas, 0 omitidas. |
| `.\tools\e1-evaluation\Export-PresentationFixture.ps1 -Mode Check` | Fixture real de Rust: 27 eventos, `Quiescent`, `8700 ns`, 15.334 bytes. |
| `.\tools\e1-evaluation\Compare-PresentationS2.ps1` | Rust/C# byte a byte iguales y fixture igual a Rust; 15.334 bytes. |
| `npx vue-tsc -b` | Pasó. |
| `npm run build` | Bloqueado por `spawn EPERM` antes de construir Vite. |
| `npm test` | Bloqueado por `spawn EPERM` antes de descubrir pruebas. |
| `npm run tauri -- dev` con Cargo en `PATH` | Llega a Vite y queda bloqueado por `spawn EPERM`; no inicia host. |
| prueba Tauri directa | Bloqueada por caché Cargo/proxy, no ejecutada. |
