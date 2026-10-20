# Matriz de compatibilidad entre motores E1 e inspector Vue

Fecha: 2026-10-18. El inspector analizado es el PoC de solo lectura en
`E:\rea-lab\packetlens-research\inspector-poc`; no se modifico.

## Regla de interpretacion

`ScenarioCanonicalReport` es un contrato de comparacion E1 aceptado por
ADR-0002. Conserva orden estable y una gramatica de `details` deliberadamente
plana. No es apropiado convertir esa cadena en una API de presentacion. Hacerlo
acoplaria Vue a una gramatica de prueba y obligaria a inferir datos que el motor
no emitio estructuralmente.

Los dos motores ya conservan PDU semanticas completas en la traza en memoria.
"Completa" aqui significa el modelo E1 de Ethernet/ARP/IPv4/ICMP, no bytes de
trama reales, checksum, FCS o PCAP; A4 lo prohibe expresamente.

| Necesidad del inspector | PoC Vue actual | Datos del motor/report canonico | Estado | Adaptacion propuesta |
| --- | --- | --- | --- | --- |
| Identidad y orden | `SimulationEvent.id`; lista ordenada y cursor visual. | `TraceSeq` monotono y `records` ordenados; el canonico publica `seq`. | Compatible para eventos crudos. | Usar `trace_seq` como identidad estable; no reutilizar el indice filtrado. |
| Tiempo virtual | `timestampNs: number` y texto formateado. | `at_ns` por registro y tiempo final; ambos exactos en E1. | Compatible semantico. | Transportar ns como cadena decimal en el contrato UI; formatear solo en Vue. Evita perdida futura de precision JavaScript. |
| Dispositivo y puertos | `atDevice`, `lastDevice`, `ingressPort`, `egressPort`. | `FrameTransmitted` tiene `from`/`to`; `FrameDelivered` tiene endpoint; drops y tablas tienen dispositivo y a veces interfaz. | Parcial. | Proyectar campos estructurados por tipo de evento. No inventar puerto para actualizaciones de tabla sin puerto emitido. |
| PDU Ethernet, ARP, IPv4, ICMP | `inboundPdu`/`outboundPdu` con cabeceras. | La traza conserva frame o paquete semantico; el JSON canonico lo aplana en `details`. | Parcial para el canonico; bueno en API de motor. | Exportar PDU estructurada desde un adaptador. Distinguir `inbound`, `outbound` y ausencia. |
| Protocolo/filtro | `ARP` o `ICMP`. | Los tipos de PDU permiten clasificar; eventos de tabla/cola pueden tocar mas de un protocolo o ninguno. | Parcial. | Emitir `protocols: []` semanticos por evento; no forzar una unica etiqueta cuando no corresponde. |
| Decision de capas | `LayerDecision[]` con accion y texto. | Hay `DropReason`, `TableUpdate`, colas y resultado ping; no existe una decision OSI por capa ni texto pedagogico. | No compatible directamente. | Catalogo de explicaciones del inspector, basado en codigos semanticos del motor. No fabricar decisiones del dominio. |
| Cambios ARP/CAM | El PoC los describe narrativamente. | `TableUpdated` contiene aprendizaje/expiracion ARP y CAM; consultas actuales permiten algunos valores, no una exportacion completa de tablas. | Parcial. | Incluir deltas estructurados y, si se requiere una vista final, una consulta/exportador de snapshot de tablas. |
| Causalidad | El canvas infiere direccion con `lastDevice`; no tiene grafo. | Orden y secuencia existen, pero no hay `parent_event_id` ni id de comando de origen. | Insuficiente. | Contrato futuro con `command_id` y `caused_by` opcionales. Hasta entonces mostrar orden, no un grafo causal afirmado. |
| Topologia | Hard-codea PC1-Switch1-PC2, sus puertos y posiciones. | Los motores conocen topologia, pero no exponen un snapshot tipado de ella. | Insuficiente. | La prueba vertical debe exportar nodos, interfaces y enlaces. Coordenadas y etiquetas visuales son del cliente. |
| Retencion/truncamiento | El PoC asume 10 eventos completos. | Canonico expone capacidad, secuencia mas antigua/nueva y descartes. | Compatible pero no consumido. | La UI debe mostrar hueco de historial y no simular una traza completa si fue truncada. |
| Estado de ejecucion | No se muestra. | Tiempo final, outcome y diagnostico disponibles. | Compatible. | Mostrar outcome y estado de retencion como metadatos de corrida, no como evento. |
| Datos ilustrativos | IHL, longitud, checksum, TTL narrativo, resumen, texto pedagogico, iconos, layout. `isIllustrative` ya existe. | No provienen del modelo E1; varios estan fuera de A4. | Correctamente externos. | Mantenerlos en catalogos/presentacion con `source: illustrative`; nunca presentarlos como bytes calculados por motor. |

## Observaciones del PoC y de la investigacion externa

El PoC tiene 10 hitos manuales, mientras que E1-S2 frio del motor tiene 27
registros de traza. Esa diferencia es de granularidad pedagogica, no una
incompatibilidad de scheduler. La siguiente integracion debe poder mostrar los
27 registros crudos; una futura vista de hitos podra agruparlos con reglas
explicitas y testeadas.

`inspector-design.md` y `research-report.md` separan algunas observaciones de
flujo de control de simbolos/RTTI. Son evidencia de investigacion externa,
util para justificar que el inspector separe evento y explicacion; no cambian
la fuente normativa E1 ni autorizan copiar implementacion o textos del
software analizado.

## Decision de contrato

Conservar `ScenarioCanonicalReport` sin cambios para comparacion. Crear en la
siguiente fase un contrato separado, provisionalmente llamado
`SimulationPresentationSnapshot v1`, versionado y estructurado. Debe contener
solo hechos del motor y referencias semanticas; los textos localizados, layout,
colores, agrupaciones de hitos y campos ilustrativos pertenecen al inspector.

Esto preserva ADR-0002: el canonico sigue siendo minimo, determinista y apto
para hash; la UI recibe una forma que no necesita parsear `details`.
