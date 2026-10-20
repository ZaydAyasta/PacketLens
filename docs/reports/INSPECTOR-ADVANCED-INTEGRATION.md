# Integración controlada del Inspector Educativo avanzado

Fecha: 2026-10-19

## Alcance y resultado

Se sustituyó la composición mínima de `apps/packetlens-e1-vertical/src/App.vue`
por el Inspector Educativo modular. La única aplicación Vue sigue siendo
`apps/packetlens-e1-vertical`; no se creó una segunda aplicación, no se
modificó el workspace de Antigravity y no se agregaron protocolos, plugins ni
código de motor.

La fuente normativa que entra a la interfaz sigue siendo
`SimulationPresentationSnapshot v1` (`presentation/v1`).
`ScenarioCanonicalReport` y ADR-0002 no participan en esta migración.

## Reversibilidad

Se intentó crear la rama `integration/advanced-inspector`, pero el sandbox no
permite escribir `E:\packetlens\.git\refs\heads`:

```text
fatal: cannot lock ref 'refs/heads/integration/advanced-inspector':
unable to create directory for .git/refs/heads/integration/advanced-inspector
```

Por ello los cambios quedaron sin commit sobre la rama de trabajo existente.
No se intentó forzar permisos ni alterar la historia. En un entorno con `.git`
escribible, debe crearse esa rama antes de preparar el commit.

## Auditoría de compatibilidad y matriz de migración

| Origen de Antigravity (solo lectura) | Destino principal | Decisión |
| --- | --- | --- |
| `src/contracts/presentationV1.ts` | `src/contracts/presentation.ts` y `presentationContract.ts` | No migrado: era un DTO duplicado y añadía `_provenance`, fuera del contrato normativo. |
| `src/types/simulation.ts` | — | No migrado: era un modelo legado incompatible y sugería campos no emitidos. |
| `src/types/inspector.ts` | `src/types/inspector.ts` | Migrado como límite interno de ViewModels, reescrito para importar exclusivamente el DTO de destino. |
| `src/adapters/presentationAdapter.ts` | `src/adapters/presentationAdapter.ts` | Migrado y adaptado: recibe solo snapshots válidos, usa `BigInt` y no depende de procedencia en el DTO. |
| `src/catalog/pedagogicalCatalog.ts` | `src/catalog/pedagogicalCatalog.ts` | Migrado y corregido: las explicaciones son didácticas, no decisiones emitidas por el motor. |
| `src/composables/useSimulation.ts` | `src/composables/useSimulation.ts` | Migrado: navegación, filtros y reproducción son locales e inmutables. |
| `src/components/{common,layout,topology,timeline,inspector}` | Rutas homónimas bajo `src/components` | Migrados selectivamente: badges, retención, encabezado, topología, eventos, PDU, tablas y lectura didáctica. |
| `src/fixtures/e1-s2-cold.presentation.json` | Fixture de destino existente | Conservado: el destino usa el fixture generado por Rust, no una copia del PoC. |
| `src/fixtures/syntheticScenarioE1Snapshot.ts` | `src/fixtures/syntheticE1Snapshot.ts` | Adaptado a una demostración pequeña, válida y explícitamente sintética. |
| cuatro `src/__tests__/*.spec.ts` | siete `*.test.ts` de destino nuevos o ampliados | Comportamientos trasladados a la convención de destino y corregidos contra el contrato vigente. |
| ejemplo `loadSnapshotFromTauri` | — | No migrado: esa API no existe. Se conserva `tauriPresentationBridge.runE1S2Cold()`. |

Las dependencias del PoC (`vue`, `@vue/test-utils`, `jsdom`) ya estaban en el
destino. No se actualizó Vite/Vitest/TypeScript ni se agregaron dependencias.

## Contrato, procedencia y bridge

Los únicos módulos que definen o validan el DTO son
`src/contracts/presentation.ts` y `src/contracts/presentationContract.ts`.
El adaptador solo recibe un DTO tipado; los ingresos locales de `App.vue` pasan
por `parsePresentationSnapshot`. El bridge existente conserva el orden
`contract_version` y `run_e1_s2_cold`, y vuelve a validar el JSON IPC antes de
resolver.

`src/state/presentationSourceState.ts` distingue tres procedencias sin ampliar
el contrato del motor:

| Procedencia | Dato | Etiqueta visible |
| --- | --- | --- |
| `saved_engine_fixture` | `e1-s2-cold.presentation.json`, exportado por Rust | Snapshot guardado generado por el motor |
| `live_engine` | Resultado validado de `tauriPresentationBridge.runE1S2Cold()` | Ejecución viva del motor Rust |
| `synthetic_fixture` | `SYNTHETIC_E1_SNAPSHOT`, válido pero no producido por motor | Fixture sintético demostrativo |

Una ejecución viva fallida no cambia `session.snapshot`; deja el inspector en
el último snapshot válido, libera la carga y expone el error con `role="alert"`.
Filtros, cursor, reproducción y selección no invocan el bridge ni modifican el
motor.

## Correcciones pedagógicas aplicadas

- Evento 1: muestra una PDU IPv4/ICMP encolada mientras espera ARP; no afirma
  que exista un ARP Request dentro del buffer.
- Evento 4: `mac_learned` se presenta como aprendizaje CAM Ethernet, no como
  aprendizaje ARP del host.
- Eventos 19 y 24: mantienen `ethernet` como protocolo propio. El tráfico
  IPv4/ICMP inmediatamente anterior es contexto de traza didáctico y se declara
  expresamente que no es causalidad emitida por el motor.
- Los campos emitidos son `engine_fact`; contexto de formato es
  `didactic_context`; FCS, checksums e IHL no instrumentados son `unmodeled`.
  No se muestran valores inventados para esos campos.

## TDD: evidencia RED, GREEN y refactor

| Incremento | RED ejecutado | GREEN ejecutado | Refactor/decisión |
| --- | --- | --- | --- |
| Adaptador | `vue-tsc -b` falló con TS2307 para `presentationAdapter` ausente | `vue-tsc -b` terminó con código 0 tras crear adaptador y ViewModels | Se eliminó cualquier tipo de contrato duplicado y se usó `BigInt`. |
| Navegación | `vue-tsc -b` falló con TS2307 para `useSimulation` ausente | `vue-tsc -b` terminó con código 0 | La sesión de entrada es inmutable; el temporizador se libera con `dispose`. |
| Componentes | `vue-tsc -b` falló con cinco TS2307 para componentes avanzados ausentes | `vue-tsc -b` terminó con código 0 | La vista OSI se convirtió en lectura didáctica con advertencia explícita. |
| Procedencia/errores | `vue-tsc -b` falló con TS2307 para `presentationSourceState` ausente | `vue-tsc -b` terminó con código 0 | La sustitución del snapshot es atómica después de que resuelva el bridge. |
| Fixture sintético | `vue-tsc -b` falló con TS2307 para `syntheticE1Snapshot` ausente | `vue-tsc -b` terminó con código 0 | La procedencia sigue fuera del contrato; el fixture sí valida como `presentation/v1`. |
| Composición `App.vue` | `App.test.ts` se añadió antes de sustituir la composición mínima; Vitest no pudo descubrirlo | `npm run build` terminó correctamente | Vitest no tiene resultado de aserciones por infraestructura. |

Los primeros cinco RED son fallos de compilación por superficie de producción
aún ausente, no aserciones funcionales. El RED de composición no llegó a la
fase de aserciones porque Vitest no pudo arrancar.

## Pruebas y verificaciones ejecutadas

| Comando | Resultado medido |
| --- | --- |
| `npx vue-tsc -b` en `apps/packetlens-e1-vertical` | Correcto, código 0 después de todos los cambios. |
| `npm run build` en `apps/packetlens-e1-vertical` | La repetición final terminó correctamente, código 0; Vite 6.4.4 transformó 50 módulos y produjo el bundle. Dos intentos intermedios fallaron antes de compilar por el mismo `spawn EPERM` de esbuild. |
| `npm test` en `apps/packetlens-e1-vertical` | No inició pruebas: `esbuild` falla al cargar `vitest.config.ts` con `spawn EPERM`. Se intentó tres veces, mismo resultado. |
| `C:\Users\familia\.cargo\bin\cargo.exe test` en `spikes/spike_e1_rust` | Correcto: 16 unitarias + 3 de contrato + 2 de exportación; 21 aprobadas, 0 fallidas. |
| `dotnet test tests\PacketLens.SpikeE1.Tests\PacketLens.SpikeE1.Tests.csproj --no-restore` | Correcto: 24 aprobadas, 0 fallidas, 0 omitidas. |
| `npm run tauri -- dev` | Sin Cargo en `PATH`, Tauri falló en `cargo metadata`. |
| Tauri con `C:\Users\familia\.cargo\bin` añadido solo al proceso | Cargo fue reconocido y Tauri llegó a `BeforeDevCommand`; Vite falló por `spawn EPERM`. No hubo compilación del host ni ventana. |

### Inventario de pruebas Vue añadidas o ampliadas

| Requisito | Evidencia de prueba (pendiente de ejecución Vitest) |
| --- | --- |
| 27 eventos y secuencias estables | `presentationAdapter — E1-S2 cold snapshot > projects the 27 engine events without changing their trace identities` |
| Eventos 1, 4, 19 y 24 | `presentationAdapter — E1-S2 cold snapshot > keeps ARP queueing, CAM learning, and IPv4/ICMP CAM refresh as distinct facts` |
| BigInt, PDU y deltas ARP/CAM | `formats values past JavaScript safe integer precision using BigInt`; `keeps PDU fidelity and ARP/CAM state grounded in the emitted fields and deltas`; `removes ARP and CAM entries when the emitted expiration deltas arrive` |
| Retención truncada | `marks table history incomplete when retention declares capacity loss`; `advanced inspector components > makes retention and synthetic provenance explicit` |
| Filtros, navegación y reproducción inmutables | `useSimulation — read-only presentation navigation > filters and navigates the 27-event engine snapshot without mutating it`; `retains source provenance and stops playback at the visible boundary` |
| Topología, lista, PDU y tablas | `advanced inspector components > shows all real events and emits selection by decimal trace sequence`; `renders topology and preserves PDU provenance and table deltas` |
| Selector, bridge, error y recuperación | `presentation source state > distinguishes saved engine output, synthetic data, and a successful live run`; `retains the last valid snapshot and exposes an error when the live bridge fails`; `App advanced inspector integration > uses the existing bridge for a live run and preserves the prior view on error` |
| Versión, orden y errores del bridge | `createTauriPresentationBridge > checks the version before parsing a real engine snapshot`; `rejects an unsupported bridge contract before requesting a run`; `rejects a non-text bridge result without accepting a partial snapshot`; `rejects malformed engine JSON before it can reach presentation state`. |

No se informa ninguna prueba Vitest como aprobada: el runner no llegó a
descubrirlas. Typecheck y build son comprobaciones independientes y no las
sustituyen.

## Revisión de pruebas y límites pendientes

La revisión estática de pruebas nuevas utiliza igualdad, colecciones, texto,
forma de objeto, negación y transición de estado. No hay pruebas sin
aserciones, `skip` ni `only`. Las rutas de recuperación comprueban el mensaje y
la identidad del snapshot previo. Sin Vitest, siguen sin verificación empírica
el render Vue, emisiones DOM, temporizadores falsos, mocks del bridge y el
validador dentro del proceso Tauri.

La herramienta de emparejamiento estático se invocó para este paquete
TypeScript, pero no pudo analizarlo porque falta `tree-sitter-language-pack`.
No se instaló nada ni se trata esta ausencia como evidencia de cobertura.

No se validó la vertical Tauri: falta observar una ventana e invocar con éxito
el motor Rust desde Vue, obteniendo un snapshot validado `presentation/v1`,
`Quiescent`, `8700 ns` y 27 eventos.

## Archivos principales modificados o añadidos

- `apps/packetlens-e1-vertical/src/App.vue`
- `apps/packetlens-e1-vertical/src/adapters/presentationAdapter.ts`
- `apps/packetlens-e1-vertical/src/catalog/pedagogicalCatalog.ts`
- `apps/packetlens-e1-vertical/src/composables/useSimulation.ts`
- `apps/packetlens-e1-vertical/src/state/presentationSourceState.ts`
- `apps/packetlens-e1-vertical/src/types/inspector.ts`
- `apps/packetlens-e1-vertical/src/fixtures/syntheticE1Snapshot.ts`
- componentes bajo `src/components/{common,layout,topology,timeline,inspector}`
- pruebas Vue correspondientes `*.test.ts`
- `docs/architecture/TAURI-E1-VERTICAL-OPERATION.md`

Los artefactos internos del proceso de generación de pruebas están fuera del
repositorio, en el directorio temporal del proceso, y no forman parte del
entregable.
