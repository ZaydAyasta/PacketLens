# Evaluacion tecnica para seleccionar el motor E1

Fecha: 2026-10-18. Estado: propuesta de decision; no seleccion irreversible.

## Hechos medidos

La comparacion completa esta en
`docs/reports/E1-CROSS-SPIKE-COMPARISON.md`. Los hechos relevantes son:

- Las suites pasaron: Rust 12/12 y C# 17/17.
- Los reportes de E1-S1, S2, S3A, S3B, S4, S5 y S6 fueron identicos byte a
  byte entre implementaciones y deterministas en 100 ejecuciones independientes.
- Rust gano la carga repetida S2 fria; C# gano S6. Rust registro menor memoria
  de proceso en el procedimiento utilizado. Ninguna carga representa aun una
  topologia educativa real ni una aplicacion de escritorio.
- El motor Rust esta contenido en una unidad de 2.016 lineas de produccion y
  474 de pruebas. C# separa 1.044 lineas de produccion entre `Domain` y
  `Application`, con 405 lineas de pruebas. Las lineas no son una medida de
  calidad ni son directamente comparables entre lenguajes.
- No hay dependencias de terceros de runtime en el crate Rust ni en el proyecto
  de produccion C# E1. C# usa xUnit y el SDK de pruebas en su proyecto de tests.

## Evaluacion cualitativa

| Criterio | Rust | C# | Juicio basado en evidencia |
| --- | --- | --- | --- |
| Conformidad E1 | Completa en la suite y en los siete reportes cruzados. | Completa en la suite y en los siete reportes cruzados. | Empate para E1. |
| Determinismo observable | 100 hashes identicos por escenario. | 100 hashes identicos por escenario. | Empate para E1. |
| Rendimiento E1 medido | Mejor mediana para S2 frio; menor memoria de proceso muestreada. | Mejor mediana para S6; mayor huella de proceso en esta maquina. | No hay ganador universal. |
| Estructura actual | Dominio, scheduler y serializador estan juntos en `lib.rs`; facil de compilar, pero requerira extraer modulos antes de crecer. | Tiene separacion `Domain`/`Application`; `SimulationEngine` y el serializador aun son clases extensas. | Ventaja moderada de organizacion actual para C#. Ambos requieren refactor dirigido antes de ampliar protocolos. |
| Evidencia TDD historica | Las pruebas existentes prueban comportamiento, pero no hay bitacora que demuestre los ciclos RED/GREEN originales. | `E1-CSHARP-TDD-LOG.md` registra ciclos; varios RED fueron de compilacion y algunos de ejecucion. | Ventaja documental de C#, no prueba de superioridad del motor. |
| Interoperabilidad con Vue | Necesita un host/bridge; JSON estructurado es natural. | Necesita host/bridge; JSON estructurado es natural. | El contrato propuesto elimina la diferencia para la UI. |
| Distribucion futura | Un motor Rust encaja de forma directa si se elige host Tauri, sujeto a validar su empaquetado. | Puede publicarse autocontenido y alojarse en un host WebView o como sidecar, sujeto a validar tamano y runtimes. | Depende de la eleccion de host, no de E1. |
| Ecosistema de escritorio | Fuerte candidato para una aplicacion Vue con host nativo compacto. | Fuerte candidato para host .NET/Windows y automatizacion del ecosistema .NET. | Decidir despues de una prueba vertical, no por preferencia. |

## Inferencias y riesgos

Estas son inferencias, no resultados medidos:

1. Si se escoge Tauri como host de escritorio, un motor Rust en proceso evita
   un protocolo de proceso hijo y reduce una clase de problemas de distribucion.
   Eso no se ha implementado ni medido en PacketLens.
2. Si se desea conservar la opcion de seleccionar motor mas adelante, un
   sidecar JSON-RPC hace que Rust y C# sean equivalentes a ojos de Vue, pero
   agrega versionado, ciclo de vida, diagnostico y empaquetado de dos procesos.
3. El menor working set observado de Rust no predice el consumo de una UI ni de
   topologias grandes; incluye decisiones de runtime y del arnes.
4. La modularidad de C# es visible en el arbol actual; la granularidad de sus
   clases aun no ha sido tensionada por mas protocolos. Rust tampoco ha sido
   tensionado fuera de E1.

## Recomendacion

Recomiendo designar **Rust como candidato primario provisional**, no como
alternativa definitiva descartando C#. La razon es una combinacion de:

1. igualdad funcional demostrada;
2. menor huella de proceso en las dos cargas medidas;
3. buen resultado en la carga S2 y rendimiento competitivo, aunque no ganador,
   en S6; y
4. una ruta de integracion potencialmente mas corta si se aprueba un host Vue
   de tipo Tauri.

La recomendacion esta condicionada a aprobar esa direccion de host y a superar
la prueba vertical E1-S2 definida en el plan. C# debe permanecer como
implementacion independiente de referencia durante la siguiente fase: protege
contra regresiones semanticas y mantiene una alternativa real si el host .NET
o un sidecar demuestra menor riesgo de distribucion.

No se recomienda iniciar nuevos protocolos, convertir el spike directamente
en producto, ni compartir codigo de motor entre lenguajes antes de esa
validacion.

## Refactors recomendados antes de crecer

No se ejecutan en esta fase; son condiciones de salud arquitectonica:

- Rust: extraer, sin cambiar comportamiento, modulos de dominio, scheduler,
  topologia, traza y serializacion. Mantener el crate independiente y las
  pruebas actuales como red de seguridad.
- C#: separar el exportador del contrato de presentacion del
  `SimulationEngine`; conservar el dominio libre de JSON/UI. Reducir el tamano
  de los metodos del engine solo tras caracterizar su comportamiento con tests.
- Ambos: introducir una capa de exportacion de lectura tipada. No usar
  `ScenarioCanonicalReport.details` como formato de intercambio con Vue.
