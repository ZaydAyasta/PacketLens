# Diagnóstico de tooling e integración E1

**Actualización posterior a la autorización del usuario, 2026-10-22:**
corrección CAM validada, lockfile creado y dependencias descargadas, host
compilado y ventana/IPC reales verificados. Resultados finales: Vitest 26/26,
host 2/2, aceptación nativa 1/1. Véase
[E1-NATIVE-INTEGRATION.md](E1-NATIVE-INTEGRATION.md) para RED -> GREEN,
evidencias y avisos pendientes. El diagnóstico original que sigue se conserva
como registro de la fase de solo lectura; sus restricciones de autorización
y pendientes de compilación fueron superados en la fase autorizada.

Fecha: 2026-10-22, America/Lima. Repositorio: `E:\packetlens`.
HEAD observado: `851aa1a`, con cambios sin commit preexistentes de E2.1.
Aplicación examinada: `apps/packetlens-e1-vertical`.

## Resultado

**El `spawn EPERM` reproducido depende del contexto restringido de ejecución,
no del motor de simulación.** La misma sonda Node con tuberías falla dentro del
sandbox y pasa fuera de él, mediante la ejecución autorizada por la herramienta.
No se identificó qué mecanismo concreto de Windows/sandbox deniega la operación.

Al ejecutar las pruebas existentes fuera del sandbox apareció un fallo real,
independiente de infraestructura: **25 pruebas Vitest pasadas y 1 fallida**.
La repetición focalizada produjo **5 pasadas y la misma 1 fallida**. Por tanto,
resolver los permisos no vuelve verde la suite automáticamente.

Tauri, fuera del sandbox, inició Vite en `http://localhost:1420/` y alcanzó
`cargo run`. Se detuvo por la ausencia de `src-tauri/Cargo.lock`, porque se
utilizó deliberadamente `--locked` para preservar la restricción de no modificar
lockfiles. **No se compiló el host, no se abrió una ventana y no se verificó IPC
real Vue -> Rust.**

`npm run build` sí terminó correctamente en una invocación con permisos por
defecto: comprobación de tipos y build Vite de 50 módulos. Es una observación
positiva acotada; no demuestra que todo proceso bajo ese contexto pueda crear
tuberías. Una sonda y `npm test` posteriores volvieron a fallar con `EPERM`.

## Alcance y aplicación de TDD

Se leyeron `AGENTS.md`, `docs/engineering/TDD-POLICY.md`, las referencias E1
pertinentes y los informes previos de integración. Este trabajo es diagnóstico
de infraestructura y caracterización de pruebas existentes: no introduce
comportamiento productivo ni declara un ciclo GREEN inexistente.

- No se editaron código, pruebas, dependencias, lockfiles, configuración ni
  especificaciones. Este informe es el único archivo fuente añadido.
- No se ejecutaron suites ni builds de los spikes Rust/C#, ni herramientas de
  comparación E2.1. No se hizo commit, reset, stash ni limpieza de cambios ajenos.
- Las ejecuciones generaron artefactos habituales ignorados, como `dist/` y
  cachés de herramientas. La consulta online de Cargo pudo actualizar su índice
  local; no creó un lockfile ni compiló dependencias.
- Una comparación SHA-256 inicial/final de los 48 archivos enumerados de la
  aplicación, excluyendo `node_modules`, `target` y `dist`, no encontró cambios,
  adiciones ni eliminaciones. `git diff -- apps/packetlens-e1-vertical` quedó vacío.
- Tras el ensayo Tauri, la conexión a `127.0.0.1:1420` devolvió `ECONNREFUSED` y
  `Test-Path src-tauri/Cargo.lock` devolvió `False`. No quedó el servidor temporal.

## Entornos y versiones comprobadas

Se distinguen **S** (herramienta con permisos por defecto/sandbox) y **H**
(herramienta con `sandbox_permissions=require_escalated`, fuera del sandbox).
H no implica que fuese necesario usar una cuenta administradora: la comparación
demuestra el efecto del contexto de ejecución, no una necesidad de privilegios
de administrador para desarrollar PacketLens.

| Elemento | Observado |
| --- | --- |
| Sistema | Windows `10.0.26200`, x64 |
| Node / npm | `24.11.1` / `11.6.2` |
| Cargo / rustc | `1.99.1` / `1.99.1` |
| Toolchain Rust | `stable-x86_64-pc-windows-msvc` |
| Vite / Vitest | `6.4.4` / `3.2.7` |
| esbuild | `0.25.12`; ejecutable Windows x64 accesible |
| Vue / plugin Vue | `3.5.44` / `5.2.4` |
| jsdom | `29.1.1` |
| TypeScript / vue-tsc | `5.7.3` / `2.2.12` |
| Tauri API / CLI npm | `2.12.4` / `2.12.3` |
| WebView2, según `tauri info` en H | `155.0.4283.62` |
| MSVC, según `tauri info` en H | Visual Studio Community 2022 detectado |

Las versiones principales instaladas coinciden con `package-lock.json`.
Los manifiestos instalados de Vite, Vitest, esbuild, plugin Vue y jsdom admiten
Node `24.11.1`. El plugin Vue admite Vite 6. No se observó una incompatibilidad
de esos rangos que explique el error inicial.

En S se observaron `CARGO_NET_OFFLINE=true` y `HTTP_PROXY`, `HTTPS_PROXY`,
`ALL_PROXY` apuntando a `http://127.0.0.1:9`. En H esas variables no aparecieron
en la consulta del entorno; no se borraron manualmente. Cargo estaba en `PATH`
en esta sesión, a diferencia de algunos ensayos históricos.

## Causas confirmadas y límites de la evidencia

### 1. Creación de tuberías/IPC desde Node bajo S

Una sonda sin dependencias de PacketLens lanzó el mismo `process.execPath`:

| Operación | S | H |
| --- | --- | --- |
| `spawnSync`, `stdio: 'pipe'` | `EPERM`, `status: null` | Hijo ejecutado, estado 0, salida `child-ok` |
| `spawnSync`, `stdio: 'inherit'` | Estado 0 | Estado 0 |
| `spawnSync`, `stdio: 'ignore'` | Estado 0 | Estado 0 |
| `spawn`, solo stdin en `pipe` | `EPERM` | No repetido |
| `spawn`, solo stdout en `pipe` | `EPERM` | No repetido |
| `spawn`, solo stderr en `pipe` | `EPERM` | No repetido |
| `spawn`, stdio ignorado más canal `ipc` | `EPERM` | No repetido |
| `spawn`, todo stdio ignorado | Estado 0 | No repetido |
| `Worker` con `eval` y mensaje | `worker-ok` | No repetido |

No es correcto concluir que Node no pueda crear **ningún** subproceso. La
restricción observada afecta las modalidades con tuberías/IPC ensayadas.
La [documentación de Node 24.11.1](https://nodejs.org/download/release/v24.11.1/docs/api/child_process.html#optionsstdio)
define esas modalidades de `stdio`.

El archivo instalado `node_modules/esbuild/lib/main.js:1978` inicia su servicio
con `stdio: ['pipe', 'pipe', 'inherit']`. La llamada mínima
`require('esbuild').transform('const value: number = 1', { loader: 'ts' })`
falló en S con `spawn EPERM` en esa ruta. En cambio, ejecutar directamente
`node_modules/@esbuild/win32-x64/esbuild.exe --version` devolvió `0.25.12`.
Esto distingue la ejecución del binario de la comunicación de su servicio.

### 2. Cambiar cargador/pool no elimina todas las operaciones bloqueadas

Vite carga la configuración con esbuild por defecto. Sus alternativas
`runner` y `native` están documentadas para [Vite 6](https://v6.vite.dev/config/).
Se ensayaron por CLI sin editar los archivos del proyecto:

- `runner` con pool `threads`: falló al cargar configuración, ahora en
  `optimizeSafeRealPathSync`.
- `native` con pool `threads`, un worker y sin paralelismo de archivos:
  cargó la configuración y enumeró 10 archivos, pero terminó con 10 errores
  no controlados de infraestructura y **ninguna prueba ejecutada**.

El código instalado de Vite, en
`node_modules/vite/dist/node/chunks/dep-3O3b9SCF.js:6938`, llama a
`exec('net use', ...)` para examinar mapeos de unidades Windows. Ese es el
segundo punto de `spawn EPERM` observado. No demuestra que `E:` sea una unidad
de red: la llamada pertenece a una optimización de resolución de rutas Windows.

Vitest 3 utiliza `forks` por defecto y ofrece `threads`, según su
[referencia de configuración](https://v3.vitest.dev/config/#pool).
Los threads funcionaron en la sonda aislada, pero no solucionaron las otras
dependencias de subprocesos. Ni `--configLoader native` ni `--pool threads`
quedaron validados como solución completa.

### 3. Cargo: restricciones del entorno y lockfile ausente

En S, `cargo test --offline` se detuvo antes de compilar:

```text
error: no matching package named `tauri` found
location searched: crates.io index
```

Forzar únicamente `net.offline=false`, conservando el entorno S, produjo:

```text
download of config.json failed
[7] Could not connect to server
Failed to connect to index.crates.io:443 over proxy 127.0.0.1
```

En H, el mismo `cargo metadata --locked` con resolución online avanzó hasta:

```text
error: cannot create the lock file E:\packetlens\apps\packetlens-e1-vertical\src-tauri\Cargo.lock because --locked was passed to prevent this
```

Por tanto, no se reprodujo en H el bloqueo de acceso al índice observado en S.
Esto no prueba que todas las descargas y compilaciones posteriores funcionen.
El siguiente límite confirmado es que falta el lockfile del host, no un error
de compilación Rust. `--locked` debe rechazar un lockfile ausente, conforme a
la [documentación de Cargo](https://doc.rust-lang.org/cargo/commands/cargo-metadata.html#manifest-options).
No se retiró ese flag ni se generó el archivo por la restricción del usuario.

Los errores históricos Schannel `SEC_E_NO_CREDENTIALS`, WSL y Docker no se
reensayaron: no son necesarios para explicar estos resultados ni se presentan
como bloqueos vigentes confirmados de H.

### 4. Discrepancia funcional de presentación, reproducida dos veces

Archivo: `src/adapters/presentationAdapter.test.ts:33`.
Prueba: `keeps ARP queueing, CAM learning, and IPv4/ICMP CAM refresh as distinct facts`.

```text
Expected: "CAM"
Received: "Hecho del motor: delta mac_learned y protocolo del evento ethernet."
```

La prueba exige `CAM` en `pedagogicalRationale` para los eventos 4, 19 y 24.
`src/catalog/pedagogicalCatalog.ts`, rama `mac_learned`, sí utiliza `CAM` en
`summary`, pero no en la parte común de `rationale`. El adaptador asigna
`explanation.rationale` a `pedagogicalRationale`.

Esta discrepancia pertenece a contenido/pruebas de presentación. No demuestra
un defecto en aprendizaje CAM del motor. La ejecución se detiene en la primera
aserción fallida de esa prueba; no se declaran comprobadas las aserciones
posteriores. Debe decidirse la expectativa de contenido y aplicar RED -> GREEN
en un cambio autorizado, sin relajar el esperado solo para ocultar el fallo.

## Revisión de configuración

| Área | Comprobación y evaluación |
| --- | --- |
| Scripts npm | `test = vitest run`, `dev = vite`, `build = vue-tsc -b && vite build`; apuntan a herramientas locales. |
| Vitest | Plugin Vue, `jsdom`, patrón `src/**/*.test.ts`; la ejecución H encontró los 10 archivos esperados. El archivo separado tiene prioridad sobre `vite.config.ts`, como documenta [Vitest](https://v3.vitest.dev/config/). |
| Vite/Tauri dev | Puerto Vite `1420`, `strictPort: true`, `devUrl = http://localhost:1420`; H confirmó que el servidor inicia con esa configuración. |
| Distribución frontend | `frontendDist = ../dist` desde `src-tauri`; corresponde a la salida Vite producida por el build. |
| Hooks Tauri | `beforeDevCommand = npm run dev`, `beforeBuildCommand = npm run build`; el primero se ejecutó realmente en S y H. |
| Host Rust | Cargo leyó el manifiesto edition 2024, el path al spike Rust y el target `bridge_commands` mediante `metadata --no-deps`; esto no compila el host. |
| Versiones Tauri | API npm y CLI son 2.x. Falta resolución fijada del crate `tauri`; aún no se puede certificar la sincronización minor con API `2.12.4`, recomendada por [Tauri](https://v2.tauri.app/develop/updating-dependencies/#sync-npm-packages-and-cargo-crates-versions). La diferencia patch API/CLI no explica EPERM. |
| Capacidades/seguridad | `core:default`, ventana `main`, CSP sin definir, bundle desactivado. Son valores revisados, no una validación de permisos IPC ni una revisión de seguridad para distribución. |
| Dependencias adicionales | `npm ls --depth=0` marcó `detect-libc@2.1.2` y `lightningcss-win32-x64-msvc@1.33.0` como `extraneous`; ambas aparecen como opcionales en el lockfile. No se probó que causen un fallo; no se reinstaló ni depuró `node_modules`. |

La CLI H detectó MSVC, toolchain MSVC y WebView2. Son prerrequisitos de Windows
según [Tauri](https://v2.tauri.app/start/prerequisites/#windows); su presencia
detectada no sustituye una compilación y una ventana reales.

## Registro de verificaciones ejecutadas

Todos los comandos de esta tabla usan como directorio de trabajo
`E:\packetlens\apps\packetlens-e1-vertical`. `cargo` se invocó por su ruta
`C:\Users\familia\.cargo\bin\cargo.exe` en las consultas directas.

| Contexto | Comando o sonda | Resultado real |
| --- | --- | --- |
| S, dos ejecuciones | `npm test` | Ambas bloqueadas al cargar configuración: esbuild `spawn EPERM`; ninguna prueba ejecutada. |
| H | `npm test` | 10 archivos: 9 pasados, 1 fallido. 26 pruebas: 25 pasadas, 1 fallida; código de salida 1. |
| H | `npm test -- src/adapters/presentationAdapter.test.ts` | 6 pruebas: 5 pasadas, 1 fallida; mismo fallo CAM, salida 1. |
| S | `npm test -- --configLoader runner --pool threads --maxWorkers 1 --minWorkers 1 --no-file-parallelism` | Bloqueo `exec('net use')`, salida 1; sin pruebas ejecutadas. |
| S | `npm test -- --configLoader native --pool threads --maxWorkers 1 --minWorkers 1 --no-file-parallelism` | 10 errores `EPERM`, sin pruebas ejecutadas, salida 1. |
| S | `npm run build` | Salida 0; tipos y build completados; 50 módulos transformados. |
| S/H | Sondas `spawnSync` | Matriz detallada arriba; la sonda informa errores como datos, no como aserciones de Vitest. |
| S | Sondas `spawn`, IPC, `Worker`, esbuild directo/API | Resultados detallados arriba; API esbuild abortó con `EPERM`. |
| S | `npm run tauri -- dev --no-watch` | Falló `beforeDevCommand`: Vite/esbuild `spawn EPERM`, salida 1. |
| H | `$env:CARGO_NET_OFFLINE = 'true'` seguido de `npm run tauri -- dev --no-watch -- --locked` | Vite listo en 306 ms; `cargo run --locked` rechazó lockfile ausente; salida 1. Variable limitada al proceso de esa comprobación. |
| S | `cargo metadata --manifest-path src-tauri/Cargo.toml --no-deps --format-version 1 --offline` | Manifiesto y targets leídos; `resolve: null`; no valida dependencias completas. |
| S | `cargo test --manifest-path src-tauri/Cargo.toml --test bridge_commands --offline` | Bloqueada por falta de `tauri` en resolución offline; ninguna prueba Rust ejecutada. |
| S | `cargo metadata --manifest-path src-tauri/Cargo.toml --locked --format-version 1 --config net.offline=false --config net.retry=0 --config http.timeout=10` | Falló acceso al índice a través del proxy local. |
| H | Mismo comando `cargo metadata --locked` anterior | Superó el acceso inicial al índice y rechazó crear el lockfile ausente. |
| H | `npm run tauri -- info` | Entorno y configuración detectados; salida 0. No detectó versiones resueltas de `tauri-build`, `wry` o `tao`. |
| S | `npm ls --depth=0`, lectura de manifests/lock y `--help` de Vitest/Vite/Tauri | Versiones, rangos y opciones inspeccionados; no se instalaron paquetes. |
| H | Conexión TCP a `127.0.0.1:1420`; `Test-Path src-tauri/Cargo.lock` | `ECONNREFUSED`; `False`. |

No se ejecutaron `cargo test` del spike, `dotnet test`, pruebas IPC reales,
build release Tauri ni instaladores. Sus resultados históricos no se reutilizan
como verificación del estado actual.

## Soluciones y reproducción sin tocar el motor

### Comprobado ahora

Ejecutar las herramientas en una terminal Windows normal con las dependencias
ya presentes, o mediante ejecución autorizada fuera del sandbox, elimina el
`EPERM` ensayado. No requiere cambiar el motor, bajar versiones, cambiar CSP,
habilitar plugins ni sustituir esbuild.

```powershell
Set-Location E:\packetlens\apps\packetlens-e1-vertical
npm test
npm test -- src/adapters/presentationAdapter.test.ts
npm run build
npm run tauri -- info
```

Los dos primeros comandos actualmente reproducen el fallo funcional descrito;
no se esperan verdes hasta una corrección autorizada. No se ensayó `npm ci`;
reinstalar dependencias en esta copia compartida no es necesario para explicar
el diagnóstico.

Para comprobar el límite Tauri sin permitir un lockfile nuevo se ejecutó, en
un proceso temporal H, `CARGO_NET_OFFLINE=true` y:

```powershell
npm run tauri -- dev --no-watch -- --locked
```

Este comando acredita el arranque del servidor y el siguiente bloqueo; no es
una aceptación del escritorio. Una repetición desde otro proceso puede tener
una caché distinta y detenerse antes en resolución offline.

Sonda mínima reproducible para comparar contextos, sin escribir un script:

```powershell
@'
const cp = require('node:child_process');
for (const stdio of ['pipe', 'inherit', 'ignore']) {
  const r = cp.spawnSync(process.execPath,
    ['-e', 'process.stdout.write("child-ok\\n")'],
    { stdio, timeout: 5000 });
  console.log(JSON.stringify({
    stdio, status: r.status, error: r.error?.code,
    stdout: r.stdout?.toString()
  }));
}
'@ | node
```

### Siguiente cambio propuesto, pendiente de autorización

1. Resolver la discrepancia de contenido CAM con la prueba focalizada ya en
   RED. Ejecutar después esa prueba, la suite Vitest completa y el build.
2. Autorizar la generación y revisión de `src-tauri/Cargo.lock` en H, con red
   operativa; revisar las versiones resueltas y su compatibilidad con la API
   npm. No es una modificación del motor, pero sí una escritura expresamente
   excluida del alcance actual.
3. Con el lockfile revisado, obtener las dependencias y ejecutar las pruebas
   del host y la vertical real. Comandos propuestos, **no ejecutados**:

```powershell
cargo generate-lockfile --manifest-path src-tauri/Cargo.toml
cargo fetch --manifest-path src-tauri/Cargo.toml --locked
cargo test --manifest-path src-tauri/Cargo.toml --locked
npm run tauri -- dev --no-watch -- --locked
```

El primer comando crea el lockfile. Debe preceder a los siguientes solo tras
autorizar esa escritura. Si falta Cargo en otra terminal, exponer su directorio
en `PATH` únicamente para esa sesión. No se propone desactivar TLS, antivirus,
controles de seguridad ni modificar proxies globales.

La CLI local también confirma que `--no-bundle` pertenece a Tauri, antes del
separador de argumentos para Cargo. Para una futura validación del ejecutable,
tras pruebas verdes, la forma es:

```powershell
npm run tauri -- build --no-bundle -- --locked
```

La guía `docs/architecture/TAURI-E1-VERTICAL-OPERATION.md` muestra
`build -- --no-bundle`: coloca esa opción entre los argumentos del runner.
Debe corregirse en un cambio documental autorizado; no se editó aquí.
La [referencia CLI de Tauri](https://v2.tauri.app/reference/cli/#build) y el
`build --help` instalado distinguen ambas clases de argumentos.

## Hipótesis y pruebas pendientes

- **Hipótesis:** la política o implementación del sandbox restringe la creación
  de los canales Windows usados por Node. La comparación S/H la respalda, pero
  no identifica la regla concreta, ACL, API denegada ni componente responsable.
  No hay evidencia suficiente para atribuirlo a antivirus o a corrupción de Node.
- **Sin explicación confirmada:** un build S pasó mientras las sondas y Vitest S
  fallaron. No se atribuye a caché, a una carrera o a propagación de permisos sin
  instrumentación adicional. La conclusión útil es ejecutar la suite completa
  en H y conservar el resultado de cada invocación.
- **Pendiente:** compilar `tauri::generate_context!`, recursos y host nativo;
  ejecutar la prueba unitaria de mapeo de errores y `bridge_commands`; comprobar
  permisos y serialización mediante una invocación real, no solo mocks.
- **Pendiente de aceptación:** observar la ventana, pulsar **Ejecutar motor Rust**
  y comprobar procedencia `live_engine`, `presentation/v1`, 27 eventos,
  `Quiescent` y `8700` ns. También comprobar conservación de la sesión previa
  ante error. Las cuatro pruebas mock del bridge pasaron en H, pero no cubren
  este flujo real.
- **Evaluación arquitectónica:** estos resultados no justifican cambiar el motor
  ni introducir otro backend. Mantener el límite actual entre simulación, DTO,
  bridge y UI es adecuado para este diagnóstico. La mejora inmediata de
  reproducibilidad es fijar las dependencias del host y registrar por separado
  pruebas frontend, compilación nativa y aceptación IPC. Escalabilidad y usabilidad
  globales no fueron evaluadas mediante estos comandos.
