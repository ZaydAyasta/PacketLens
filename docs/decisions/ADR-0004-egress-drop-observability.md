# ADR-0004: descarte observable de transmisión diferida sin salida

## Estado

Aceptado para E1.

## Contexto

El addendum A1 exige que una transmisión programada compruebe el estado actual
de su interfaz y enlace al ejecutarse. Rust ya registraba `PacketDropped` con
`EgressInterfaceDown` o `EgressLinkUnavailable` si no podía transmitir. C#
comprobaba esos estados, pero terminaba en silencio. Por tanto, un snapshot de
un escenario A1 con salida inválida podía diferir de forma observable aunque
ambos motores evitaran `FrameTransmitted`.

SPEC-E1 y A1 no fijan explícitamente la observación de descarte de egreso. La
diferencia no aparece en E1-S1 a E1-S6, pero impediría comparar con precisión
un caso de transmisión diferida inválida si se lo expusiera al inspector.

## Decisión

Al ejecutar una transmisión diferida, ambos motores registrarán exactamente un
`PacketDropped` con la PDU Ethernet semántica original cuando no exista una
salida válida:

- `EgressInterfaceDown` si la interfaz de origen está administrativamente caída.
- `EgressLinkUnavailable` si la interfaz no tiene enlace activo o la identidad
  de enlace ya no se resuelve.

No se emitirá `FrameTransmitted` ni se programará una llegada. Si se reconecta
un enlace válido al mismo puerto antes de ejecutar el evento, se usa la nueva
identidad de enlace: no es un descarte. Una trama ya emitida conserva la regla
existente de E1-S3B y se descarta con `LinkDisconnectedInFlight` si su enlace
generacional desapareció durante el tránsito.

Esta es una decisión de observabilidad para una condición que A1 ya obliga a
comprobar. No cambia las constantes de tiempo, el orden del scheduler, los seis
escenarios, los protocolos ni ADR-0002. `presentation/v1` ya puede representar
estos códigos mediante `packet_dropped.drop_reason` y no requiere una versión
nueva.

## Consecuencias

Las dos implementaciones deben probar los dos motivos de descarte y conservar
la prueba de reconexión que transmite por el enlace vigente. Las comparaciones
canónicas E1-S1 a E1-S6 siguen sin cambios; se mantiene la comparación de
`presentation/v1` para E1-S2 frío como regresión independiente.
