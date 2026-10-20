# Operacion de la vertical Tauri E1-S2 experimental

## Estado y limite

Este documento describe la vertical experimental aprobada para E1-S2 frio. No
es la arquitectura definitiva de escritorio ni una aprobacion de distribucion.
La aplicacion ya compone el Inspector Educativo avanzado con el fixture real
de E1-S2. Aun no se ha compilado el host, no se ha abierto una ventana y no se
ha ejecutado una invocacion real Vue a Rust. Las evidencias y bloqueos vigentes
constan en `docs/reports/INSPECTOR-ADVANCED-INTEGRATION.md`.

`SimulationPresentationSnapshot v1` es el unico dato que cruza la frontera de
presentacion. El bridge no expone tipos de dominio Rust, eventos internos del
scheduler, ni reglas de ARP o CAM. `ScenarioCanonicalReport` no participa en
esta ruta y ADR-0002 permanece intacto.

## Mecanismo exacto del bridge

1. Vue carga primero el fixture guardado de E1-S2 frio, exportado por Rust y
   validado con `parsePresentationSnapshot`. Tambien puede mostrar un fixture
   sintetico, marcado explicitamente como tal. Ninguno depende de Tauri.
2. Al elegir **Ejecutar motor Rust**, `App.vue` llama
   `tauriPresentationBridge.runE1S2Cold()` a traves de
   `createPresentationSourceState`.
3. `tauriBridge.ts` invoca el comando IPC `contract_version` con
   `@tauri-apps/api/core` y exige exactamente `presentation/v1`.
4. Solo despues invoca `run_e1_s2_cold`. El handler Rust ejecuta
   `spike_e1_rust::presentation::export_e1_s2_cold`, valida el snapshot en el
   exportador y devuelve su JSON como `String`.
5. Vue aplica `JSON.parse` y `parsePresentationSnapshot`. La validacion usa
   `BigInt` para las cantidades decimales y verifica referencias de topologia,
   secuencias de traza, orden temporal, retencion, discriminantes, PDU y deltas.
6. Solo tras esa validacion el estado reemplaza la sesion visible por una con
   procedencia `live_engine`. Ante un rechazo conserva exactamente la sesion
   anterior y publica el error con `role="alert"`; el inspector navega hechos
   producidos por el motor, sin recalcular protocolos.

El host registra los dos comandos mediante `tauri::generate_handler!`. La
capacidad de la vertical es `core:default`; no se habilitaron permisos de
filesystem, red, shell ni plugins.

## Manejo de errores

| Origen | Representacion | Comportamiento del consumidor |
| --- | --- | --- |
| Version distinta | `ContractVersionMismatchError` antes de solicitar una corrida | Conserva el snapshot actual y muestra el error. |
| El host devuelve algo que no es texto | `Error` del bridge | No se intenta parsear ni reemplazar el snapshot. |
| JSON invalido o contrato invalido | `SyntaxError` o `PresentationContractError` | No se muestran hechos parciales. |
| Fallo de ejecucion Rust | codigo `engine_execution_failed` | Tauri rechaza la invocacion; Vue conserva el snapshot actual. |
| Snapshot inconsistente | codigo `presentation_contract_invalid` | Igual: no se actualiza el inspector. |
| Resultado no esperado | codigo `unexpected_execution_outcome` | Igual: no se actualiza el inspector. |

`presentationSourceState` captura el rechazo, deja el snapshot anterior intacto
y libera el estado de ejecucion; `App.vue` lo presenta con `role="alert"`.
`tauriBridge.test.ts` cubre el orden `contract_version` -> `run_e1_s2_cold` y
la version incompatible. Las pruebas no se pudieron ejecutar en este sandbox
porque Vitest falla al arrancar esbuild (`spawn EPERM`), por lo que no se
declaran validadas hasta ejecutarlas en un entorno compatible.

## Ejecucion y empaquetado minimo

En un equipo Windows con Node, Cargo/Rust accesible desde `PATH`, dependencias
Cargo resolubles y permisos para que Node cree el subproceso de esbuild:

```powershell
Set-Location E:\packetlens\apps\packetlens-e1-vertical
npm ci
npm test
npm run build
$env:Path = "C:\Users\familia\.cargo\bin;$env:Path"
& cargo test --manifest-path src-tauri\Cargo.toml --test bridge_commands
npm run tauri -- dev
```

La aceptacion de la vertical requiere observar una ventana Tauri, pulsar el
boton y comprobar que la respuesta validada tiene `presentation/v1`, 27 eventos,
`Quiescent` y `8700` ns. Ningun comando anterior se considera exitoso sin esa
observacion ni sin las pruebas Vitest y Rust verdes.

Para producir solo el ejecutable de validacion, sin instalador, despues de las
pruebas verdes:

```powershell
npm run tauri -- build -- --no-bundle
```

La configuracion actual declara `bundle.active = false`; por tanto no se
generara instalador. La CLI instalada confirma que `--no-bundle` salta el paso
de bundling. La ruta esperada del binario de release es
`src-tauri\target\release\packetlens-e1-vertical.exe`; aun no fue producida en
este entorno.

Un instalador Windows requiere una decision posterior para activar un bundle y
elegir `msi` o `nsis`, junto con pruebas en una maquina limpia y una politica de
firma. No se cambia esa configuracion experimental ahora.

## Bloqueos actuales, no decisiones de arquitectura

- Vitest/Vite falla antes de descubrir pruebas: Node recibe `spawn EPERM` al
  iniciar el servicio de esbuild. Una sonda que intenta crear otro proceso Node
  también recibe `EPERM`, por lo que no es un defecto específico de esbuild.
- La CLI Tauri inicialmente no encontro Cargo porque este no estaba en `PATH`
  de npm. El 2026-10-19, exponiendo solo para el proceso
  `C:\Users\familia\.cargo\bin`, alcanzo `beforeDevCommand` y se detuvo en el
  mismo `spawn EPERM` de Vite. No se alcanzo la compilacion del host ni una
  ventana.
- Cargo no tiene fuentes locales de `tauri`; con red habilitada el proxy
  configurado `127.0.0.1:9` no alcanza `index.crates.io:443`. Al retirar las
  variables de proxy solo para el proceso de prueba, Schannel devuelve
  `SEC_E_NO_CREDENTIALS`; tampoco hay una ruta directa autenticada al registro.
  No se llego a compilacion del host ni se atribuye un error al codigo Tauri.
- WSL existe en el host, pero `wsl --status` devuelve
  `Wsl/EnumerateDistros/Service/E_ACCESSDENIED`; Docker no esta instalado. Esta
  sesión no puede abrir un entorno alternativo sin dichos bloqueos.

Resolver esos prerequisitos permitira ejecutar la vertical sin cambiar de
contrato ni de arquitectura.
