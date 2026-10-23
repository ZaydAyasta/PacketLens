# ADR-0005: observaciones ARP de router con ámbito de interfaz

## Estado

Aplicado al incremento 03 experimental de E2.1, 2026-10-23. Derivado de ARP-01/02 de RC1 aprobada; no modifica E1 ni un contrato de presentación.

## Contexto

El router puede tener el mismo vecino IP en interfaces distintas. Una observación de aprendizaje o expiración que identifique solo dispositivo e IP sería ambigua. RC1 exige identificar la interfaz en observaciones de dominio E2, pero mantiene `presentation/v1` y ADR-0002 protegidos.

## Decisión

- Agregar metadato opcional de ámbito a la observación de dominio: `TraceRecord.observation_interface` en Rust y `TraceEvent.ObservationInterface` en C#. En C# su escritura queda restringida al ensamblado del motor.
- Las mutaciones ARP y los paquetes encolados del router emiten la interfaz real. Los eventos de trama ya contienen sus extremos. Las observaciones E1 mantienen el campo ausente/nulo.
- No cambiar la numeración, el orden, el reloj, la retención ni el payload de los eventos E1. No agregar entrega local, capas OSI o aristas causales inferidas.
- Mantener las cachés, colas y resoluciones privadas del router separadas de configuración y presentación. Rust usa mapas por interfaz; C# usa claves compuestas en sus propios diccionarios.
- La comparación de S1 utiliza un arnés diagnóstico independiente. No amplía `ScenarioCanonicalReport` ni los DTO/exportadores v1 y no fija un golden E2.

## Consecuencias y verificación

Las pruebas observan tablas por interfaz y verifican colisiones de IP entre interfaces, renovación/expiración y FIFO. Se mantiene la comparación exacta de los siete reportes E1 y del fixture v1 E1-S2 frío.

Los temporizadores privados nuevos se clasifican como resolución activa o expiración pasiva según su estado/época; no cambia el comparador `(at, sequence)`. El índice de trabajo activo de Rust cancela una sola vez la resolución al recibir su respuesta. Los temporizadores obsoletos no generan observaciones ni bloquean configuración.

Este ADR no autoriza sustituir descartes normativos por excepciones. `RouterIcmpErrorPathPending` es únicamente un diagnóstico transitorio de ramas aún no implementadas: no representa ICMP ni un terminal de ping. Su alcance y retirada pendiente se registran en [el informe del incremento 03](../reports/E2.1-INCREMENT-03-TDD.md). No se declara conformidad de esas ramas.
