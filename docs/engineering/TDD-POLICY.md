# Política de desarrollo guiado por pruebas (TDD)

**Estado:** aprobada para PacketLens

**Ámbito:** motores Rust y C#, contratos de presentación, bridge Tauri y frontend Vue.
**Precedencia:** las especificaciones normativas, sus addenda y los ADR aplicables prevalecen sobre esta política. Esta política no autoriza cambiar un contrato, una especificación ni un golden test para acomodar una implementación.

## Propósito

PacketLens usa TDD para reducir el riesgo de cambiar un simulador determinista: primero se formula el comportamiento observable, después se implementa el mínimo necesario y finalmente se mejora la forma sin cambiar el resultado. La política busca evidencia útil, no rituales ni una multiplicación de pruebas que no detectan defectos.

Se aplica de forma proporcional. Una corrección localizada no exige una ceremonia extensa; un cambio de contrato, scheduler o límite arquitectónico sí exige evidencia más amplia.

## Fundamento y los cuatro ciclos de TDD

Esta política toma como fuente primaria *The Cycles of TDD* de Robert C. Martin. El artículo describe cuatro ciclos anidados; sus duraciones son orientativas, no cuotas ni compromisos de tiempo de entrega. Las reglas concretas de PacketLens que aparecen junto a ellos son decisiones propias del proyecto.

| Ciclo | Idea de Martin | Aplicación práctica en PacketLens | Evidencia mínima útil |
| --- | --- | --- | --- |
| **Nano** | Las Three Laws gobiernan cada paso de código productivo. | Escribir únicamente el código productivo requerido para que falle una prueba; después, únicamente el necesario para que pase. | Prueba que expresa el siguiente comportamiento y cambio de producción limitado a satisfacerlo. |
| **Micro** | RED -> GREEN -> REFACTOR. | Ejecutar una prueba focalizada en rojo, hacerla verde con la solución mínima y refactorizar solo con la suite pertinente verde. | Comando, resultado RED clasificable, resultado GREEN y, si hubo refactor, verificación posterior. |
| **Milli** | Specific -> Generic: las pruebas sucesivas hacen evolucionar la solución desde el caso concreto hacia una regla adecuada. | Antes de añadir excepciones para una fixture, buscar la regla de dominio o contrato que cubre el nuevo caso y los casos vecinos válidos. | Prueba variante con valor de borde o forma distinta, y una implementación que no depende de IDs, tiempos o datos particulares de la fixture. |
| **Primary** | Architectural Boundaries. | Revisar periódicamente dependencias, duplicación y límites entre dominio, simulación, adaptadores, contratos, bridge y UI. | Decisión o ADR cuando cambia un límite; pruebas de contrato/integración que protegen el límite. |

Referencia primaria: [Robert C. Martin, *The Cycles of TDD*](https://blog.cleancoder.com/uncle-bob/2014/12/17/TheCyclesOfTDD.html). La descripción de los cuatro ciclos procede de esa fuente; las prácticas concretas de esta política no se atribuyen a Martin.

### Ciclo nano: las Three Laws

1. No se escribe código productivo salvo para hacer pasar una prueba que falla.
2. No se escribe más prueba de la necesaria para fallar; un fallo de compilación es suficiente para ese paso, pero no demuestra comportamiento funcional.
3. No se escribe más código productivo del necesario para hacer pasar la prueba que falla.

En PacketLens esto implica que el primer paso de un nuevo comportamiento debe exponer un hecho observable: por ejemplo, que una transmisión diferida descarta por `EgressInterfaceDown`, que un snapshot rechaza una referencia de interfaz inexistente o que un filtro Vue no muta el snapshot. Un error de tipos puede formar parte de RED, pero debe ir seguido de una aserción de comportamiento antes de declarar GREEN.

### Ciclo micro: RED -> GREEN -> REFACTOR

1. **RED.** Añadir o ajustar una prueba que represente una regla normativa, un defecto reproducido o un contrato. Ejecutarla de forma focalizada y registrar el resultado real. Clasificarlo como fallo de compilación, aserción funcional o infraestructura.
2. **GREEN.** Implementar el mínimo para satisfacer la prueba. Ejecutar la prueba focalizada y el conjunto proporcional al área modificada.
3. **REFACTOR.** Mejorar nombres, estructura o duplicación sin cambiar el comportamiento. Volver a ejecutar las mismas verificaciones. Si aparece una nueva regla observable, no es refactor: vuelve a RED.

No se exige un diario artificial para cada línea. La evidencia puede ser un historial de commits, salida de comandos en el informe de tarea, una nota breve junto al cambio o registros de CI, siempre que permita distinguir los tres estados cuando importe revisarlos.

### Ciclo milli: Specific -> Generic

El ciclo Specific/Generic evita dos errores opuestos: codificar una excepción para que pase una fixture concreta y diseñar una abstracción genérica antes de que exista una necesidad real. Después de hacer verde un caso específico, el siguiente caso significativo debe preguntar qué regla cercana, permitida por la especificación, no quedaría cubierta.

Procedimiento práctico:

1. Formular primero un caso específico y observable.
2. Tras uno o pocos microciclos, elegir una variación válida que cambie la forma del dato, un borde o una transición, sin duplicar exactamente la primera prueba.
3. Generalizar solo hasta la regla que expliquen esos casos y el contrato. No anticipar protocolos, plugins, ECS ni abstracciones que E1 no necesita.
4. Si la nueva prueba obliga a condicionar por un literal de fixture, `trace_seq`, ID de dispositivo o instante particular, detenerse y buscar la regla de dominio, a menos que la propia especificación nombre ese literal como comportamiento normativo.

Ejemplos de PacketLens:

| Señal de sobreajuste | Regla adecuada |
| --- | --- |
| El scheduler trata de manera especial el temporizador o la secuencia de E1-S2. | Determinar el trabajo activo según el estado y los eventos pendientes, preservando el orden `(at, sequence)` para cualquier escenario válido. |
| El validador de `presentation/v1` acepta solo los 27 eventos del fixture. | Validar discriminantes, cadenas decimales, orden, retención y referencias para todo snapshot que declare `presentation/v1`. |
| La UI filtra con listas de `trace_seq` de los eventos 1, 4, 19 y 24. | Filtrar por los campos y discriminantes del contrato; el catálogo pedagógico puede describir esos eventos, pero no altera los hechos emitidos por el motor. |

Una prueba de variación no obliga a inventar semántica. Se deriva de la especificación, addendum, ADR o contrato. Si no hay base normativa para la generalización, se documenta la ambigüedad y se solicita decisión.

### Ciclo primary: Architectural Boundaries

En revisiones periódicas, y antes de ampliar un área sensible, comprobar:

- Dominio y simulación no dependen de Vue, Tauri, serialización ni textos pedagógicos.
- Los adaptadores de presentación proyectan hechos del motor; no reconstruyen protocolos ni inventan causalidad.
- `ScenarioCanonicalReport` y `SimulationPresentationSnapshot` mantienen responsabilidades separadas.
- Rust y C# continúan siendo implementaciones independientes comparadas por comportamiento observable, no por código compartido.
- El bridge valida versión y contrato antes de que la UI sustituya su estado.
- La duplicación representa una decisión local comprensible o se elimina mediante una abstracción que tiene más de un consumidor real.

La revisión puede producir una nota de diseño, una tarea de refactorización o un ADR. Un ADR es obligatorio si cambia un contrato público, un límite de responsabilidad o la interpretación de una regla normativa. No se introduce una capa solo para satisfacer una lista de capas.

## Taxonomía de trabajo y pruebas

Clasificar el cambio antes de elegir la evidencia. Una misma tarea puede incluir más de una categoría, pero no deben confundirse.

| Tipo de trabajo | Punto de partida | Evidencia principal | Regla de cambio |
| --- | --- | --- | --- |
| Nueva funcionalidad | Regla nueva de spec, addendum, ADR o requisito aprobado | RED funcional -> GREEN -> REFACTOR | La prueba precede al comportamiento productivo. |
| Corrección de defecto | Reproducción mínima de una salida observada incorrecta | Prueba de regresión que falla antes de corregir | No reescribir el esperado para ocultar el defecto. |
| Refactorización | Comportamiento actual caracterizado | Suite pertinente verde antes y después | No cambia contratos, outputs ni semántica observable. |
| Optimización | Hipótesis y línea base reproducible | Benchmark antes/después y pruebas de caracterización | No se justifica por Big-O aislado ni por una medición única. |
| Caracterización | Código existente sin conducta suficientemente protegida | Pruebas de entradas/salidas actuales | Describe lo observado; no convierte accidentalmente un defecto en norma. |
| Contrato | Límite serializado o API entre módulos | Validación estructural, versiones e invariantes | No acoplar consumidores a detalles privados. |
| Integración o aceptación | Flujo entre módulos o escenario normativo | Ejecución del flujo real o escenario E1 | No sustituir una integración real por mocks y declararla validada. |
| Infraestructura bloqueada | Herramienta, permiso, red o dependencia impide ejecutar | Comando, error literal, entorno y alcance afectado | No equivale a GREEN ni a falta de pruebas. |

## Independencia de las pruebas

Las pruebas deben verificar contratos observables, no copiar la implementación o consultar su estado privado sin necesidad. Se permite inspeccionar el estado de dominio cuando este forma parte del contrato del módulo bajo prueba, como una tabla ARP/CAM expuesta por un escenario, pero no para afirmar únicamente que una función interna fue llamada.

Preferir:

- Entradas, salida, trazas y estado de dominio contractual del motor.
- Invariantes del scheduler: orden total `(at, sequence)`, reloj virtual monótono, quiescencia y temporizadores obsoletos tratados según la norma.
- Validación del DTO de presentación, incluido `BigInt` para cadenas decimales y referencias de topología existentes.
- Interacciones reales en la vertical Tauri cuando se afirme que el bridge funciona de extremo a extremo.

Evitar:

- Aserciones de número de llamadas, clases privadas o estructuras internas si la conducta observable basta.
- Repetir el algoritmo bajo prueba dentro de la expectativa.
- Fixtures gigantes que ocultan qué norma se está verificando.

## Especificaciones, golden tests y contratos

SPEC-E1, su addendum y ADR aplicables son la fuente de verdad para E1. Un golden test representa comportamiento aprobado, no una expectativa que pueda editarse para convertir una regresión en éxito.

Antes de modificar una especificación, addendum, ADR, fixture dorada o esperado canónico:

1. Identificar la contradicción, defecto de la prueba o nueva decisión que lo exige.
2. Comparar las salidas observables de Rust y C# cuando corresponda.
3. Registrar la razón en el ADR o documento de decisión apropiado.
4. Modificar producción y pruebas coherentemente; nunca solo el esperado para pasar la suite.

El contrato `presentation/v1` es independiente de `ScenarioCanonicalReport`. Sus pruebas deben proteger como mínimo: `schema_version`, discriminantes, cadenas decimales que exceden la precisión segura de JavaScript, referencias topología-interfaz-enlace, orden de eventos, deltas de tablas y metadatos de retención. La presentación no atribuye al motor datos binarios, capas OSI, causalidad o decisiones que el motor no emitió.

## Bordes, invariantes, propiedades y determinismo

Las pruebas de ejemplo siguen siendo esenciales, pero se añaden pruebas de borde, propiedades o determinismo cuando eliminan una familia real de errores.

| Riesgo | Prueba apropiada |
| --- | --- |
| Empates y orden de eventos | Secuencias con igual `at`; comprobar orden `(at, sequence)`. |
| Tiempo virtual | Invariante de no retroceso y resultado al alcanzar quiescencia. |
| Timers y enlaces | Casos de expiración, temporizador obsoleto, desconexión y reconexión diferida. |
| ARP/CAM | Aprendizaje, renovación, expiración y descarte normativo con trazas observables. |
| Contratos de presentación | Generadores o tablas de casos válidos/inválidos si el coste es razonable. |
| Determinismo | Misma entrada y configuración, misma proyección observable repetida. |

No se exige property-based testing en cada módulo. Se usa cuando un conjunto de ejemplos no cubre adecuadamente un espacio combinatorio o una invariante crítica.

## Cuándo ejecutar cada nivel

| Momento | Verificación requerida |
| --- | --- |
| Durante RED/GREEN local | Prueba focalizada del cambio. |
| Antes de cerrar un cambio en Rust o C# | Suite del spike afectado y pruebas de determinismo pertinentes. |
| Cambio de contrato o exportador | Unitarias del contrato, validación de fixture generado y comparación Rust/C# si ambos lo implementan. |
| Cambio de bridge o Vue | Vitest, comprobación TypeScript/build disponible y pruebas de integración pertinentes. |
| Cambio de flujo real Tauri | Ejecución de la vertical real; no declarar validación si no se abrió e invocó exitosamente. |
| Cambio normativo o golden | Suite E1 completa, comparación cruzada y ADR/justificación antes del cambio. |

Los comandos y resultados se informan con precisión: aprobados, fallidos y no ejecutados. Un build o una compilación exitosa no sustituye una aserción funcional.

## Optimización y benchmarks

La optimización es una categoría separada de GREEN. Antes de editar producción:

1. Revisar el árbol de trabajo y no sobrescribir cambios ajenos.
2. Formular una hipótesis de coste y analizar complejidad teórica, coste real y corrección.
3. Crear una línea base reproducible: datos de entrada, versión de herramientas, modo de compilación, número de repeticiones y mediciones de tiempo y memoria cuando puedan medirse de forma fiable.
4. Añadir pruebas de caracterización o regresión de invariantes para el comportamiento que no puede cambiar.
5. Elegir un único cuello de botella demostrado; aplicar RED -> GREEN -> REFACTOR.
6. Repetir exactamente el benchmark en el mismo entorno y describir variabilidad y limitaciones.

Una iteración O(n) no es por sí misma un defecto. Una mejora solo se afirma si las mediciones comparables la demuestran; de lo contrario se conserva el código anterior y se documenta el resultado. Las optimizaciones del scheduler preservan sin excepción orden `(at, sequence)`, quiescencia, trazas, temporizadores obsoletos y resultados normativos.

## Mutation testing y cobertura

La cobertura responde qué código fue ejecutado; no demuestra que las aserciones detecten una regresión. Mutation testing evalúa selectivamente esa capacidad.

Usarlo de forma dirigida cuando exista riesgo alto o dudas sobre la calidad de pruebas, por ejemplo en:

- Comparadores de tiempo y secuencia del scheduler.
- Condiciones de descarte de enlaces e interfaces.
- Validadores de discriminantes, referencias y precisión decimal del contrato.
- Transformaciones de ARP/CAM con efectos observables.

No se establece un porcentaje global obligatorio ni se bloquea un cambio pequeño por mutantes irrelevantes. Cada mutante superviviente se clasifica: prueba faltante, equivalente, fuera de alcance o decisión pendiente. La cobertura y mutación no sustituyen las pruebas de aceptación normativas.

## Bloqueos de infraestructura

Un bloqueo se registra de forma honesta y acotada con:

- Comando exacto y resultado literal relevante.
- Fecha, plataforma y versión de herramienta cuando estén disponibles.
- Si el problema es permiso, proxy/TLS, dependencia, compilación, ejecución o entorno gráfico.
- Qué pruebas sí se ejecutaron y cuál es exactamente el alcance no verificado.
- Prerrequisito o acción segura para repetirlo fuera del entorno bloqueado.

Ejemplos: `spawn EPERM` de `esbuild`, imposibilidad de descargar dependencias Cargo por proxy o una ventana Tauri no ejecutable en sandbox. Ninguno significa que la prueba haya pasado; tampoco autoriza desactivar TLS, controles de seguridad ni cambiar de arquitectura solo para eludir el sandbox.

## Contradicciones normativas y condiciones de detención

Detener la implementación y comunicar la contradicción cuando un requisito propuesto:

- contradiga SPEC-E1, su addendum o un ADR aplicable;
- requiera cambiar un resultado golden sin explicación normativa;
- exija datos que el motor no produce y que se presentarían como hechos;
- deshaga la independencia entre Rust y C#; o
- amplíe el alcance sin aprobación explícita.

Primero se presenta la evidencia concreta, la menor decisión que resolvería el conflicto y su impacto. Solo se continúa tras una aclaración, addendum o ADR. No se resuelve la contradicción editando una prueba para que la suite quede verde.

## Lista de comprobación proporcional

Antes de entregar un cambio, confirmar lo que aplique:

- [ ] Se clasificó el trabajo y se identificó la fuente normativa o contractual.
- [ ] La prueba nueva falla funcionalmente antes de la implementación, o se registró por qué solo fallaba compilación/infrastructura y se añadió después una aserción funcional.
- [ ] La implementación mínima hizo verde la prueba y el refactor no cambió conducta observable.
- [ ] Se aplicó el ciclo Specific/Generic cuando aparecieron casos vecinos, sin sobre-generalizar.
- [ ] Las pruebas no duplican el algoritmo ni dependen innecesariamente de detalles privados.
- [ ] Se ejecutó el nivel de prueba proporcional al límite afectado y se reportaron resultados reales.
- [ ] Los goldens, contratos y especificaciones no se modificaron para ocultar una regresión.
- [ ] Cualquier bloqueo se declaró como no verificado, con evidencia y prerrequisito de repetición.
- [ ] Cuando correspondía, se revisaron límites arquitectónicos, determinismo, complejidad y comparación cruzada.

## Aplicación por área

**Rust y C#.** Las implementaciones son independientes. Cada una deriva comportamiento de la especificación, y la comparación cruzada contrasta proyecciones observables, no la estructura del código. Una discrepancia requiere clasificarla como variación interna permitida o incompatibilidad observable antes de corregirla.

**Vue y Tauri.** Vue consume snapshots validados y no reimplementa ARP, CAM ni reglas del scheduler. El bridge comprueba versión, ejecuta el motor, valida el snapshot y mantiene el estado anterior ante error. Los textos pedagógicos se prueban como contenido o presentación, separados de hechos del motor.

**Documentación y ADR.** Los informes de pruebas contienen comandos y resultados realmente obtenidos. Un ADR conserva la razón de una decisión que modifica interpretación normativa, contrato o límite arquitectónico; no reemplaza las pruebas que demuestran el comportamiento.
