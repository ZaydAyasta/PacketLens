# ADR-0001: Errores y cancelación mínimos para Spike E1

## Estado

Aceptado para el experimento E1.

## Contexto

SPEC-E1 y su addendum definen los resultados normales de ejecución, pero no
definen la forma de representar comandos inválidos, desbordamientos de los
contadores `u64`, ni cancelación. El addendum A6 exige pruebas para
desbordamiento y respuesta a cancelación y pide documentar una decisión mínima
cuando la política no esté definida.

## Decisión

Las operaciones mutantes y de avance de `Engine` devuelven
`Result<..., EngineError>`. Un error no es un `ExecutionOutcome`: no altera el
reloj, no procesa parcialmente el siguiente evento y no emite trazas por sí
mismo.

- `TargetTimeInPast` rechaza `AdvanceUntil` hacia el pasado.
- `TimeOverflow` y `SequenceOverflow` se devuelven antes de programar un
  evento cuyo tiempo o secuencia no pueda representarse; no hay wraparound.
- Los errores de topología/configuración (`UnknownDevice`, `UnknownInterface`,
  `DuplicateId`, `EndpointAlreadyConnected`, `InvalidHostConfiguration`) se
  rechazan antes de mutar el modelo.
- `request_cancellation()` activa una señal cooperativa local al motor. El
  siguiente control de bucle de una operación de avance devuelve `Cancelled`
  sin extraer otro evento. `clear_cancellation()` permite al llamador continuar
  explícitamente. La cancelación no forma parte de la simulación ni de la
  traza.
- `max_steps == 0` es una cuota válida: si quedan eventos activos devuelve
  `BudgetExhausted`; si no, devuelve el resultado de reposo correspondiente.
- Una capacidad de traza de cero se rechaza como `InvalidTraceCapacity`.

## Consecuencias

La decisión añade una superficie de errores pequeña, necesaria para pruebas
seguras y para una futura aplicación de escritorio. No añade UI, tiempo real,
concurrencia, enrutamiento ni comportamiento de red fuera de E1.
