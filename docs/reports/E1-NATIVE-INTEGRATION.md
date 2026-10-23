# Validación nativa de la vertical E1

Fecha: 2026-10-22. Plataforma: Windows x64 `10.0.26200`.
Alcance autorizado: `apps/packetlens-e1-vertical` y su documentación específica.

## Resultado verificado

La aplicación Tauri abrió una ventana Windows y ejecutó E1-S2 mediante el
botón Vue y el host Rust reales. La aceptación automatizada comprobó:

- `schema_version = presentation/v1`, escenario `E1-S2`, corrida `cold`.
- 27 eventos, secuencias 1 a 27 y `total_emitted_ever = "27"`.
- `final_virtual_time_ns = "8700"` y `outcome = Quiescent`.
- Procedencia `live_engine`, presentada como **Ejecución viva del motor Rust**.
- Interfaz con **27 eventos visibles**, **Quiescent** y **8.700 µs**.
  `8.700 µs` equivale a `8700 ns`, es decir, 8.700 nanosegundos con separador
  de miles español. No se alteró la cadena decimal del contrato.
- Ante un rechazo real del host, conservación de procedencia, resumen, traza,
  evento seleccionado y contenido del inspector. Una nueva corrida válida
  posterior funcionó y eliminó el aviso de error.

Resultados finales: **Vitest 26/26**, **host Rust 2/2**, **aceptación nativa
1/1**. No quedaron bloqueos funcionales observados en el flujo solicitado.

Además de la automatización, se abrió una instancia normal sin puerto de
depuración. Computer Use observó la ventana, pulsó **Ejecutar motor Rust** y
observó el cambio de snapshot guardado a ejecución viva, con Quiescent y
8.700 µs. Esa instancia quedó abierta para revisión del usuario.

## Evidencia durable

- [Resultado estructurado y secuencia IPC](evidence/e1-native-2026-10-22/native-e1.json).
- [Captura tras éxito IPC](evidence/e1-native-2026-10-22/native-e1-live.png).
- [Captura de sesión conservada tras rechazo nativo](evidence/e1-native-2026-10-22/native-e1-preserved.png).

La última evidencia automatizada se obtuvo entre `2026-10-22T14:20:41.409Z`
y `2026-10-22T14:20:51.389Z` (09:20 en Lima). El ejecutable probado fue
`src-tauri/target/debug/packetlens-e1-vertical.exe`, con assets embebidos,
servido por `http://tauri.localhost/`. No era una página en un navegador
externo ni una ejecución contra Vite.

## RED -> GREEN y cambios realizados

### CAM: corrección de contenido, con aserciones intactas

La ejecución focalizada previa al cambio produjo **5 pasadas, 1 fallida**:

```text
npm test -- src/adapters/presentationAdapter.test.ts
Expected: "CAM"
Received: "Hecho del motor: delta mac_learned y protocolo del evento ethernet."
```

Se cambió únicamente el rationale de la rama `mac_learned` del catálogo a:

```text
Hecho del motor: actualización de la tabla CAM mediante delta mac_learned y protocolo del evento ethernet.
```

Se conserva el contexto posterior de IPv4/ICMP como vecindad didáctica, sin
afirmar causalidad adicional. No hay condiciones especiales por ID de evento,
dispositivo o tiempo. Las aserciones para eventos 4, 19 y 24 no se editaron.

GREEN focalizado: **6/6**. Suite completa: **26/26**, repetida con el mismo
resultado tras la reinstalación reproducible mediante `npm ci`.

### Host: recursos y macros antes no compilados

La primera compilación real falló en `tauri-build` por:

```text
src-tauri\icons/icon.ico not found; required for generating a Windows Resource file during tauri-build
```

Se añadió un icono local sencillo de nodos de red: fuente vectorial `icon.svg`
y recursos `icon.ico`/`icon.png` generados mediante la CLI Tauri existente.
Los demás tamaños/plataformas generados quedaron en `target`, no en fuentes.
No se instalaron herramientas gráficas ni generadores globales.

La siguiente compilación reveló cuatro errores `E0255`: macros
`__cmd__contract_version`, `__tauri_command_name_contract_version`,
`__cmd__run_e1_s2_cold` y `__tauri_command_name_run_e1_s2_cold` duplicadas.
Se movieron los handlers públicos desde la raíz de `lib.rs` a `commands.rs`;
`lib.rs` conserva los reexports y registra las rutas de ese módulo en
`generate_handler!`. Se mantienen los nombres IPC, tipos de retorno, JSON y
códigos de error. La prueba unitaria existente se trasladó con su función,
sin cambiar su esperado.

Después pasaron la prueba unitaria de mapeo y la prueba de integración del
comando. Esas pruebas ejecutan Rust real; por sí solas no prueban transporte
IPC. La aceptación WebView2 cubre ese límite adicional.

### Aceptación nativa y correcciones del propio ensayo

Se añadió un ensayo focalizado en `scripts/native-e1.test.mjs`, usando el runner
de Node y `playwright-core`, contra una instancia real del ejecutable.

Su primera ejecución falló **0/1**: la instrumentación intentaba reemplazar
`__TAURI_INTERNALS__.invoke`, que es una propiedad inmutable; no registró las
llamadas. Se corrigió el ensayo para observar y delegar el transporte `fetch`
hacia `ipc.localhost`, sin cambiar esas propiedades de Tauri.

La segunda ejecución falló **0/1** al comprobar recuperación demasiado pronto:
el aviso desaparece al comenzar la nueva corrida, antes de recibir su respuesta.
Se añadió una espera por las seis respuestas IPC observadas. No se relajó el
esperado ni se cambió el producto para acomodar la prueba.

La tercera ejecución pasó **1/1**; la última, con reconstrucción automática
previa del ejecutable, también pasó **1/1**.

Durante una verificación intermedia, `cargo test` no pudo reemplazar
`target/debug/packetlens-e1-vertical.exe` porque nuestra ventana estaba abierta:
`Acceso denegado (os error 5)`. Se cerró esa instancia y se repitió el comando:
**2/2 pasadas**. El resultado final no se infiere del estado 0 de una shell
compuesta: se comprobó la salida individual de Cargo. La guía ahora indica
cerrar las ventanas antes de recompilar.

## Qué demuestra la inyección de error

La prueba no entrega snapshots prefabricados ni simula respuestas Rust.
Observa los cuerpos de respuesta reales mediante clones de `Response` y
devuelve la respuesta original al frontend.

Para la segunda corrida cambia únicamente el destino de la solicitud de
`run_e1_s2_cold` a `packetlens_acceptance_unknown_command`, conservando el
transporte y las cabeceras originales. El host responde realmente:

```text
Command packetlens_acceptance_unknown_command not found
```

Se comprueba el aviso y que el estado visible previo es idéntico, con evento
19 y la pestaña de lectura didáctica seleccionados. La tercera corrida utiliza
otra vez el comando correcto y verifica su respuesta real. El ensayo restaura
`fetch`, cierra su conexión y termina únicamente el árbol de su proceso.

Esto valida la conservación ante un rechazo IPC nativo. No pretende demostrar
un fallo físico del motor ni fuerza `engine_execution_failed` en producción;
el mapeo de ese error se verifica en la prueba unitaria Rust existente.

La conexión CDP del WebView2 sigue el método documentado por
[Playwright](https://playwright.dev/docs/webview2) y referenciado por
[Microsoft](https://learn.microsoft.com/en-us/microsoft-edge/webview2/how-to/playwright).
Se usa un perfil aislado en `target`, un puerto loopback temporal y variables
limitadas al proceso hijo. La ejecución normal final no usa ese puerto.

## Dependencias y compatibilidad

| Cambio | Revisión |
| --- | --- |
| `src-tauri/Cargo.lock` nuevo | Cargo fijó 407 dependencias; el TOML contiene 408 entradas contando la propia aplicación (406 de registro y dos paquetes locales). No se editaron los manifiestos de los motores. |
| Crates nativos | `tauri 2.12.5`, `tauri-build 2.7.4`, `tauri-runtime 2.12.2`, `tauri-runtime-wry 2.12.3`, `wry 0.57.0`, `tao 0.37.1`. |
| API/CLI npm conservadas | API `2.12.4`, CLI `2.12.3`; API y crate `tauri` comparten minor `2.12`, como recomienda [Tauri](https://v2.tauri.app/develop/updating-dependencies/#sync-npm-packages-and-cargo-crates-versions). La CLI comprobó versiones al compilar sin error de incompatibilidad. |
| Nueva dependencia npm | Solo `playwright-core 1.64.1`, exacta, de desarrollo; usa WebView2 instalado. No se descargó Chromium adicional ni se instalaron herramientas globales. |
| Dependencia npm retirada | `@vue/tsconfig 0.9.1`: no se referenciaba en ninguna configuración y su peer TypeScript `>=5.8` chocaba con `5.7.3`. El primer intento de instalar Playwright falló con `ERESOLVE` por ello. |
| Lock npm | No cambió la versión de ningún paquete conservado. npm retiró además 12 entradas opcionales huérfanas de lightningcss/detect-libc. `npm ci` posterior pasó sin `--force` ni `--legacy-peer-deps`. |
| Entorno | Node `24.11.1`, npm `11.6.2`, Rust/Cargo `1.99.1`, MSVC y WebView2 `155.0.4283.62`. |

SHA-256 de los lockfiles finales:

```text
Cargo.lock:        47276C79818D48FC41D7647C105E71A47A598A9D5FC334E021CBAE8A6AD180B5
package-lock.json: 5B496C47D3F51D2B5F41981B68B0586355C2EB48F901BEA38A8881D46CCD4556
```

La ejecución se realizó fuera del sandbox diagnosticado, mediante comandos
autorizados de la herramienta. No se cambiaron TLS, antivirus, firewall,
proxies globales, capacidades ni CSP. No se necesitó WSL ni Docker.

## Matriz de requisitos y evidencia

| Requisito | Evidencia ejecutada |
| --- | --- |
| CAM educativo sin ocultar el defecto | `keeps ARP queueing, CAM learning, and IPv4/ICMP CAM refresh as distinct facts`: RED antes y GREEN después; aserciones intactas. |
| Suite frontend completa | `npm test`: 10 archivos, 26 pruebas pasadas, 0 fallidas. |
| Dependencias reproducibles | `npm ci`: 148 paquetes instalados; `cargo generate-lockfile` y `cargo fetch --manifest-path src-tauri/Cargo.toml --locked --target x86_64-pc-windows-msvc`: completados. |
| Mapeo de error del host | `commands::tests::maps_engine_execution_failures_without_exposing_engine_types_to_vue`: pasada. |
| Comando Rust E1-S2 | `bridge_exposes_presentation_v1_and_the_real_e1_s2_cold_snapshot`: pasada. |
| Ventana, IPC y sesión ante errores | `native E1-S2 uses real IPC and preserves the displayed live session after a native rejection`: pasada, incluidas las condiciones conjuntas del resultado y recuperación. |
| Compilación | `npm run build`: tipos y Vite correctos, 50 módulos. `tauri build --debug --no-bundle -- --locked`: ejecutable Windows producido. |
| Formato Rust de la aplicación | `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`: conforme. |
| Comando documentado | Corregido a `npm run tauri -- build --no-bundle -- --locked`; la variante debug se ejecutó con éxito, directamente y como hook de `test:native`. |

## Reproducción

Cerrar otras ventanas de este ejecutable antes de compilar. En una terminal
Windows normal, con Cargo en PATH y red disponible para las cachés ausentes:

```powershell
Set-Location E:\packetlens\apps\packetlens-e1-vertical
npm ci
npm test
cargo fetch --manifest-path src-tauri/Cargo.toml --locked --target x86_64-pc-windows-msvc
cargo test --manifest-path src-tauri/Cargo.toml --locked
npm run test:native
Start-Process .\src-tauri\target\debug\packetlens-e1-vertical.exe
```

`pretest:native` reconstruye el binario embebido antes del ensayo; no depende
de que el último binario producido por `cargo test` tenga el modo correcto.
Los resultados de nuevas ejecuciones quedan en `test-results/`; las evidencias
enlazadas arriba son la copia conservada de esta entrega.

## Avisos pendientes y límites

- `npm audit` completo terminó con salida 1 e informó tres dependencias de
  desarrollo afectadas: `@vitest/mocker` (moderada), `tinypool` y `vitest`
  (críticas según el informe). npm propone un cambio mayor a Vitest `5.0.5`.
  No se aplicó `audit fix --force` ni una migración mayor ajena a esta
  corrección. Requiere evaluación específica de las versiones soportadas y
  sus cambios; el resultado funcional verde no elimina estos avisos.
- `npm audit --omit=dev --json` terminó con salida 0 y cero vulnerabilidades
  informadas. Es un resultado de esa auditoría, no una garantía de seguridad.
- npm advirtió `EBADENGINE` para las dependencias transitivas opcionales
  `abbrev 5.0.0` y `nopt 10.0.1`, que requieren Node más reciente dentro de sus
  líneas admitidas. `npm ci`, build y pruebas pasaron con el Node existente.
  No se cambió globalmente Node para resolver esas advertencias.
- No se produjo release, instalador ni firma, ni se probó una máquina Windows
  limpia. La aceptación acreditada es la vertical debug nativa con assets
  embebidos. El comando dev con Vite no se reensayó tras las correcciones.
- No se ejecutaron suites completas de los motores Rust/C#, ni comparaciones
  E2.1 en este trabajo. No se editaron motores, goldens, contratos normativos
  ni especificaciones, ni se cambió de rama o se restauraron archivos ajenos.

La separación motor -> DTO -> bridge -> UI se conserva. La corrección no
necesita otro backend ni lógica de simulación en Vue. El nuevo ensayo protege
el límite nativo que antes solo tenía pruebas directas del comando y mocks.
