# Contrato `SimulationPresentationSnapshot v1`

Estado: experimental, implementado por los dos spikes E1 para E1-S2 frío.

## Propósito y límites

El contrato expresa lo que el motor E1 observó para un consumidor de
presentación. No es un formato de comparación canónica, no reemplaza
`ScenarioCanonicalReport` (ADR-0002), no serializa tramas reales y no incluye
texto pedagógico, posiciones, causalidad ni razonamiento OSI inferido.

La versión exacta es `presentation/v1`. Todas las cantidades potencialmente
mayores que la precisión segura de JavaScript son cadenas decimales sin signo:
`trace_seq`, `at_ns`, tiempos de expiración, contadores, TTL e identificadores
ICMP.

## Raíz

| Campo | Tipo | Regla |
| --- | --- | --- |
| `schema_version` | cadena | Debe ser `presentation/v1`. |
| `scenario_id`, `run_label` | cadena no vacía | `E1-S2` y `cold` en la primera vertical. |
| `topology` | objeto | Dispositivos, interfaces y enlaces estables. |
| `execution` | objeto | `final_virtual_time_ns` y resultado del motor. |
| `trace_retention` | objeto | Contadores y límites de la traza disponible. |
| `events` | arreglo ordenado | Registros observables retenidos; no hitos manuales. |

## Topología e invariantes

`devices[]` contiene `{id, kind}`, donde los valores v1 son `host` y
`l2_switch`. `interfaces[]` contiene `{id, device_id, name}`.
`links[]` contiene `{id, a_interface_id, b_interface_id}`.

- Los identificadores de dispositivo, interfaz y enlace son únicos.
- Toda interfaz referencia un dispositivo existente.
- Los dos extremos de cada enlace existen y son distintos.
- No se incluyen posiciones, iconos ni etiquetas didácticas.

## Ejecución y retención

`execution.outcome` es uno de `Idle`, `Quiescent`, `TargetTimeReached`,
`BudgetExhausted` o `HaltedBySafetyGuard`.

`trace_retention` tiene `total_emitted_ever`, `dropped_by_capacity`,
`oldest_available_trace_seq` y `newest_available_trace_seq`.
La cantidad de eventos disponibles debe ser `total_emitted_ever -
dropped_by_capacity`; si está vacía los dos límites son `null`, y si no lo está
coinciden con el primer y el último `trace_seq`.

## Evento

Cada evento contiene:

```text
trace_seq, at_ns, kind, actor, ingress, egress, destination, link_id,
protocols, pdu, table_delta, drop_reason, ping
```

`trace_seq` es contiguo en la traza retenida y `at_ns` no decrece. Los
discriminantes de `kind` son `frame_transmitted`, `frame_delivered`,
`packet_dropped`, `arp_packet_queued`, `table_updated`, `ping_completed` y
`ping_failed`.

Los extremos de evento se expresan como `{device_id, interface_id}`. Los
campos se establecen en `null` cuando la traza E1 no los emite; no se rellenan
por inferencia.

## PDU y cambios de tabla

`pdu` es `{disposition, pdu}`. `disposition` es `inbound`, `outbound`,
`queued` o `dropped`.

- Ethernet: `kind: "ethernet"`, MAC fuente/destino y payload ARP o IPv4.
- ARP: operación `request`/`reply`, MAC/IP de emisor y objetivo.
- IPv4: `src_ip`, `dst_ip`, `ttl` y payload ICMP.
- ICMP: tipo, identificador y secuencia.

`table_delta.kind` es `arp_learned`, `arp_entry_expired`, `mac_learned` o
`mac_entry_expired`. `drop_reason` y `ping.failure_reason` solo usan códigos
directamente emitidos por el motor.

## Validación y errores del bridge

Los dos exportadores validan su snapshot antes de devolverlo. El consumidor Vue
vuelve a validar versión, cadenas decimales con `BigInt`, referencias de
topología, orden/retención y discriminantes de PDU/tabla antes de renderizar.

El bridge Tauri experimental expone `contract_version` y `run_e1_s2_cold`.
Si el motor falla, el comando reserva el código `engine_execution_failed`; una
proyección inválida usa `presentation_contract_invalid`. Vue rechaza una versión
distinta antes de solicitar la simulación.
