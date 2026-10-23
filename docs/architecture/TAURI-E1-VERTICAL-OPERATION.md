# Operacion de la vertical Tauri E1-S2 experimental

## Estado y limite

Este documento describe la vertical experimental aprobada para E1-S2 frio. No
es la arquitectura definitiva de escritorio ni una aprobacion de distribucion.
La aplicacion compone el Inspector Educativo avanzado. El 2026-10-22 se
compilo y abrio el ejecutable Windows y se valido IPC real Vue a Rust: 27
eventos, 8700 ns, Quiescent y procedencia live_engine. Tambien se comprobo que
un rechazo nativo conserva la sesion previa y permite una nueva ejecucion.
La evidencia vigente consta en [E1-NATIVE-INTEGRATION.md](../reports/E1-NATIVE-INTEGRATION.md).

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
la version incompatible. Las 26 pruebas Vitest pasaron en Windows fuera del
sandbox. `npm run test:native` verifica ademas el transporte del WebView2 real;
no devuelve respuestas fabricadas ni usa el fixture para satisfacer el IPC.

## Ejecucion y empaquetado minimo

En un equipo Windows con Node, Cargo/Rust accesible desde `PATH`, dependencias
Cargo resolubles y permisos para que Node cree el subproceso de esbuild:

```powershell
Set-Location E:\packetlens\apps\packetlens-e1-vertical
npm ci
npm test
$env:Path = "C:\Users\familia\.cargo\bin;$env:Path"
cargo fetch --manifest-path src-tauri\Cargo.toml --locked --target x86_64-pc-windows-msvc
cargo test --manifest-path src-tauri\Cargo.toml --locked
npm run test:native
```

`test:native` ejecuta primero su hook `pretest:native`: compila frontend y un
binario debug con assets embebidos mediante Tauri. Luego abre ese binario en
una instancia WebView2 con perfil aislado y puerto de inspeccion loopback
temporal, pulsa el boton, verifica el JSON real y la vista, provoca un rechazo
real de comando y comprueba preservacion y recuperacion. Cierra su instancia
y su puerto al finalizar; no cambia configuracion global de WebView2.

Los resultados quedan en `test-results/native-e1.json`, `native-e1-live.png`
y `native-e1-preserved.png`. El detalle y los limites de la inyeccion de fallo
estan en el informe de integracion. Los perfiles de prueba quedan bajo
`src-tauri/target/webview2-e1-*` para inspeccion local.

Para abrir la aplicacion normal despues de las pruebas:

```powershell
Start-Process .\src-tauri\target\debug\packetlens-e1-vertical.exe
```

La interfaz muestra `8.700 µs`, equivalente a `8700 ns` (8.700 nanosegundos).
El contrato conserva la cadena decimal `"8700"`. La procedencia visible es
`Ejecucion viva del motor Rust`, correspondiente a `live_engine`.

Cerrar las ventanas PacketLens antes de volver a compilar: Windows bloquea
el reemplazo de un ejecutable en uso. Ejecutar `cargo test` antes de
`test:native`; el hook recompila con los assets embebidos y evita reutilizar
accidentalmente el binario de desarrollo que espera un servidor Vite.

Para desarrollo interactivo con Vite, el comando es:

```powershell
npm run tauri -- dev --no-watch -- --locked
```

La aceptacion automatizada de esta entrega uso el ejecutable con assets
embebidos; no se presenta ese comando dev como una prueba adicional ejecutada.

Para producir solo el ejecutable de validacion, sin instalador, despues de las
pruebas verdes:

```powershell
npm run tauri -- build --no-bundle -- --locked
```

La configuracion actual declara `bundle.active = false`; por tanto no se
generara instalador. La CLI instalada confirma que `--no-bundle` salta el paso
de bundling. La ruta esperada del binario de release es
`src-tauri\target\release\packetlens-e1-vertical.exe`. Esta entrega produjo y
valido la variante debug con `build --debug --no-bundle -- --locked`, en
`src-tauri\target\debug\packetlens-e1-vertical.exe`; no produjo release.

`--no-bundle` es una opcion de Tauri y debe preceder al segundo `--`.
`--locked` es un argumento de Cargo y lo sigue. La sintaxis debug fue ejecutada
con exito por la CLI local; la forma anterior `build -- --no-bundle` colocaba
incorrectamente una opcion Tauri en los argumentos de Cargo.

Un instalador Windows requiere una decision posterior para activar un bundle y
elegir `msi` o `nsis`, junto con pruebas en una maquina limpia y una politica de
firma. No se cambia esa configuracion experimental ahora.

## Entorno y dependencias

Las verificaciones vigentes se ejecutaron en Windows fuera del sandbox que
restringe tuberias de Node con `spawn EPERM`. No se debilitaron protecciones
del sistema ni TLS. No son necesarios WSL ni Docker para esta vertical.

Se versiona `src-tauri/Cargo.lock`: tauri 2.12.5 es compatible por version minor
con API npm 2.12.4. Se conservaron las versiones npm existentes; solo se anadio
`playwright-core` 1.64.1 para aceptar el WebView2 instalado, sin descargar otro
navegador, y se retiro `@vue/tsconfig`, no utilizado y con un peer TypeScript
incompatible. `npm ci` paso sin flags para ignorar peers.

Los iconos minimos proceden de `src-tauri/icons/icon.svg`. Para regenerarlos:

```powershell
npm run tauri -- icon src-tauri/icons/icon.svg --output src-tauri/target/icon-generation
Copy-Item src-tauri/target/icon-generation/icon.ico src-tauri/icons/icon.ico
Copy-Item src-tauri/target/icon-generation/icon.png src-tauri/icons/icon.png
```

No se hizo una migracion mayor de Vitest para resolver avisos de auditoria
de desarrollo ajenos al bloqueo de integracion. Esos avisos y las advertencias
de engines opcionales constan en el informe; `npm audit --omit=dev` informo cero
vulnerabilidades. Esto no constituye una aprobacion de distribucion.
