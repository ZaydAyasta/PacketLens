# Borrador de politica TDD de PacketLens

**Estado:** propuesta para revision. No modifica por si misma `AGENTS.md`,
especificaciones, ADRs, codigo ni criterios de aceptacion ya aprobados.

## Proposito, alcance y precedencia

Esta politica busca que PacketLens aumente su capacidad de cambio seguro sin
convertir cambios pequenos en una ceremonia documental. Se aplica a Rust, C# y
Vue, con enfasis en el motor determinista, sus contratos de lectura y el
inspector. No exige que toda prueba sea unitaria ni que cada modificacion tenga
una bitacora manual.

Orden de precedencia:

1. `docs/specs/SPEC-E1-v1.0.md` y su addendum para E1; el addendum prevalece.
2. ADRs aceptados, incluido ADR-0002 para comparacion canonica, ADR-0003 para
   `presentation/v1` y ADR-0004 para descartes de egreso.
3. Esta politica y las convenciones locales de cada paquete.

Una prueba no autoriza a alterar una norma. Si un requisito pedido, una prueba
existente o una optimizacion contradice una especificacion o ADR, se detiene el
cambio, se cita el conflicto y se solicita una decision. Si hay una ambiguedad
material, se propone la menor decision compatible, su prueba y, cuando
corresponda, un ADR antes de modificar el comportamiento. No se edita un golden
ni una especificacion para ocultar el conflicto.

## Fundamento y adaptacion de PacketLens

Robert C. Martin describe tres escalas complementarias: las tres leyes como
nano-ciclo, RED-GREEN-REFACTOR como micro-ciclo y una revision periodica de
limites arquitectonicos. En particular, propone escribir primero una prueba que
falle, escribir solo lo necesario de prueba para producir el fallo (la
compilacion puede fallar) y escribir solo lo necesario de produccion para hacer
pasar la prueba actual. Describe tambien revisar cada cierto tiempo si el
trabajo invade un limite arquitectonico y vigilar que el codigo no se vuelva
demasiado especifico para una prueba.

Estas son las fuentes primarias consultadas:

- Robert C. Martin, [The Cycles of TDD](https://blog.cleancoder.com/uncle-bob/2014/12/17/TheCyclesOfTDD.html), para las tres escalas, RED-GREEN-REFACTOR,
  especificidad/generalidad y limites.
- Robert C. Martin, [Symmetry Breaking](https://blog.cleancoder.com/uncle-bob/2017/03/07/SymmetryBreaking.html), para la formulacion de las tres leyes y el
  orden test-first.
- Robert C. Martin, [Test Contra-variance](https://blog.cleancoder.com/uncle-bob/2017/10/03/TestContravariance.html), para evitar que la estructura de pruebas
  sea un espejo fragil de las clases de produccion.

Lo que sigue convierte esos principios en decisiones propias de PacketLens. No
se atribuyen a Martin los umbrales, tipos de prueba, comandos ni controles de
contrato que aqui se definen.

## Diagnostico de E1: hechos, no reconstruccion retrospectiva

### Fortalezas observadas

- E1 tiene oraculos normativos concretos: tiempos, eventos, orden, retencion,
  estados ARP/CAM y limites. El addendum A2, por ejemplo, permite probar que un
  timer ARP obsoleto no evita `Quiescent` a 8.700 ns.
- Rust y C# tienen suites independientes y comparacion cruzada de reportes
  canonicos. ADR-0002 evita usar el conteo de eventos internos como falso
  criterio de equivalencia, y ADR-0003 separa el contrato de presentacion del
  reporte de comparacion.
- `E1-CSHARP-TDD-LOG.md` distingue explicitamente RED de compilacion de un RED
  funcional. El caso de expiracion CAM y varias correcciones posteriores tienen
  evidencia de asercion ejecutada y fallida antes del arreglo.
- `E1-PRESENTATION-INTEGRATION-TDD-LOG.md` reconoce correctamente que una
  prueba C# que pasa al ser agregada es caracterizacion, no prueba retrospectiva
  de un ciclo TDD. Tambien conserva la distincion entre `spawn EPERM` y una
  prueba Vue verde.
- Los golden generados desde el motor se verifican con `Mode Check` y la
  comparacion de E1-S2 confronta Rust y C# byte a byte. Eso protege una frontera
  de contrato sin duplicar reglas ARP/CAM en Vue.

### Limites y oportunidades

- La evaluacion tecnica documenta que el Rust E1 original tiene pruebas de
  comportamiento, pero no evidencia historica suficiente para afirmar que se
  construyo test-first. No debe reescribirse la historia como si tuviera RED
  observados.
- Muchos RED registrados en las fases posteriores son errores de compilacion por
  una superficie inexistente. Son RED validos para la segunda ley, pero por si
  solos no prueban el resultado funcional ni sustituyen una asercion ejecutada
  tras implementar la API.
- Las pruebas Vue y de bridge fueron escritas, pero Vitest no llego a
  descubrirlas por `spawn EPERM`; la construccion o el typecheck no equivalen a
  una suite de aserciones ejecutada. La vertical Tauri tampoco queda validada
  hasta abrir el host e invocar el motor con exito.
- La auditoria reciente del scheduler incorporo caracterizacion, benchmark y
  una invariante interna. Es una buena separacion entre correccion y rendimiento,
  aunque su RED inicial de compilacion debe clasificarse como falta de superficie,
  no como prueba funcional fallida.
- No existe aun una practica acordada para pruebas por propiedades ni mutation
  testing selectivo. Tampoco se debe imponerlos indiscriminadamente a cada DTO o
  componente.

## Las tres leyes, convertidas en reglas practicables

### Ley 1. Test primero para comportamiento nuevo

Antes de cambiar produccion para un comportamiento nuevo, escribir la prueba
mas pequena que exprese una consecuencia observable de su fuente normativa o
de su requisito aprobado. Debe poder explicarse en formato dado/cuando/entonces.

Ejemplo E1: para una nueva regla de scheduler, primero una prueba que fije la
salida, tiempo virtual, estado o traza observables; no una prueba que fuerce
una estructura privada determinada.

### Ley 2. RED pequeno y clasificado

Escribir solo la parte de prueba necesaria para obtener un RED. Un fallo de
compilacion es aceptable y puede demostrar una API ausente, pero se registra
como **RED de superficie**. No cuenta como evidencia de que una regla de red,
una transicion de estado o una asercion de UI haya fallado.

Cuando la API exista, la prueba debe ejecutarse y contener una asercion sobre
el comportamiento previsto. Para un cambio normativo o defecto, se prefiere un
**RED funcional**: compilacion correcta y asercion fallida contra el
comportamiento incompleto o incorrecto.

### Ley 3. GREEN minimo

Cambiar solo la cantidad minima de produccion necesaria para satisfacer la
prueba que esta en rojo, sin adelantar protocolos, capas o casos futuros. La
prueba se vuelve a ejecutar de inmediato. Si el arreglo solo satisface una
forma literal del fixture y no casos vecinos razonables, se debe elegir una
prueba mas simple o generalizar el diseno antes de continuar.

En PacketLens, un GREEN no esta completo si cambia una frontera protegida sin
ejecutar sus pruebas correspondientes. Por ejemplo, modificar una proyeccion
`presentation/v1` requiere su validacion de contrato; modificar resultados E1
requiere ademas las verificaciones normativas y, cuando aplique, comparacion
cruzada Rust/C#.

## Micro-ciclo RED-GREEN-REFACTOR

El micro-ciclo es la unidad normal de trabajo para funcionalidad nueva y
defectos:

1. Elegir un comportamiento pequeno y la fuente que lo exige.
2. Crear el RED y ejecutar el comando mas estrecho que pueda demostrarlo.
3. Implementar el minimo; ejecutar de nuevo hasta obtener GREEN.
4. Refactorizar solo si mejora claridad, duplicacion, acoplamiento o diseno.
   Durante un refactor sin cambio observable, las pruebas relevantes permanecen
   verdes antes, durante y despues de cada paso pequeno.
5. Ampliar con el siguiente borde, invariante o caso, no con una gran bateria
   especulativa de una vez.

No hace falta un documento RED/GREEN/REFACTOR para cada cambio de tres lineas.
La evidencia normal es el test que nombra el comportamiento, el historial o
diff y el comando ejecutado. Se pide una nota breve en el resumen de cambio
solo cuando haya una de estas condiciones: regla normativa, correccion de
defecto, contrato/golden, optimizacion, refactor transversal, o bloqueo de
infraestructura. La nota debe decir que tipo de RED hubo y que comando termino
verde; no necesita narrar cada nano-ciclo.

## Taxonomia obligatoria de trabajo y evidencia

| Tipo de trabajo | Secuencia requerida | Evidencia suficiente | No debe afirmarse |
| --- | --- | --- | --- |
| Funcionalidad nueva | Test-first, RED, GREEN, refactor opcional. | Prueba ejecutada con aserciones del nuevo comportamiento; pruebas de frontera aplicables. | Que un error de compilacion cubre la semantica. |
| Defecto | Reproductor de regresion primero; RED funcional si el entorno lo permite; correccion minima; GREEN. | La prueba falla por el defecto y luego pasa; si no se puede reproducir, se declara expresamente y se pide datos. | Que una prueba escrita despues demuestra TDD del codigo previo. |
| Refactor sin cambio observable | Suite relevante verde antes/durante/despues; pasos pequenos reversibles. | Mismos contratos y escenarios observables pasan. | Que cambiar pruebas de comportamiento para acomodar una nueva estructura sea refactor. |
| Caracterizacion | Prueba sobre comportamiento existente, normalmente verde al introducirse. | Resultado ejecutado y etiquetado como caracterizacion; protege antes de tocar codigo legado. | Que fue RED/GREEN historico. |
| Optimizacion | Caracterizacion y linea base primero; medicion repetible; cambio; misma medicion despues. | Resultados comparables, memoria cuando sea pertinente, y pruebas de correccion verdes. | Una mejora por complejidad teorica sin medicion, o una ganancia no medida. |
| Contrato, aceptacion o integracion | Oraculo externo/DTO/vertical aprobado; ejecutar en la frontera afectada. | Version, invariantes y resultado de extremo a extremo realmente ejecutados. | Que una prueba unitaria reemplaza la vertical solicitada. |
| Bloqueo de infraestructura | Diagnosticar sin cambiar semantica ni seguridad; registrar estado. | Comando, etapa alcanzada, error, alcance, alternativa segura intentada y prerrequisito para reintentar. | `passed`, `green` o `validated`. |

## Diseno de pruebas: independencia y utilidad

Las pruebas se organizan por comportamiento y frontera, no por el arbol de
clases. Una prueba puede cubrir varios tipos internos si usa una API de dominio
estable; esa es preferible a una coleccion de tests fragiles por metodo privado.

Reglas practicas:

- Pruebas de motor: preferir comandos publicos, tiempo virtual, estado
  consultable y trazas observables. Se permite una sonda interna o
  `#[cfg(test)]` cuando un limite de seguridad no es alcanzable de forma
  razonable por una API publica. Debe ser minima, documentada y no convertirse
  en contrato publico; ADR-0001 ya establece este precedente para E1.
- Pruebas de contrato: validar forma, version, discriminantes, referencias y
  representacion decimal. No parsear `ScenarioCanonicalReport.details` para
  alimentar la UI; ADR-0002 no es API de presentacion.
- Pruebas Vue: comprobar el DOM, entradas/salidas del composable o ViewModel y
  estado visible. No atar todas las pruebas a la jerarquia exacta de
  componentes. Filtros y navegacion deben ser locales e inmutables: nunca
  reconstruyen ARP/CAM ni invocan el motor.
- Dobles: usar solo para aislar una frontera real, por ejemplo IPC Tauri. Un
  doble no valida que el host, Cargo, Vue y Rust se comuniquen; para eso existe
  una prueba de integracion o vertical.
- Aserciones: verificar valores, transiciones, ausencia de efectos y orden
  cuando sean requisitos. Evitar presencia vaga (`contains`) si la norma exige
  una cantidad, motivo, tiempo o PDU exactos.

Ejemplos:

- A2: una prueba de quiescencia debe comprobar `Quiescent` y `8.700 ns`, no
  solamente que el heap conserve o no un timer fisico.
- ADR-0004: el defecto C# se protege con exactamente un `PacketDropped` y su
  motivo, ademas de la ausencia de `FrameTransmitted`; no basta con que la
  aplicacion no lance una excepcion.
- `presentation/v1`: una prueba valida enlaces contra interfaces reales y
  conserva cadenas decimales con `BigInt`; no acepta un numero JavaScript que
  pierda precision.

## Especificaciones, fixtures y golden tests

Los documentos normativos y goldens son oraculos protegidos.

1. No cambiar una especificacion, ADR, fixture golden o expected JSON solo para
   volver verde la suite.
2. Si se descubre que la expectativa contradice la norma, documentar el hecho,
   corregir primero la fuente de verdad mediante el proceso de decision
   aplicable y solo despues actualizar la prueba y el golden con una explicacion.
3. Los fixtures generados desde motor deben tener una comprobacion que vuelve a
   generarlos y compara el resultado, como `Export-PresentationFixture.ps1
   -Mode Check`. La regeneracion deliberada (`Write`) exige revision del diff.
4. Cuando Rust y C# deben ser independientes, comparar comportamiento y
   contratos, nunca compartir una implementacion de protocolo para hacer pasar
   el test.
5. Una diferencia permitida por A3 se documenta como diagnostica; no se fuerza
   igualdad de eventos internos para obtener un golden artificial.

## Bordes, invariantes, propiedades y determinismo

No se persigue cobertura porcentual como sustituto de razonamiento. Para cada
cambio, elegir los bordes e invariantes que puedan romperlo:

- **Bordes:** cero, uno, capacidad, justo antes/en/despues de un timestamp,
  limites de `u64`, cola llena, traza truncada y enlace desconectado/reconectado.
- **Invariantes:** orden `(at, sequence)`, monotonia del reloj, identificadores
  de traza, consistencia de retencion, referencias topologicas validas y que
  una ejecucion cancelada no extrae otro evento.
- **Propiedades:** usarlas para funciones puras o espacios de entrada amplios
  donde expresen mejor una ley que ejemplos aislados. Deben ser deterministas:
  semilla registrada y caso minimo reproducible ante fallo.
- **Determinismo:** ejecutar repetidamente escenarios relevantes si cambia el
  scheduler, serializacion, ordenacion, colecciones o exportadores. Comparar
  salida normalizada o hash, no tiempo de pared. E1 ya usa 100 ejecuciones
  independientes para informes canonicos; no hace falta repetirlas para un
  cambio de CSS aislado.

## Pruebas por nivel y momento de ejecucion

| Cambio | Antes de integrar | Antes de declarar terminado |
| --- | --- | --- |
| Dominio/scheduler Rust o C# | Prueba unitaria estrecha en cada micro-ciclo. | Suite del spike; determinismo y escenarios/contratos afectados; comparacion cruzada si cambia lo observable. |
| DTO, serializador o exportador | Prueba de validador/mapper. | Contrato, golden generado desde motor y paridad Rust/C# cuando aplique. |
| Vue/composable/componente | Prueba unitaria o de componente con fixture valido. | Vitest, typecheck y build; si el runner esta bloqueado, el estado es bloqueado, no verde. |
| Bridge/IPC Tauri | Unitarias de orden, version y errores con doble. | Prueba de host y vertical real: ventana abierta, accion Vue, motor Rust invocado y snapshot validado. |
| Cambio visual puramente local | Prueba de componente cuando tiene logica observable. | Typecheck/build y verificacion visual acordada si la tarea la requiere. |
| Refactor interno | Pruebas de caracterizacion existentes antes de mover codigo. | Misma bateria relevante; contrato/aceptacion si cruza una frontera. |

Los comandos se eligen por paquete y por el cambio real. Habitualmente son
`cargo test` para Rust, `dotnet test <proyecto>` para C# y `npm test` junto con
typecheck/build para Vue. No se declara una prueba aprobada si el runner no
llego a ejecutar aserciones.

## Optimizacion, complejidad y benchmarks

Una optimizacion no es una funcionalidad nueva y no debe simularse como un RED
de rendimiento fragil. El procedimiento es:

1. Proteger el comportamiento con pruebas existentes o de caracterizacion.
2. Escribir una hipotesis de complejidad y candidatos, sin asumir que `O(n)` es
   perjudicial.
3. Medir una linea base repetible, con cargas que representen el riesgo,
   medicion de tiempo y memoria cuando importe.
4. Cambiar un solo cuello de botella demostrado.
5. Repetir exactamente la medicion y todas las pruebas de correccion.

La auditoria de `has_active_work()` es el patron: timers pasivos anteriores al
trabajo activo revelaron escaneos repetidos; se preservaron `(at, sequence)`,
quiescencia y timers obsoletos; se compararon mediciones antes/despues. La
politica no admite reclamar mejora en casos sin diferencia medida ni sustituir
un benchmark por notacion Big-O.

## Mutation testing: selectivo, no metrica de cobertura

Mutation testing se usa ocasionalmente para preguntar si las aserciones
detectarian cambios semanticos plausibles. No mide lineas ejecutadas y no
reemplaza pruebas de contrato, escenarios ni revision humana.

Se propone aplicarlo de forma dirigida cuando haya una zona estable y de alto
riesgo, por ejemplo:

- comparadores de `(at, sequence)` y limites del scheduler;
- validacion de invariantes de `presentation/v1`;
- expiracion por epoca ARP/CAM;
- parser/validador que protege `BigInt` y retencion.

Precondiciones: suite verde, alcance pequeno, mutantes documentados y tiempo
aceptable. Un mutante superviviente se evalua: puede revelar una prueba debil,
un requisito que no importa o un mutante equivalente. No se establece un
porcentaje objetivo ni se agregan aserciones inutiles para matar mutantes.

## Revision de limites arquitectonicos

Adoptamos la revision de limites de Martin como una pausa tecnica breve, no
como reunion obligatoria por reloj. Debe ocurrir al terminar una porcion
coherente de trabajo, al aproximarse a 60--90 minutos de cambios concentrados,
o antes si aparece una de estas senales:

- se duplica una regla de protocolo entre motor, exportador o Vue;
- un tipo de dominio empieza a depender de JSON, UI, Tauri, sistema de archivos
  o transporte;
- Rust y C# dejan de ser implementaciones independientes;
- un contrato de comparacion intenta convertirse en API de UI;
- un cambio exige modificar muchas pruebas por detalles estructurales;
- aparece una cola, cache, mapa o buffer sin limite, o un nuevo bucle en el
  camino caliente;
- la siguiente prueba requiere mucho codigo fuera del micro-ciclo.

Checklist de la pausa:

1. Confirmar que dominio, simulacion, exportacion, contrato, transporte e UI
   siguen separados segun ADRs vigentes.
2. Revisar complejidad, copias de PDU, retencion y limites de memoria en rutas
   nuevas o modificadas.
3. Buscar duplicacion y acoplamiento accidental de pruebas a privados.
4. Decidir si basta un refactor local protegido o si la frontera requiere ADR.
5. Anotar solo la decision que afecte arquitectura, alcance, contrato o riesgo;
   no producir un informe para cada pausa sin hallazgos.

## Estado de infraestructura y comunicacion honesta

Usar una de estas etiquetas en reportes y revisiones:

- **Passed:** el comando termino correctamente y ejecuta las aserciones
  pretendidas.
- **Failed - compilation:** no se compilo; puede ser RED de superficie, no
  evidencia de asercion funcional.
- **Failed - assertion:** se ejecuto y una expectativa no se cumplio; conserva
  el mensaje, valor esperado y observado.
- **Blocked - infrastructure:** el comando no alcanzo el codigo o las pruebas
  por permisos, proceso, dependencias, proxy, SDK u host.
- **Not run:** no se intento; no inferir resultado.

Para `spawn EPERM`, cache/proxy Cargo o un fallo de Tauri: registrar comando,
etapa alcanzada, mensaje relevante, si el bloqueo afecta solo al runner o a la
compilacion, y el prerequisito concreto para reintentar. Se pueden ejecutar
pruebas independientes que sigan disponibles, pero `vue-tsc`, un build Vite o
la prueba con un doble IPC no validan por si solos la vertical. No se desactiva
TLS, no se omite validacion de contrato ni se cambia de arquitectura solo por
un sandbox restringido.

## Criterio ligero de cumplimiento

Antes de finalizar un cambio, quien lo realiza puede responder estas preguntas
sin producir formularios nuevos:

1. Que comportamiento, defecto, frontera o mejora se cambio, y cual es su
   fuente de verdad?
2. Que tipo de trabajo de la taxonomia fue? Si fue funcional o defecto, cual
   fue el RED y cual el GREEN? Si fue caracterizacion o refactor, esta marcado
   como tal?
3. Cual es la prueba mas pequena que protege el resultado y que borde o
   invariante vale la pena agregar?
4. Que nivel adicional se vio afectado: contrato, integracion, aceptacion,
   determinismo o comparacion cruzada?
5. Que comandos se ejecutaron realmente, y cuales quedaron bloqueados o no se
   ejecutaron?
6. Hubo una senal de limite arquitectonico, complejidad, duplicacion o conflicto
   normativo?

Responder "no aplica" con una razon concreta es valido. La finalidad es hacer
visibles decisiones tecnicas, no elevar el numero de tests ni exigir una
bitacora artificial.

## Texto propuesto para `AGENTS.md` (no aplicado)

```md
## Politica TDD
- Para cambios de comportamiento, defectos, contratos, refactors u optimizaciones,
  seguir `docs/engineering/TDD-POLICY-DRAFT.md` (hasta su aprobacion; despues,
  la version aprobada). Clasificar con honestidad RED funcional, RED de
  compilacion, caracterizacion, refactor o bloqueo de infraestructura.
- La especificacion y los ADR aceptados prevalecen sobre una prueba o golden.
  Detener y pedir decision ante contradiccion normativa; no modificar el oraculo
  solo para volver verde la suite.
```

No hay una seccion equivalente hoy en `AGENTS.md`: solo exige pruebas antes de
optimizaciones, no inventar resultados y documentar ambiguedades. El enlace
anterior evita repetir esta politica completa en las instrucciones de agentes.
