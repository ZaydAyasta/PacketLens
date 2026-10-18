# Reporte de validación — Spike E1 Rust

Fecha de ejecución: 2026-10-18.

## Entorno observado

- .NET SDK 10.0.302 está disponible.
- Rust 1.99.1 y Cargo 1.99.1 están instalados en
  `C:\Users\familia\.cargo\bin`, pero no estaban en el `PATH` inicial de
  PowerShell. Las ejecuciones se hicieron anteponiendo esa ruta para la sesión,
  sin cambiar la configuración global.
- El directorio de trabajo no contiene todavía metadatos `.git`; por ello no
  hubo un estado Git preexistente que revisar.

## Alcance implementado

`spikes/spike_e1_rust` es un crate Rust sin interfaz gráfica ni dependencias
externas. Incluye topología, reloj virtual, scheduler por `(timestamp,
secuencia)`, transmisiones diferidas, enlaces con identidad privada no
reutilizable, Ethernet semántico, ARP, IPv4 local, ICMP Echo, switch L2,
aprendizaje/expiración CAM, buffer circular de trazas y JSON canónico.

No se modificaron `SPEC-E1-v1.0.md` ni el addendum. La única política ausente
de las fuentes normativas (errores de comandos, overflow, cuota cero y
cancelación solicitada por A6) quedó documentada antes de implementarse en
[`ADR-0001`](../decisions/ADR-0001-e1-engine-errors-and-cancellation.md).

## Comparación contra los comportamientos normativos

| Caso | Comportamiento verificado por prueba | Resultado observado |
| --- | --- | --- |
| E1-S1 | ARP listo al avanzar a 4 300 ns; dos entradas CAM | `TargetTimeReached`, reloj 4 300 ns |
| E1-S2 | Ping frío, timer ARP obsoleto no prolonga reposo; ping caliente | RTT 8 700 ns, luego RTT 4 300 ns y reloj 14 300 ns |
| E1-S3A | Tres intentos ARP, limpieza y fallo de ping | `Quiescent` a 3 000 000 000 ns |
| E1-S3B | Cable sustituido con trama en vuelo | descarte `LinkDisconnectedInFlight` a 1 000 ns |
| E1-S4 | PC2 migra a `port4` sin MAC duplicada | CAM con dos entradas; ping completo a 24 300 ns, RTT 4 300 ns |
| E1-S5 | TTL ARP de PC1 no se renueva por uso | presente a 60 000 004 299 ns, expira a 60 000 004 300 ns y se solicita ARP de nuevo |
| E1-S6 | Bucle L2 y capacidad de traza 500 | `BudgetExhausted`, 1 000 eventos, 500 trazas y contadores de truncamiento coherentes |
| A6 | avance vacío, destino en pasado, overflow, cancelación | rutas explícitas sin pánico ni wraparound |
| Scheduler | guardas de tiempo cero y cola | detiene al evento 1 001 en el mismo tiempo; rechaza el evento 100 001 en cola |
| Determinismo | 100 ejecuciones del JSON canónico E1-S2 | mismo SHA-256 en cada ejecución; función contrastada con vector estándar de `abc` |

De acuerdo con el addendum A3, el número de eventos internos se conserva como
diagnóstico, pero no se compara como criterio de equivalencia entre Rust y C#.

## Verificación ejecutada

Comando final ejecutado desde `spikes/spike_e1_rust`:

```powershell
$env:Path = 'C:\Users\familia\.cargo\bin;' + $env:Path
cargo fmt --all -- --check
cargo test
cargo clippy -- -D warnings
```

Resultado final: formato correcto; `cargo test` ejecutó 12 pruebas, con 12
aprobadas, 0 fallidas; doc-tests: 0 ejecutadas, 0 fallidas; Clippy terminó sin
advertencias.

Durante la primera verificación `cargo fmt -- --check` señaló formato no
normalizado. Se aplicó `cargo fmt`; la comprobación final indicada arriba pasó.
La primera ejecución de `cargo test` ya tuvo 10/10 pruebas aprobadas. No hubo
pruebas fallidas.

## Diferencias y límites conocidos

- El prototipo C# todavía no existe. Por tanto no se ejecutó ni se afirma una
  comparación Rust–C#; solo queda preparado el serializador canónico y el
  contrato de observables para la validación posterior.
- La identidad de enlace se implementa como contador interno monótono no
  reutilizado, en vez de exponer una `SlotMap`; conserva la propiedad observable
  exigida para invalidar tramas de un enlace destruido y no filtra esa clave por
  la API pública.
- No hay routers ni rutas entre subredes: fuera del alcance E1, que únicamente
  usa hosts de la misma subred y conmutación L2.
- Las PDU no son bytes Ethernet/IP reales ni permiten exportación PCAP fiel;
  no se implementaron checksums, longitudes, FCS o campos reservados, tal como
  limita A4.
