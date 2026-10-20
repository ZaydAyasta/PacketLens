# ADR-0003: `SimulationPresentationSnapshot v1` para consumidores de presentación

## Estado

Aceptado de forma experimental para la vertical E1-S2. Rust continúa como motor
primario provisional; C# conserva un exportador independiente de referencia.

## Contexto

`ScenarioCanonicalReport`, fijado por ADR-0002, sirve para comparar resultados
normativos entre spikes. Su gramática compacta no contiene topología, puertos,
PDU estructuradas ni los metadatos necesarios para que una interfaz pueda
inspeccionar una ejecución sin volver a implementar el dominio.

Además, el consumidor Vue no puede representar con seguridad todos los enteros
futuros del motor mediante `number` de JavaScript.

## Decisión

Se adopta el DTO de lectura versionado `SimulationPresentationSnapshot` con
`schema_version = "presentation/v1"`.

- El contrato es independiente de `ScenarioCanonicalReport`; no modifica
  ADR-0002 ni sus números JSON.
- Secuencias, tiempos virtuales, contadores, TTL y los identificadores de ICMP
  se emiten como cadenas decimales. El cliente valida con `BigInt` y no los
  convierte a `number`.
- La topología usa identificadores estables de dispositivo, interfaz y enlace.
  Cada enlace debe apuntar a dos interfaces existentes y distintas.
- Los eventos llevan `trace_seq`, `at_ns`, actor, puertos de entrada/salida
  cuando el motor los emitió, enlace, PDU semántica, deltas ARP/CAM, descarte o
  resultado de ping cuando corresponda.
- Las PDU conservan la encapsulación semántica Ethernet → ARP o IPv4 → ICMP;
  no declaran bytes, FCS, checksums, longitudes ni PCAP.
- No existe campo de causalidad ni decisiones OSI inferidas. Los únicos códigos
  de descarte son razones que el motor ya emitió.
- La UI puede añadir textos pedagógicos, iconos y disposición visual en un
  catálogo separado. Esos datos no son hechos del motor.

## Consecuencias

Los exportadores Rust y C# implementan el mismo contrato desde sus propias
trazas y se comparan externamente. Vue consume el snapshot validado y conserva
un cursor visual; navegar no rebobina ni modifica la simulación.

El primer transporte aprobado solo es un bridge Tauri en proceso y sigue siendo
experimental. Una futura alternativa (por ejemplo sidecar) debe conservar este
DTO o introducir una versión nueva, no reutilizar el reporte canónico como API
de UI.
