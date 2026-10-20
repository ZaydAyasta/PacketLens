# ADR-0002: Esquema canonico de reportes E1 entre spikes

## Estado

Aceptado para E1.

## Contexto

SPEC-E1 pide un `ScenarioCanonicalReport` comparable y el addendum A5 aclara
que el ejemplo JSON no fija el orden de claves, pero si exige invariantes de
traza y un esquema estable. La representacion concreta de una PDU dentro de
cada registro no quedo definida de forma suficiente para comparar dos
serializadores independientes.

## Decision

Los dos spikes serializan, sin espacios, las claves de raiz en este orden:
`scenario_id`, `final_virtual_time_ns`, `execution_outcome`, `diagnostics`,
`trace_summary`. La metrica diagnostica queda en
`diagnostics.total_internal_events_processed` y no se usa como criterio de
equivalencia, conforme a A3.

Cada registro de `trace_summary.records` tiene exactamente las claves
`seq`, `at_ns`, `kind` y `details`, en ese orden. `details` es una gramatica
ASCII estable de pares `clave=valor` separados por `;`. Las PDU se conservan
por completo mediante `eth(...)`, `arp(...)` e `ipv4(...)`, con MAC en
minusculas y direcciones IPv4 decimales. Esto permite comparar el contenido
de una PDU sin convertirla en una afirmacion de serializacion Ethernet real.

Los contadores y las secuencias de traza siguen siendo valores numericos JSON;
las claves con ausencia usan `null`.

## Consecuencias

Se puede comparar JSON byte a byte para una misma ejecucion de escenario, o
campo a campo si una futura herramienta reordena claves. No se comparte motor,
tipos internos ni codigo de protocolos entre Rust y C#; solo se implementa el
mismo contrato de salida normativo.
