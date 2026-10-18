# Spike E1 en Rust

Implementación Rust independiente y headless del motor de eventos discretos
definido por `SPEC-E1-v1.0.md` y su addendum. No comparte motor, código ni
dependencias con la futura implementación C#.

## Ejecutar

En el equipo en que se creó el spike, Rust está instalado pero su directorio no
está agregado al `PATH` de PowerShell. Desde este directorio:

```powershell
$env:Path = (Join-Path $env:USERPROFILE '.cargo\bin') + ';' + $env:Path
cargo fmt --all -- --check
cargo test
cargo clippy -- -D warnings
```

El crate no tiene dependencias externas ni usa reloj del sistema, hilos, tiempo
real o aleatoriedad para la simulación.

## Modelo

- `Engine` almacena topología, reloj virtual, heap mínimo determinista y buffer
  circular de trazas.
- El heap ordena por `(at_ns, secuencia)` y usa enteros `u64` con comprobación
  de desbordamiento.
- `TransmitFrame` es el evento diferido que exige el addendum A1; valida el
  estado vigente de interfaz y enlace justo antes de transmitir.
- Cada enlace posee una identidad interna monótona que no se reutiliza. Así una
  llegada ya programada no puede entregarse por un cable reconectado.
- Hosts implementan ARP, IPv4 local e ICMP Echo; switches aprenden MAC y
  inundan puertos en orden lexicográfico mediante `BTreeMap`.
- `ScenarioCanonicalReport::to_canonical_json()` produce un JSON con orden de
  claves fijo. El total de eventos internos se ubica bajo `diagnostics`, pues
  el addendum A3 lo clasifica como métrica informativa y no criterio de
  equivalencia entre lenguajes.

## Decisiones y límites

La política mínima para errores, desbordamiento, cuota cero y cancelación está
en [`ADR-0001`](../../docs/decisions/ADR-0001-e1-engine-errors-and-cancellation.md).

El modelo cubre exclusivamente E1: hosts en la misma subred IPv4 y switches L2.
No implementa routing, DHCP, DNS, VLAN, serialización de bytes, FCS, checksums
ni PCAP. Las PDU son representaciones semánticas, tal como aclara el addendum
A4.
