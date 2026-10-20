# Plan de siguiente fase: contrato de lectura e integracion vertical E1-S2

Estado: plan pendiente de aprobacion. No autoriza todavia integracion real,
nuevos protocolos, GUI definitiva, Tauri, MCP ni cambios al PoC externo.

## Objetivo

Validar una frontera entre un motor E1 headless y un inspector Vue sin acoplar
el dominio a la presentacion. La primera vertical consume exclusivamente la
parte fria de E1-S2 y mantiene Rust y C# como implementaciones independientes.

## Incrementos TDD

| Incremento | RED verificable | GREEN minimo | Refactor permitido | Criterio de aceptacion |
| --- | --- | --- | --- | --- |
| 0. Decision y fixture | Tests de contrato inexistentes. | Aprobar motor candidato, transporte y esquema `presentation/v1`. | Ninguno. | Decisiones registradas; no se modifica SPEC-E1 ni ADR-0002. |
| 1. Esquema tipado | Test no compila/falla al no existir el DTO. | Tipos de contrato fuera del dominio E1. | Extraer serializacion sin tocar scheduler. | JSON versionado, enteros grandes como cadenas, sin `details` parseado. |
| 2. Exportador Rust | Golden test falla por snapshot ausente. | Adaptador Rust de E1-S2 frio. | Separar mapper de serializador. | 8.700 ns, `Quiescent`, 27 eventos ordenados y PDU semanticas conservadas. |
| 3. Exportador C# | Mismo golden test falla contra exportador C#. | Adaptador C# independiente. | Reusar solo el contrato, nunca codigo del motor Rust. | Proyeccion semantica igual a Rust; diferencias de diagnostico solo si A3 las permite y quedan documentadas. |
| 4. Retencion, tablas y causalidad | Tests de borde faltan. | Metadatos de retencion y deltas ARP/CAM; causalidad nula cuando no se emite. | Extraer consultas de lectura del motor si son necesarias. | El consumidor detecta truncamiento; no se inventa parentage. |
| 5. Consumidor Vue aislado | Vitest falla con fixture nuevo. | Composable/mapper en el repo principal que consume el DTO. | UI conserva filtros/cursor puros. | Filtros no mutan eventos, seleccion por `trace_seq`, tiempo y PDU exactos. |
| 6. Bridge de host | Test de extremo a extremo falla. | Un solo transporte previamente aprobado. | Manejo de errores/versionado sin filtrar tipos internos. | Solicitud E1-S2 frio devuelve snapshot validado de punta a punta. |

Cada incremento sigue RED -> GREEN -> REFACTOR y conserva una bitacora que
separe pruebas que no compilan de fallos de ejecucion. Ninguna optimizacion se
acepta sin pruebas existentes y una medicion reproducible.

## Pruebas obligatorias

1. **Contrato estructural:** valida esquema, version, nombres de discriminantes
   y representacion decimal de enteros.
2. **Proyeccion de traza:** conserva `TraceSeq`, `at_ns`, kind, endpoint y orden
   de cada uno de los 27 eventos E1-S2 frios.
3. **PDU:** compara semanticamente Ethernet/ARP/IPv4/ICMP; prohibe completar
   checksum, longitud, FCS o bytes que no existen en E1.
4. **Tablas:** verifica los deltas de aprendizaje/expiracion de ARP y CAM y su
   timestamp. Si se pide snapshot final, prueba tambien su coherencia con los
   deltas no truncados.
5. **Retencion:** usa una capacidad pequena y verifica advertencia de historial
   incompleto, no una reconstruccion falsa.
6. **Determinismo:** exporta E1-S2 frio 100 veces por motor y compara hash de
   la proyeccion normalizada.
7. **Compatibilidad dual:** ejecuta las mismas acciones publicas en Rust y C#;
   compara los campos normativos y registra explicitamente cualquier diferencia
   permitida por A3.
8. **Vue:** ejecuta la suite con fixture de contrato. El control Anterior solo
   mueve el cursor visual; no invoca comandos del motor.
9. **Vertical:** inicia el host elegido, solicita la corrida y valida el DTO
   recibido contra el fixture. No requiere una interfaz final.

## Criterios de salida de la fase

- El contrato de presentacion queda versionado y separado del reporte canonico.
- Rust y C# producen la misma proyeccion semantica para E1-S2 frio.
- La UI muestra la traza real retenida, incluidos PDU/deltas disponibles, sin
  analizar cadenas de `details`.
- La procedencia de todo dato ilustrativo es visible para el usuario.
- Las pruebas de motores, contrato, consumidor y vertical se ejecutan y sus
  resultados se anotan, incluidos fallos de infraestructura.
- No se agrega protocolo fuera de Ethernet, ARP, IPv4, ICMP y switch L2 E1.

## Riesgos que se resolveran antes de ampliar alcance

| Riesgo | Mitigacion y prueba |
| --- | --- |
| Precision JavaScript | Cadenas decimales en transporte; parser/formatter probado. |
| Confundir comparacion con UI | Regla de no parsear `ScenarioCanonicalReport.details`; linter/revision de contrato. |
| Trazas truncadas | Metadatos obligatorios y estado visible de historial incompleto. |
| Causalidad falsa | `caused_by = null` por defecto; solo emitir relaciones instrumentadas. |
| Duplicar reglas de red en UI | Mappers consumen codigos semanticos; no recrean ARP/CAM. |
| Bloqueo del inspector PoC | Reejecutar Vitest en un entorno donde Vite pueda crear sus procesos. El `spawn EPERM` actual no se interpreta como prueba verde. |
| Seleccion de host prematura | No construir bridge hasta aprobar motor y alternativa de transporte. |

## Aprobaciones requeridas

1. **Motor provisional:** aprobar Rust como candidato primario, manteniendo C# como referencia independiente, o indicar otra ponderacion de criterios.
2. **Contrato:** aprobar `SimulationPresentationSnapshot v1`, separado de ADR-0002, incluyendo cadenas decimales y ausencia de causalidad no instrumentada.
3. **Transporte:** escoger la direccion de la primera vertical: bridge en proceso con host Vue/Rust, sidecar JSON-RPC neutral, o host .NET/WebView. La recomendacion actual es bridge Rust solo si tambien se aprueba ese host; de lo contrario sidecar de prueba.
4. **Granularidad del inspector:** aprobar que la primera UI consuma los 27 eventos crudos de E1-S2 frio. Los 10 hitos actuales del PoC seran una proyeccion pedagogica posterior, no un reemplazo de la traza.
5. **Investigacion externa:** confirmar que sus hallazgos se usen solo como referencia de UX/observacion y que SPEC-E1/RFCs abiertos sigan siendo la norma, sin copiar codigo, recursos ni textos propietarios.
