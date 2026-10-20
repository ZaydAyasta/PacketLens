# Comparacion definitiva Rust vs. C# - Spike E1

Fecha de verificacion: 2026-10-18.

## Alcance y fuentes normativas

La referencia de comportamiento es `SPEC-E1-v1.0.md` junto con
`SPEC-E1-addendum.md`; el addendum prevalece. En particular, A3 exige igualdad
de observables y no de la descomposicion interna del scheduler. ADR-0002 fija
el formato de `ScenarioCanonicalReport` para comparacion, no para UI.

Esta revision sustituye la comprobacion cruzada anterior, que solo cubria
E1-S2 frio. No se modificaron los motores, las especificaciones ni los
artefactos de `E:\rea-lab\packetlens-research`.

## Verificaciones ejecutadas

| Artefacto | Comando | Resultado observado |
| --- | --- | --- |
| Rust E1 | `cargo test` desde `spikes/spike_e1_rust` | 12 aprobadas, 0 fallidas; 0 doc-tests. |
| C# E1 | `dotnet test tests\PacketLens.SpikeE1.Tests\PacketLens.SpikeE1.Tests.csproj --no-restore` desde `spikes/spike_e1_csharp` | 17 aprobadas, 0 fallidas, 0 omitidas. |
| Evaluador Rust | `cargo build --release` desde `tools/e1-evaluation/rust` | Correcto. |
| Evaluador C# | `dotnet build PacketLens.E1Evaluation.csproj -c Release --no-restore` desde `tools/e1-evaluation/csharp` | Correcto; 0 advertencias, 0 errores. |
| Inspector PoC | `npm test` desde `E:\rea-lab\packetlens-research\inspector-poc` | No se descubrieron pruebas: Vite fallo al cargar configuracion con `spawn EPERM`. Es un fallo de infraestructura del entorno restringido, no un resultado satisfactorio ni un fallo funcional demostrado del PoC. |

El arnes `tools/e1-evaluation` construye cada topologia mediante las APIs
publicas de cada spike. No comparte implementacion de protocolos ni usa el
motor de un lenguaje desde el otro.

## Comparacion canonica S1-S6

Se ejecuto `reports` en ambos arneses. E1-S3 conserva sus dos subescenarios
normativos y por eso aparece como S3A y S3B. E1-S2 ejecuta su parte fria y,
tras avanzar a 10.000 ns, su parte caliente; por ello su estado final es
14.300 ns. Cada fila compara el JSON completo, incluido `diagnostics`; la
igualdad diagnostica es un hecho de esta corrida, no un requisito de A3.
En escenarios con varios comandos, el numero diagnostico procede del
`ExecutionReport` de la operacion terminal del arnes, no de una suma de todas
las operaciones previas; A3 impide usarlo como criterio de equivalencia.

| Escenario | Tiempo final ns | Outcome | Registros retenidos | Eventos internos diagnosticos | Bytes | SHA-256 | Rust = C# byte a byte |
| --- | ---: | --- | ---: | ---: | ---: | --- | --- |
| E1-S1 | 4.300 | `TargetTimeReached` | 16 | 10 | 4.342 | `d809a7e38c545e1fde4140955a051d7d683567252dda91d788dcc0b7f826b104` | Si |
| E1-S2 | 14.300 | `Quiescent` | 38 | 8 | 9.873 | `4c39b72802f296b672502a9488fe193dcceb8ba163faf610944cc815c74df7a1` | Si |
| E1-S3A | 3.000.000.000 | `Quiescent` | 27 | 21 | 7.527 | `b09a16f0c597264153d818bbcfd759de857302c412d9ce677dd14b9ca8ea8d01` | Si |
| E1-S3B | 1.500 | `TargetTimeReached` | 3 | 1 | 1.073 | `4f224e2494d35cb577e663b5c8b21446473f3176a6bc5a4b83f9a49294bf48a0` | Si |
| E1-S4 | 24.300 | `Quiescent` | 38 | 8 | 9.873 | `a551f74d03acd14b43ab595338171bb4a364d687786f61a4b27f02f051655d7b` | Si |
| E1-S5 | 61.000.008.700 | `Quiescent` | 67 | 18 | 17.255 | `fee2cb25e4e05367ab5051e7256076a8d3490c8ef99884725fa8e9f17d581b97` | Si |
| E1-S6 | 184.700 | `BudgetExhausted` | 500 | 1.000 | 134.993 | `5eabbf915fe2dd66a97a725a24c3dc34510a65a876ae974fa42a9ff2f4703407` | Si |

Ademas, se ejecutaron 100 instancias independientes de `reports` por cada
lenguaje. Para cada uno de los siete reportes, cada lenguaje produjo un unico
SHA-256. Las 14 parejas lenguaje/escenario conservaron exactamente el hash de
la tabla; por tanto tambien hubo identidad cruzada en las 100 repeticiones.

## Resultado contra los escenarios normativos

Los JSON completos anteriores incluyen las trazas ordenadas y los valores de
retencion. Las afirmaciones de las suites confirman adicionalmente el estado
consultable de ARP/CAM y los casos de control:

| Caso | Resultado medido en ambos spikes |
| --- | --- |
| S1 | ARP de `pc1` listo a 4.300 ns, cola pendiente vacia y dos entradas CAM. |
| S2 | RTT frio 8.700 ns; RTT caliente 4.300 ns; no se entrega trafico caliente a `pc3`. |
| S3A | Tres intentos ARP; fallo y limpieza a 3.000.000.000 ns. |
| S3B | La trama ligada a la instancia vieja se descarta a 1.000 ns. |
| S4 | La MAC de `pc2` migra a `port4`, sin duplicado; RTT 4.300 ns. |
| S5 | La entrada ARP expira a 60.000.004.300 ns y el siguiente ping reemite ARP. |
| S6 | 1.000 pasos, `BudgetExhausted`, 500 trazas retenidas y metadatos de truncamiento coherentes. |

No se detecto incompatibilidad observable E1 entre ambos. Las diferencias de
organizacion son internas y permitidas: Rust concentra 2.016 lineas de
produccion y 474 de pruebas en `lib.rs`; C# separa 1.044 lineas de produccion
en `Domain` y `Application`, con 405 lineas de pruebas. Estas cifras son una
descripcion de estructura, no una medida universal de mantenibilidad.

## Benchmark reproducible

El arnes esta versionado en `tools/e1-evaluation`. Se compilo en Release y se
ejecuto `tools/e1-evaluation/Measure-E1.ps1`. El script hace tres procesos
frescos por par carga/lenguaje, muestra la mediana y valida una suma de control
para impedir que el trabajo sea eliminado. `EngineElapsedMs` se mide dentro del
arnes alrededor de la carga mediante `Instant`/`Stopwatch`, fuera del motor y
sin afectar el tiempo virtual. `ProcessElapsedMs` incluye arranque. La memoria
es el maximo de `WorkingSet64` y de memoria privada muestreada cada 10 ms.

| Carga equivalente | Repeticiones | Mediana Rust: motor / proceso ms | Mediana C#: motor / proceso ms | Pico WS Rust / C# MiB | Privada muestreada Rust / C# MiB |
| --- | ---: | ---: | ---: | ---: | ---: |
| E1-S2 frio, 100.000 ejecuciones (checksum `871800000`) | 3 | 1.142,941 / 1.169,585 | 1.317,008 / 1.394,859 | 4,648 / 47,820 | 0,824 / 28,121 |
| E1-S6, 5.000 ejecuciones de 1.000 pasos (checksum `928500000`) | 3 | 2.155,267 / 2.188,211 | 1.623,743 / 1.698,782 | 5,516 / 45,613 | 1,559 / 25,980 |

Hechos medidos: Rust fue mas rapido en S2 frio y C# fue mas rapido en S6 en
esta maquina. Rust tuvo menor consumo de proceso segun esta medicion. No es
valido inferir una victoria global: son solo dos cargas E1, con runtime y JIT
incluidos.

Entorno verificable: Windows x64, RID `win-x64`, SDK .NET 10.0.302, runtime
.NET 10.0.10, `rustc 1.99.1`, `cargo 1.99.1`, Node 24.11.1. La consulta CIM de
CPU y RAM fue denegada por el sandbox; por ello no se atribuye el resultado a
un modelo de procesador ni a una cantidad de memoria inventada.

Para repetir:

```powershell
$env:Path = 'C:\Users\familia\.cargo\bin;' + $env:Path
cargo build --release --manifest-path tools\e1-evaluation\rust\Cargo.toml
dotnet restore tools\e1-evaluation\csharp\PacketLens.E1Evaluation.csproj
dotnet build tools\e1-evaluation\csharp\PacketLens.E1Evaluation.csproj -c Release --no-restore
.\tools\e1-evaluation\Measure-E1.ps1
```

Limitaciones: no hay afinidad de CPU, aislamiento de procesos, perfilador,
AOT, medicion de asignaciones por motor ni muestreo de alta resolucion. La
memoria privada muestreada puede no capturar un pico menor a 10 ms y el working
set incluye runtime y arnes. Estos numeros son una linea base reproducible,
no un benchmark de producto.

## Incidencias de la evaluacion

1. La primera ejecucion amplia de 20 muestras excedio el limite de una llamada
   de automatizacion y no produjo una tabla; fue descartada.
2. La primera version de `Measure-E1.ps1` uso el separador `_` no admitido por
   PowerShell en un literal numerico. Su parser fallo antes de ejecutar cargas;
   se corrigio a `1000000` y la version actual se ejecuto correctamente.
3. Antes de su correccion, el proyecto C# del arnes tenia una estructura de
   programa invalida. El `dotnet build` la detecto y el build posterior paso.

Ninguna de esas incidencias pertenece al codigo de los motores; se incluyen
para no presentar resultados como si el procedimiento hubiera sido lineal.
