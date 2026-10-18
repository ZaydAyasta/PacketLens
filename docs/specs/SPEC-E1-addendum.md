# Addendum de implementación para SPEC-E1-v1.0

**Estado:** Correcciones acordadas para las implementaciones experimentales Rust y C#.

Leer este documento **junto con** `SPEC-E1-v1.0.md`. Cuando exista conflicto entre ambos, este addendum tiene precedencia para E1. No modifica el alcance de los seis escenarios ni convierte el experimento en un producto.

## A1. Transmisión diferida como evento programable

La especificación original enumera `FrameArrived` y eventos de temporizador, pero exige transmisiones después de 100 ns de procesamiento. Ambas implementaciones deben incluir un evento interno de transmisión programada (por ejemplo `TransmitFrame`/`FrameTransmitReady`) o mecanismo semánticamente equivalente. Cuando se ejecuta en su tiempo programado, debe comprobar las condiciones actuales de la interfaz y el enlace antes de emitir `FrameTransmitted` y programar `FrameArrived`. El modelo conserva los tiempos externos esperados de E1: 8.700 ns para primer ping y 4.300 ns para segundo ping con caché caliente.

## A2. Quiescencia: temporizadores ARP obsoletos no mantienen el motor activo

Al completarse la resolución ARP en t=4.300 ns, el reintento previsto para t=1.000.000.000 ns queda obsoleto por la época del estado de resolución. `RunUntilQuiescent` considera **solo eventos futuros activos**: el temporizador obsoleto no provoca avance de `VirtualTime` hasta 1 s. El primer ping termina en `Quiescent` con `now=8.700 ns`. Se permite conservar físicamente entradas obsoletas dentro del heap hasta su extracción futura; al calcular quiescencia deberán tratarse como inactivas sin efectos secundarios observables. Los temporizadores pasivos de expiración tampoco impiden la quiescencia. `AdvanceUntil(t)` sí avanza hasta t tras procesar en orden los eventos vigentes hasta ese tiempo.

## A3. Comparación Rust vs. C#: mismo comportamiento observable, no mismo diseño interno

Las dos implementaciones deben coincidir en estados de red finales, eventos observables normativos y su orden causal/temporal, paquetes enviados/recibidos, descartes, resultados de ping y tiempo virtual final. **No exigir igualdad del número de eventos internos procesados**: las implementaciones pueden descomponer de forma diferente una transmisión diferida. Registrar ese número como métrica diagnóstica de cada motor, pero no usarlo como aserción de equivalencia. `ScenarioCanonicalReport` debe distinguir métricas informativas de condiciones de aceptación. Si hay diferencias en observaciones no normativas, documentarlas sin inventar una falsa equivalencia.

## A4. Representación semántica de paquetes

Los modelos `EthernetFrame`, `ArpPacket`, `Ipv4Packet`, `IcmpPacket` de E1 son **representaciones semánticas simplificadas**. No afirmar que conservan todos los bytes de las tramas reales ni que ya es posible exportarlos con fidelidad a PCAP. Verificar por separado checksums, longitudes, campos reservados, FCS y serialización real cuando se implementen posteriormente.

## A5. Reportes canónicos

El JSON mostrado en `SPEC-E1-v1.0.md` es ilustrativo, no una muestra completa que satisfaga sus contadores. Cuando se genere un reporte real, `total_emitted_ever`, `dropped_by_capacity`, `records`, `oldest_available_seq` y `newest_available_seq` deben mantener sus invariantes. La comparación no debe depender del orden de claves JSON ni de metadatos no normativos; fijar un esquema y un orden canónico de registros.

## A6. Garantías de pruebas mínimas

Además de las seis pruebas de E1, incluir pruebas enfocadas en: (a) temporizador ARP obsoleto y quiescencia a 8.700 ns; (b) enlace desconectado y reconectado durante la transmisión; (c) `AdvanceUntil` sin eventos activos; (d) rechazo de eventos en el pasado; (e) desbordamiento de tiempo/secuencia manejado sin pánico ni wraparound silencioso; y (f) ejecución por cuotas y respuesta a cancelación. Si el protocolo normativo aún no define una política para alguno de estos errores, no inventarla silenciosamente: registrar una decisión mínima en `docs/decisions/` antes de implementarla.

## A7. Límites de investigación

REA es una herramienta auxiliar, no una dependencia del motor ni un permiso para modificar software propietario. Basar el simulador en RFC, estándares abiertos, código con licencia compatible, implementaciones propias y pruebas. Analizar ejecutables de terceros solo cuando los términos y permisos aplicables lo permitan.
