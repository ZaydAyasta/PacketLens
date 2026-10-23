# SPEC-E2.1 — Candidata consolidada RC1

Estado: candidata para aprobación definitiva de la dirección del proyecto. No autoriza implementar producción.

Fecha: 2026-10-21. Esta candidata consolida las siete decisiones del responsable de PacketLens y sustituirá, solo tras su aprobación, al borrador de investigación para el alcance E2.1. No modifica las normas E1.

Origen conservado en solo lectura: `E:\rea-lab\packetlens-research\SPEC-E2.1-DRAFT.md`, revisión declarada 2026-10-21, SHA-256 `FD59F686886863BEE521FEE504D88670EEB62F09E8A81DF5BA82C3F3006EAF59`.

## 1. Alcance, precedencia y decisiones cerradas

E2.1 añade gateways de host, router IPv4, rutas conectadas/estáticas, CIDR/LPM, ARP por interfaz y errores ICMP a dos motores headless independientes. Incluye responder Echo Request unicast dirigido al router. No añade protocolos de transporte, DHCP, VLAN, NAT, enrutamiento dinámico, plugins ni interfaz gráfica. Los nombres de operaciones siguientes expresan semántica; cada lenguaje conserva sus convenciones y errores tipados.

Para E1 prevalecen `SPEC-E1-v1.0.md`, su addendum y ADR-0001 a ADR-0004. La aprobación de esta candidata habilitará las nuevas reglas solo para E2.1. La política de trabajo es `docs/engineering/TDD-POLICY.md`. La matriz vinculante de aceptación prevista está en `docs/reports/E2.1-ACCEPTANCE-READINESS.md`.

| Decisión recibida | Resolución en RC1 | Requisitos |
| --- | --- | --- |
| 1. Local antes de TTL y Echo al router | Recepción local sin decremento; servicio Echo unicast; respuesta es un datagrama nuevo. | RTR-01, RTR-04 |
| 2. Dos conectadas iguales | Rechazo atómico por clave normalizada, aunque una interfaz esté caída. | RT-03 |
| 3. Fixtures completas | Catálogo y ejecución completos de S1–S8 en §8. | S1–S8D, FIX-01 |
| 4. Inyección privada | Llegada interna de pruebas que conserva el procesamiento normal y la admisión L2. | TEST-01 |
| 5. ICMP y origen | Red inalcanzable sin ruta; host inalcanzable al agotarse ARP del siguiente salto on-link; TTL nuevo 64. Ver excepción material C1 debajo. | ICMP-01 a ICMP-05 |
| 6. `/31` punto a punto | Dos extremos utilizables; validación topológica; sin broadcast dirigido. | CIDR-03, CIDR-04 |
| 7. Claves y épocas | Clave activa duplicada rechazada; época nueva al reutilizar; timers anteriores inactivos. | PING-01 a PING-06 |
| Echo Reply fallido en receptor | El receptor no inicia sesión ping por responder; solo el originador tiene terminal y timeout. | HOST-03, PING-07, S7 |

### 1.1 Incompatibilidad material C1: IP origen de Echo Reply

Aplicar «IP de la interfaz de salida» a **todo** ICMP contradice el servicio Echo cuando el Request tiene como destino otra IP del router. La corrección mínima propuesta para aprobación es: **los errores ICMP usan la IP de salida; Echo Reply usa como IP origen la IP destino del Echo Request**. TTL inicial 64 en ambos. La MAC origen siempre es la de salida. Es una excepción expresa a la decisión 5; no se presenta como decisión ya aprobada. [RFC 1812 §4.3.3.6](https://www.rfc-editor.org/rfc/rfc1812.html#section-4.3.3.6), [RFC 1122 §3.2.2.6](https://www.rfc-editor.org/rfc/rfc1122.html#section-3.2.2.6).

### 1.2 Precisiones de cierre incluidas para aprobación

Estas precisiones concretan casos que el borrador no cerraba; no cambian silenciosamente las decisiones recibidas:

- C2: `session_epoch` es metadato interno del timer, nunca un campo ICMP. Invalida timers viejos, pero no permite reconocer un reply viejo con los mismos src/dst/id/seq reutilizados. No se promete esa garantía. Las fixtures drenan el tráfico antes de reutilizar claves; para pings solapados se usan identificadores distintos.
- C3: si el Echo Request de una sesión aún no ha sido transmitido y falla su emisión diferida con caché ARP caliente, el descarte de ADR-0004 cierra esa sesión con `PingFailed(EgressUnavailable)`. Evita una sesión activa sin timer ni trabajo futuro. Si ya hubo emisión, rige el timeout ICMP; no se termina el ping por descartes de otros nodos.
- C4: fracaso de ARP del router puede tardar tres segundos; el timeout ICMP del host es de dos segundos desde su transmisión. Se conservan ambos tiempos: el error tardío se emite si es elegible, pero no añade otro terminal al host cerrado.
- C5: las mutaciones de IP, prefijo, gateway, TTL configurado y rutas se realizan con el tráfico detenido; los escenarios de cambios en ejecución se limitan al estado administrativo y conexión/reconexión E1. No se añade reruteo de un paquete ya encolado por ARP.

## 2. Fronteras y preservación E1

**BASE-01.** Se conservan todas las constantes E1: propagación `1_000 ns`, procesamiento `100 ns`, originación externa `0 ns`, retry ARP `1_000_000_000 ns`, tres intentos ARP, caché ARP `60_000_000_000 ns`, CAM `300_000_000_000 ns`, guardas `1_000` eventos sin avance y `100_000` pendientes, capacidad de traza por defecto `10_000`. E2.1 añade `ICMP_ECHO_TIMEOUT_NS = 2_000_000_000` y TTL ICMP originado `64`.

**BASE-02.** Orden total `(at, sequence)` y semántica de `StepEvents`, `AdvanceUntil`, cuotas, cancelación y overflow siguen E1/ADR-0001. Se extiende la clasificación de trabajo con el nuevo timer ICMP vigente; no se cambia el comparador ni se introduce tiempo real. Los timers ARP/ICMP obsoletos y expiraciones pasivas no mantienen `RunUntilQuiescent` activo.

**BASE-03.** E1-S1–S6, goldens y fixtures E1 quedan protegidos: mismos observables, tiempos, estados ARP/CAM, retención y resultados. E1-S2 frío conserva `8_700 ns`, 27 eventos y `presentation/v1` exacto. El número de eventos internos es diagnóstico conforme A3: un hash que lo incluya no es por sí solo oráculo normativo. Cualquier diferencia exige investigar antes de tocar un golden.

**BASE-04.** Dominio E2.1 no depende de Vue, Tauri ni DTO de presentación. `presentation/v1`, sus exportadores E1 y `ScenarioCanonicalReport`/ADR-0002 permanecen sin extensiones E2. La futura `presentation/v2` requiere especificación/ADR propios. La comparación E2 usa los hechos de dominio definidos aquí, nunca interpreta textos pedagógicos ni la gramática E1 como API UI.

**TIME-01.** Se transmite tras `100 ns` desde una recepción que requiere reenviar/responder. Un ARP originado por comando externo sale a `t0`; un Request IPv4 retenido sale `100 ns` después de recibir ARP Reply. Cada tránsito de enlace suma `1_000 ns`. No se acumulan dos retardos de procesamiento por desencolar un mismo paquete. Los retries ARP salen al vencer su timer, conforme E1. Antes de toda emisión diferida se revalidan interfaz y enlace; reconexión usa el enlace vigente, y la trama en vuelo conserva la identidad generacional con que salió (ADR-0004).

## 3. CIDR y configuración de red

**CIDR-01.** El valor de longitud se encapsula y solo acepta enteros `0..32`; `33` o mayor se rechaza con `InvalidPrefixLength` sin mutación. Máscaras `/0=00000000`, `/24=FFFFFF00`, `/31=FFFFFFFE`, `/32=FFFFFFFF`, sin desplazamiento de 32 bits ni overflow.

**CIDR-02.** Prefijo normalizado = IP AND máscara, en orden de octetos de red. `/0` coincide con cualquier IP; `/32` solo con una IP. Normalizar es idempotente. `192.168.1.10/24` se normaliza a `192.168.1.0`.

**CIDR-03.** En interfaz `/31`, ambos extremos de la pareja son unicast utilizables; no existe broadcast dirigido. `/32` tampoco tiene broadcast de subred. Para `/0..30`, el broadcast de una subred configurada se calcula con sus bits host a uno; el broadcast limitado `255.255.255.255` conserva su clasificación global. No se adivina el broadcast de una red remota a partir de su apariencia decimal. [RFC 3021 §2.1–2.2](https://www.rfc-editor.org/rfc/rfc3021.html#section-2.1).

**CIDR-04.** La asignación **a interfaces** de `/31` requiere modo `PointToPoint` declarado: un enlace directo entre exactamente dos interfaces IPv4 con las direcciones distintas de la misma pareja `/31`, sin switch intermedio. Configurar antes de cablear se permite; antes de aceptar tráfico se valida la topología completa y se rechaza `InvalidPointToPointTopology` si no se cumple. No basta que un cable Ethernet conecte a un switch de dos puertos. La longitud `/31` en una ruta es notación CIDR y no requiere crear un enlace. [RFC 3021, ámbito punto a punto](https://www.rfc-editor.org/rfc/rfc3021.html#section-2).

**CFG-01.** En E2.1 cada host tiene una interfaz IPv4 seleccionada sin ambigüedad para sus pings, `default_gateway: Option<IPv4>` y `default_ping_ttl` en `1..255`, por defecto 64. Un gateway debe ser unicast utilizable on-link y distinto de la IP propia; no hay gateway recursivo. Un router tiene al menos dos interfaces configuradas, pudiendo tener interfaces de reserva sin enlace. Configuración inválida falla atómicamente sin traza de red. E1 conserva sus construcciones existentes.

**CFG-02.** Como precisión C5 propuesta, los cambios E2.1 de IP, prefijo, gateway, TTL configurado y rutas se rechazan con `EngineBusy` mientras exista tráfico o una sesión/resolución activa; no alteran estado, reloj ni traza. Con motor en reposo se validan atómicamente. Timers pasivos u obsoletos por sí solos no bloquean configuración. Cambios administrativos y de enlaces siguen permitidos conforme E1.

## 4. Rutas y ARP

**RT-01.** Cada ruta contiene prefijo normalizado/longitud, interfaz de salida, next hop opcional, métrica y origen conectado/estático. Las conectadas se derivan de interfaces configuradas, métrica 0, sin next hop. Se mantienen reservadas aunque estén inactivas.

**RT-02.** Dos estáticas con la misma clave `(prefijo normalizado, longitud)` se rechazan con `AmbiguousRouteConfig`, independientemente de métrica/next hop. Estática idéntica a conectada se rechaza con `PrefixOverlapsConnectedNetwork`; no se prohíbe todo solapamiento.

**RT-03.** Dos interfaces del mismo router con igual clave conectada se rechazan con `DuplicateConnectedPrefix`, incluso con una caída. El rechazo también rige al reconfigurar interfaces y con independencia del orden de creación de rutas. La operación no elimina la configuración previa válida.

**RT-04.** Interfaz de ruta debe existir; next hop estático es unicast utilizable de esa interfaz, distinto de una IP propia. Fuera de subred: `NextHopNotDirectlyConnected`; propia/broadcast/multicast: `InvalidNextHop`. `next_hop=None` indica envío on-link por la interfaz declarada; no hay resolución recursiva.

**RT-05.** LPM selecciona el mayor prefijo coincidente entre rutas operativas. `/28` estática prevalece sobre `/24` conectada y ésta sobre `/0` estática, sin importar métricas. Al prohibirse claves idénticas, nunca se resuelve un empate real por métrica ni por orden de inserción. Las métricas quedan informativas; no hay ECMP.

**RT-06.** Operativa = interfaz administrativamente activa y enlace conectado vigente. Una ruta inactiva se ignora, sin borrar su configuración. Subir/reconectar rehabilita la ruta. Si se pierde salida tras seleccionarla, se aplica TIME-01/ADR-0004; no se vuelve a elegir ruta para esa transmisión pendiente.

**ARP-01.** Caché, colas FIFO, resoluciones y épocas del router se identifican por `(router, interfaz, next_hop_ip)`. No se comparte una caché entre interfaces ni se reenvía ARP. Next hop L2 = `route.next_hop` si existe, o destino final si es on-link. Aprendizaje por ARP, renovación/expiración, tres intentos y descarte FIFO conservan E1. La renovación se produce por ARP válido, no por usar una entrada ni por recibir IPv4.

**ARP-02.** Un paquete reenviado queda encolado con el TTL ya decrementado; resolución ARP, reintento y desencolado no vuelven a decrementarlo. Se conserva contexto de recepción suficiente para aplicar supresión ICMP al agotar ARP. Cada expiración/mutación ARP de router identifica su interfaz en observaciones de dominio E2; las observaciones E1 no se alteran.

## 5. Recepción local y reenvío

**HOST-01.** Destino on-link: ARP del destino. Destino remoto: ARP solo del gateway, preservando `dst_ip` remoto. Si falta gateway: `PacketDropped(NoRouteToHost)` local inmediato. Si el paquete es Request de un `send_ping` aceptado, se emite además un único `PingFailed(DestinationNetworkUnreachable)`; no se transmite trama.

**HOST-02.** Host receptor responde Echo Request unicast para su IP con Echo Reply nuevo TTL 64, id/seq iguales, IP origen = IP destino del Request. Aplica su propia decisión on-link/gateway para la vuelta. Nunca reutiliza como ruta el simple hecho de conocer el MAC/IP del emisor del datagrama remoto.

**HOST-03.** Responder Echo Request no crea sesión ping, registro de inicio, timer ICMP ni terminal en el receptor. Si falla la vuelta por ausencia de gateway, emisión o ARP, hay únicamente los descartes de esa respuesta: cero `PingCompleted`/`PingFailed` atribuibles al receptor. El originador conserva su sesión hasta respuesta/error/timeout. Esta regla también se aplica al servicio Echo del router.

**RTR-01.** Después de la recepción L2 válida y antes del TTL de tránsito, se compara `dst_ip` contra las IP unicast configuradas del router. Una coincidencia es entrega local, aunque sea otra interfaz del router; no se decrementa ni descarta por TTL 0/1 y el paquete original no se reenvía. Echo Request local produce Echo Reply nuevo bajo HOST-02 y C1; se enruta la respuesta hacia el emisor. El envío de esta respuesta sí usa LPM/ARP, pues es un nuevo paquete. No se crea una sesión ping del router. El servicio Echo y TTL local siguen [RFC 1812 §4.2.2.9 y §5.2.1](https://www.rfc-editor.org/rfc/rfc1812.html#section-5.2.1).

**RTR-02.** Solo en tránsito unicast: si TTL <=1, un `PacketDropped(TtlExpiredInTransit)`, cero reenvíos del original, posible ICMP 11/0 según §6. Si TTL>1, se decrementa exactamente una vez antes de LPM; se conserva origen/destino IP. Sin ruta: `PacketDropped(NoRouteToHost)` y posible ICMP 3/0. Con ruta: trama Ethernet nueva con MAC origen de salida y MAC del next hop. Nunca se origina ni reenvía IPv4 con TTL=0.

**RTR-03.** Recepción de IPv4 en router conserva la clasificación L2 original. MAC unicast ajena se rechaza; L2 broadcast/multicast pasa por la admisión normal a la política restrictiva. E2.1 no reenvía ni responde Echo sobre difusión: L2 broadcast/multicast o destino IP broadcast/multicast se descartan con `UnsupportedIpDestination`; origen `0.0.0.0/8`, `127.0.0.0/8`, multicast, broadcast limitado o broadcast de una subred propia se descarta con `InvalidIpv4Source`. Cero errores ICMP en ambos casos. Estos controles preceden al servicio Echo y al reenvío. Son el subconjunto pedagógico explícito de validación; no se reclama validación de cabecera binaria.

**RTR-04.** La aceptación del servicio Echo se prueba con destino a la IP de entrada y a otra IP configurada del router, TTL de Request 0, 1 y 64. El Reply tiene TTL inicial 64, id/seq iguales, destino = origen del Request y origen según C1. El original no produce `TtlExpiredInTransit` ni Time Exceeded. No se añade un evento de entrega local solo para facilitar tests: Reply y PDU original observada permiten distinguirla.

## 6. ICMP y tratamiento de fallos

**ICMP-01.** Para cada descarte elegible del datagrama original se intenta exactamente un error, nunca dos: TTL tránsito -> Type 11 Code 0; ausencia de ruta -> Type 3 Code 0; agotamiento ARP del router hacia un next hop on-link -> Type 3 Code 1 (`DestinationHostUnreachable` al correlacionar ping). «Directamente alcanzable» describe el objetivo ARP, tanto destino conectado como next hop estático; no implica que haya respondido. Los hosts conservan `ArpTimeout` para sus propias resoluciones; no generan ICMP Host Unreachable por ello. [RFC 1812 §5.2.7.1](https://www.rfc-editor.org/rfc/rfc1812.html#section-5.2.7.1).

**ICMP-02.** El payload discriminado soporta Echo Request/Reply, Time Exceeded código 0 y Destination Unreachable códigos 0/1. Los errores incluyen `OriginalDatagramSummary { src_ip, dst_ip, protocol=1, echo_identifier, echo_sequence }`; campos Echo opcionales solo si el original era Echo. Es resumen semántico, no cabecera binaria ni bytes citados. El datagrama original sigue íntegro en la observación del descarte.

**ICMP-03.** Error originado: TTL 64 nuevo, `dst_ip=original.src_ip`, IP origen de la interfaz seleccionada para el retorno y MAC de dicha salida. El error se enruta por LPM y ARP normales, sin decremento por originación en ese mismo router. Para Echo Reply rige C1. [RFC 1812 §4.3.2.2–§4.3.2.4](https://www.rfc-editor.org/rfc/rfc1812.html#section-4.3.2.2).

**ICMP-04.** Suprimir error si el original ya es TimeExceeded/DestinationUnreachable, fue L2 broadcast/multicast, tiene destino IP broadcast/multicast u origen inválido RTR-03. La clasificación L2 persiste incluso tras esperar ARP. Sin ruta de retorno: queda solo el descarte original; no se construye/emite otro datagrama ni otro descarte del error. Si hay ruta pero falla ARP/egreso del error, se registra su descarte ordinario y jamás genera error secundario. [RFC 1812 §4.3.2.7](https://www.rfc-editor.org/rfc/rfc1812.html#section-4.3.2.7).

**ICMP-05.** En fallo ARP del router se elimina la cola original en FIFO y se registra `ArpResolutionFailed` por paquete; se considera como máximo un Host Unreachable por cada original elegible. No se emite error al primer miss de caché. Se conserva el contexto original para evitar cascadas al fallar ARP de un propio error. El timer del host puede haber expirado antes (C4); recibir el error después no reabre la sesión.

Fuera de alcance: rate limiting, Source Quench, Redirect, Parameter Problem, opciones, fragmentación, checksums, FCS y serialización binaria. La exclusión de rate limiting es una decisión de alcance, no un requisito del determinismo: un limitador basado en tiempo virtual podría ser determinista.

## 7. Sesión ping, tiempos y observabilidad

**PING-01.** Solo un comando `send_ping` aceptado crea una sesión. Clave = `(host, target_ip, identifier, sequence)`. Activa en `Created/AwaitingResolution/AwaitingReply`; duplicado activo -> `DuplicatePing`, sin mutar reloj, cola, traza ni la primera sesión. Tras terminar puede reutilizarse la clave con época nueva monotónica sin wraparound. No se presupone época en paquetes (C2).

**PING-02.** Al emitirse realmente el Echo Request local al medio, en `t_tx`, se pasa a `AwaitingReply` y se agenda el timeout para `t_tx + 2_000_000_000 ns` con clave y `session_epoch`. No se agenda al crear el comando ni mientras espera ARP. El cálculo usa suma comprobada; si no es representable, `TimeOverflow` se devuelve antes de la emisión parcial. RTT de `PingCompleted` sigue contando desde la aceptación del comando, como E1; es distinto del origen del timeout.

**PING-03.** Reply válido: destino local correcto, IP origen = target, id/seq coincidentes y sesión en AwaitingReply. Error válido: destino local y resumen `src_ip` local, `dst_ip` target, protocolo ICMP, id/seq coincidentes de un Echo Request enviado. TimeExceeded -> `TtlExpired`; 3/0 -> `DestinationNetworkUnreachable`; 3/1 -> `DestinationHostUnreachable`. No se exige que la IP exterior del error sea el target. Paquetes no coincidentes o recibidos sin sesión activa no generan terminal.

**PING-04.** Toda sesión aceptada emite exactamente un terminal por época bajo estos fallos modelados: éxito, ausencia local de ruta, fallo ARP local, fallo de egreso antes de transmitir (C3), error recibido o timeout ICMP. Cerrar sesión invalida su timer. Duplicados/paquetes tardíos tras terminal no la reabren. Cancelación, cuota o guarda son pausas/resultados del motor E1: no fabrican terminales de red.

**PING-05.** Timeout solo actúa si la sesión continúa AwaitingReply y coinciden clave/época: emite `PingFailed(IcmpEchoTimeout)` y cierra. Timer viejo -> no-op sin traza; no mantiene quiescencia. La comprobación usa la búsqueda del registro de sesión y comparación de época: no obliga a un contenedor O(1) ni cambia los límites del scheduler por una promesa algorítmica no medida.

**PING-06.** Reply/error/timeout con el mismo `at` se resuelven por su `sequence` real, sin prioridad artificial. El primer evento aplicable cierra la sesión; los siguientes no generan otro terminal. Se prueban ambos órdenes. C2 limita la promesa de aislamiento frente a paquetes de una clave reutilizada.

**PING-07.** Host receptor con Reply fallido nunca figura como iniciador de sesión ping. Verificar en S7 tanto cero terminales del receptor como ausencia de su timer ICMP, y mantener también la prueba con fallo ARP del Reply (además de falta de gateway).

**OBS-01.** Hechos aceptados: `FrameTransmitted`, `FrameDelivered`, `PacketDropped`, `ArpPacketQueued`, `TableUpdated`, `PingCompleted`, `PingFailed` con tiempos/secuencias de traza y PDU semánticas completas. Se usa `FrameTransmitted`, no el nombre ficticio `FrameEmitted` del borrador. Cambios ARP de router llevan interfaz; no se emiten decisiones OSI, causalidad inferida o campos binarios. Retención conserva las invariantes E1; las pruebas de cardinalidad emplean capacidad suficiente o recolección incremental sin pérdida.

**TEST-01.** El arnés privado de pruebas agenda una llegada Ethernet semántica a una interfaz y tiempo válidos. Reutiliza comprobación de interfaz, admisión L2 y recepción ARP/IPv4 de producción; no llama directamente al generador ICMP ni fabrica traza. Requiere enlace vigente y usa su identidad generacional; registra la llegada normal, pero no inventa una transmisión anterior. Rechaza pasado/overflow como ADR-0001. No se expone en comandos públicos ni build ordinario. En Rust puede estar en pruebas del crate; en C# en una sonda `internal` para tests. Los nombres de tests de la matriz no obligan a habilitar hooks desde tests externos Rust.

**TEST-02.** Presembrado ARP solo en fixtures que lo declaren, mediante constructor/helper interno de test con la misma entrada, expiración y época que el estado normal. Se realiza antes del estímulo a `t=0`, sin trazas de aprendizaje ficticias ni tráfico de calentamiento oculto; expira a `60_000_000_000 ns`. Después del estímulo no se modifica una tabla para forzar el resultado.

## 8. Fixtures y escenarios reproducibles

F1–F8 designan las fixtures de E2.1-S1 a E2.1-S8 respectivamente, tanto en este documento como en la matriz de aceptación.

**FIX-01.** Cada escenario crea un motor nuevo con reloj 0, traza de capacidad 10.000, enlaces de propagación 1.000 ns y las interfaces en orden de las tablas. Se crean dispositivos/puertos en el orden escrito, enlaces en el orden de la lista y rutas en el orden indicado. Todos los puertos enlazados están administrativamente activos salvo override. Cachés ARP/CAM vacías, sin timers, salvo TEST-02 explícito. TTL del host 64 salvo S2. Toda ejecución completa usa `RunUntilQuiescent(1000)` y debe terminar Quiescent, nunca por cuota. Las variantes usan instancias separadas. No se exige igualdad del número interno de eventos.

Convención exhaustiva de MAC: dispositivo número `d`, interfaz número `p` -> `02:00:00:00:DD:PP`, con DD/PP octetos hex de esos números decimales. Puertos numerados desde 1: host eth0=1; router eth0=1, eth1=2, etc.; switch port1=1, port2=2. IDs de dispositivo se definen en cada tabla. Ejemplo d=2,p=1 -> `02:00:00:00:02:01`. En S1 se usan las MAC originales explícitas en lugar de esta convención. Switches no tienen IPv4 ni gateway. Ningún puerto no listado existe.

### E2.1-S1 — Ida y vuelta inter-subred en frío

| Dispositivo (d) | Interfaces IPv4 / MAC | Gateway |
| --- | --- | --- |
| host1 (1) | eth0 `192.168.1.10/24`, `00:00:00:00:01:01` | `192.168.1.1` |
| sw1 (2) | port1, port2; MAC por convención | — |
| r1 (3) | eth0 `192.168.1.1/24`, `00:00:00:00:01:fe`; eth1 `10.0.0.1/24`, `00:00:00:00:02:fe` | — |
| sw2 (4) | port1, port2; MAC por convención | — |
| host2 (5) | eth0 `10.0.0.20/24`, `00:00:00:00:02:02` | `10.0.0.1` |

Enlaces: l1 host1:eth0–sw1:port1; l2 sw1:port2–r1:eth0; l3 r1:eth1–sw2:port1; l4 sw2:port2–host2:eth0. Sin estáticas. Ping de host1 a `10.0.0.20`, id=1, seq=1, t=0; ejecutar completo.

**S1.** ARP final exacto: host1 `{192.168.1.1 -> r1:eth0}`; r1:eth0 `{192.168.1.10 -> host1}`; r1:eth1 `{10.0.0.20 -> host2}`; host2 `{10.0.0.1 -> r1:eth1}`. Host1 nunca pregunta/aprende la MAC de host2 ni viceversa. Request: IP origen/destino `192.168.1.10 -> 10.0.0.20`, TTL 64 en A y 63 en B, MAC host1->r1:eth0 y r1:eth1->host2. Reply: IP invertidas, TTL 64 en B y 63 en A, MAC host2->r1:eth1 y r1:eth0->host1. Un Completed host1, cero Failed y cero terminales en otros nodos. CAM de cada switch aprende solo las dos MAC del segmento en sus puertos correspondientes.

### E2.1-S2 — Bucle con TTL=4 y retorno

| Dispositivo (d) | Interfaces IPv4 | Gateway/TTL |
| --- | --- | --- |
| host1 (1) | eth0 `192.168.1.10/24` | `192.168.1.1`, TTL 4 |
| r1 (2) | eth0 `192.168.1.1/24`; eth1 `172.16.0.1/30` | — |
| r2 (3) | eth0 `172.16.0.2/30`; eth1 `203.0.113.1/24` sin enlace, admin down | — |

MAC por convención. Enlaces: l1 host1:eth0–r1:eth0; l2 r1:eth1–r2:eth0. Estáticas: r1 `10.0.0.0/24 via 172.16.0.2 eth1 metric10`; r2 `10.0.0.0/24 via 172.16.0.1 eth0 metric10`, luego `192.168.1.0/24 via 172.16.0.1 eth0 metric10`. Ping destino exacto `10.0.0.20`, id=1,seq=1,t=0; ejecutar completo.

**S2.** TTL Request observado: host1 4, r1 3, r2 2, r1 1; r2 descarta la siguiente llegada con TTL 1. Cero TTL 0 transmitido. Un único TimeExceeded 11/0 originado por r2, src=`172.16.0.2`, dst=`192.168.1.10`, TTL inicial 64 y TTL 63 tras r1, resumen del ping. Un Failed host1/TtlExpired; ningún Completed. La retransmisión por la misma interfaz r2:eth0 no decrementa dos veces ni requiere ICMP Redirect.

### E2.1-S3 — Host sin gateway

host1 (1), eth0 `192.168.1.10/24`, MAC por convención, admin up, sin enlace, gateway=None. Ping a `172.16.5.99`, id=1,seq=1,t=0; ejecutar completo.

**S3.** Drop local NoRouteToHost a t=0 seguido de un Failed/DestinationNetworkUnreachable; cero tramas, cero timer ICMP, reloj final 0 y Quiescent.

### E2.1-S4 — Router sin ruta

host1 (1) eth0 `192.168.1.10/24`, gateway `192.168.1.1`; r1 (2) eth0 `192.168.1.1/24`, eth1 `203.0.113.1/24` sin enlace/admin down. MAC por convención. l1 host1:eth0–r1:eth0. Sin estáticas, ARP frío. Ping a `198.51.100.1`, id=1,seq=1,t=0; ejecutar completo.

**S4.** Drop r1 NoRouteToHost con TTL 63; un ICMP 3/0 src=`192.168.1.1`, dst=host1, TTL 64 por eth0 y resumen coincidente; un Failed host1/DestinationNetworkUnreachable. Conectada de eth0 garantiza retorno.

### E2.1-S5 — Tres prefijos LPM

| Dispositivo (d) | Interfaces IPv4 | Gateway |
| --- | --- | --- |
| host1 (1) | eth0 `192.168.1.10/24` | `192.168.1.1` |
| r1 (2) | eth0 `192.168.1.1/24`; eth1 `172.16.1.1/24`; eth2 `172.16.2.1/24`; eth3 `172.16.3.1/24` | — |
| r_gw1 (3) | eth0 `172.16.1.2/24`; eth1 `203.0.113.1/30` sin enlace/admin down | — |
| r_gw2 (4) | eth0 `172.16.2.2/24`; eth1 `203.0.113.5/30` sin enlace/admin down | — |
| r_gw3 (5) | eth0 `172.16.3.2/24`; eth1 `203.0.113.9/30` sin enlace/admin down | — |

Enlaces: l1 host1:eth0–r1:eth0; l2 r1:eth1–r_gw1:eth0; l3 r1:eth2–r_gw2:eth0; l4 r1:eth3–r_gw3:eth0. MAC por convención. R1 estáticas en orden: `10.0.0.0/8 via 172.16.1.2 eth1 metric10`, `10.1.0.0/16 via 172.16.2.2 eth2 metric10`, `10.1.1.0/24 via 172.16.3.2 eth3 metric10`. Cada r_gwN añade `192.168.1.0/24 via 172.16.N.1 eth0 metric10` para el retorno; no tiene ruta/default al destino.

Presembrado TEST-02 bidireccional para cada pareja de l1–l4; todo lo demás vacío. Ping host1 a `10.1.1.50`, id=1,seq=1,t=0; ejecutar completo.

**S5.** Request de R1 sale solo por eth3 a r_gw3 (TTL 63); cero tramas de R1 por eth1/eth2. r_gw3 descarta sin ruta y devuelve 3/0, que R1 reenvía por eth0; un Failed de host1 cierra el flujo. El terminal esperado de este escenario es deliberadamente NetworkUnreachable, no éxito. La aceptación LPM distingue Request y error de retorno.

### E2.1-S6 — Ruta inactiva y respaldo

| Dispositivo (d) | Interfaces IPv4 | Gateway |
| --- | --- | --- |
| host1 (1) | eth0 `192.168.1.10/24` | `192.168.1.1` |
| r1 (2) | eth0 `192.168.1.1/24`; eth1 `10.0.1.1/24`; eth2 `10.0.2.1/24` | — |
| gw_primario (3) | eth0 `10.0.1.2/24`; eth1 `203.0.113.1/30` sin enlace/admin down | — |
| gw_respaldo (4) | eth0 `10.0.2.2/24`; eth1 `203.0.113.5/30` sin enlace/admin down | — |

MAC por convención. l1 host1:eth0–r1:eth0; l2 r1:eth1–gw_primario:eth0; l3 r1:eth2–gw_respaldo:eth0. R1 estáticas: `10.20.0.0/16 via 10.0.1.2 eth1 metric5`; `0.0.0.0/0 via 10.0.2.2 eth2 metric20`. Cada gateway tiene retorno `192.168.1.0/24 via 10.0.N.1 eth0 metric10`, con N=1/2; ninguno tiene ruta/default hacia `10.20.1.5`. Presembrado TEST-02 bidireccional en l1–l3.

**S6.** Dos instancias independientes, ping host1 a `10.20.1.5`, id=1,seq=1,t=0: A) r1:eth1 admin down antes del ping: Request solo por eth2, respuesta 3/0 desde gw_respaldo y Failed host1. B) eth1 y eth2 admin down: r1 descarta sin ruta, devuelve 3/0 por eth0, Failed host1. No se afirma conectividad completa a partir de la mera selección de respaldo. Variante de RT-06 sustituye admin down por desconectar enlace y luego reconecta para comprobar restauración.

### E2.1-S7 — Echo Reply sin ruta y timeout

host1 (1) eth0 `192.168.1.10/24`, gateway=`192.168.1.1`; r1 (2) eth0 `192.168.1.1/24`, eth1 `10.0.0.1/24`; host2 (3) eth0 `10.0.0.20/24`, gateway=None. MAC por convención. l1 host1:eth0–r1:eth0; l2 r1:eth1–host2:eth0. Sin estáticas, frío. Ping host1 a host2, id=1,seq=1,t=0; ejecutar completo.

**S7.** Echo Request llega a host2; el intento de Reply se descarta localmente por NoRouteToHost. Host2 tiene cero sesiones y cero terminales. Host1 termina exactamente una vez IcmpEchoTimeout en `t_tx + 2_000_000_000`, donde t_tx es su transmisión real Request observada. Final Quiescent en ese instante; los retries ARP resueltos no lo prolongan. Variante HOST-03 usa otra instancia con gateway de host2=`10.0.0.99`, unicast on-link no asignado a dispositivo alguno. Host2 intenta resolverlo para el Reply, agota tres intentos ARP y lo descarta, sin crear sesión ni terminal de ping propio. No se alteran paquetes ni respuestas de la simulación para provocar este fallo.

### E2.1-S8 — Matriz de supresión controlada

Topología/MAC = S4. Presembrar únicamente r1:eth0 `192.168.1.10 -> MAC host1` según TEST-02. No emitir comando ping ni configurar sesión. Cada caso usa una instancia nueva; TEST-01 inyecta llegada a r1:eth0 a t=0 con MAC origen host1 y MAC destino r1:eth0 salvo override. TTL 64, src=`192.168.1.10`, dst=`198.51.100.1`, EchoRequest id7/seq1, protocolo1. Ejecutar completo. No contar la llegada inyectada como transmisión ni error generado.

| Requisito / caso | Override del datagrama base | Resultado |
| --- | --- | --- |
| S8A — elegible | Ninguno | Un drop NoRouteToHost y exactamente una transmisión de ICMP 3/0 desde r1:eth0; host1 recibe el error, cero terminales porque no inició ping. |
| S8B — error previo | Payload TimeExceeded(0) o DestinationUnreachable(0/1); resumen base válido | Un drop NoRouteToHost; cero errores nuevos. Se cuentan solo transmisiones del router, no el payload inyectado. |
| S8C — difusión | Casos separados: dst `255.255.255.255`; dst `192.168.1.255`; dst `224.0.0.1`; o MAC destino broadcast/multicast con dst base | Un drop UnsupportedIpDestination; cero IPv4 reenviado, ARP originado o error ICMP. El broadcast de subred tiene ruta conectada: la causa no es «falta de ruta». |
| S8D — origen inválido | src `0.0.0.0`, `0.1.2.3`, `127.0.0.1`, `224.0.0.1`, `255.255.255.255` o `192.168.1.255` | Un drop InvalidIpv4Source; cero error ICMP. |

Para S8C, MAC multicast exacta `01:00:5e:00:00:01`, broadcast `ff:ff:ff:ff:ff:ff`. Todas las demás MAC siguen la regla de S4. Los casos de error por TTL repiten S8A/B con TTL1: A -> un 11/0; B -> solo TtlExpiredInTransit. No se requiere implementar difusión ni multicast para suprimir errores.

### 8.1 Variantes de borde vinculadas a los requisitos

Estas fixtures derivadas completan las pruebas unitarias/de aceptación sin crear escenarios S9 ni modificar los ocho escenarios:

- CIDR-04: host1 (d=1) eth0=`172.16.0.0/31` y host2 (d=2) eth0=`172.16.0.1/31`, MAC por convención, gateway=None, ambos admin up, l1 directo declarado PointToPoint. Dos instancias frías: ping host1->host2 id1/seq1 y ping host2->host1 id2/seq1. Ambos deben completar; ni un extremo se clasifica como broadcast. Las variantes inválidas insertan un switch, repiten IP o cambian el prefijo de un extremo.
- ICMP-01/05, destino conectado sin respuesta ARP: F7 sustituye host2 por un host silencioso (mismo d=3/MAC), eth0=`10.0.0.30/24`, gateway=None; target sigue `10.0.0.20`. Conservar l2. Nadie responde ARP de `.20`. Host1 inicia id1/seq1 en t0 y se ejecuta completo: router agota tres intentos y emite 3/1 después del único timeout ICMP de host1.
- ICMP-01/05, next hop estático sin respuesta ARP: F6 sin semillas en l2, r1:eth1 activa, gw_primario:eth0=`10.0.1.3/24`. Ruta de r1 conserva next hop `.2` no asignado; gateway conserva retorno vía `.1`. Demás parámetros intactos. No se usa respaldo mientras la primaria sea operativa; se emite 3/1 al agotar ARP.
- ICMP-05, FIFO: repetir cualquiera de las dos anteriores con comandos id1/seq1 e id1/seq2 en t0 y ese orden antes de avanzar; una resolución compartida, dos originales FIFO, un terminal por sesión.
- ICMP-04, error sin retorno: F8 inyecta src=`198.18.0.10`, válido pero sin ruta/default de r1 hacia él. Solo descarte original.
- ICMP-04, ARP del propio error: F8 sin semilla, src=`192.168.1.99` no asignado; MAC origen sigue host1. IPv4 no crea ARP. Hay ruta conectada de retorno pero nadie responde ARP de `.99`; descarte del error sin cascada.
- PING-04/C3, primer egreso imposible: F5 con pareja host1/gateway presembrada; antes de la primera transmisión programada invalidar la salida con el control por pasos o sonda interna. Descarte ADR-0004 y un Failed/EgressUnavailable; ningún timer ICMP activo. La sonda puede detenerse en el punto previo a emitir, pero no cambia la prioridad del scheduler.
- RTR-01/04: F4 conserva ambas IP propias de r1; variar destino entre `192.168.1.1` y `203.0.113.1`, entrando siempre por eth0, con TTL 0/1/64. Para TTL0 se inyecta llegada; para TTL1/64 se usa ping normal o la misma llegada controlada. Probar ambos destinos verifica C1 con una IP origen diferente de la IP de salida.

## 9. Aceptación, límites y cierre

**DET-01.** Repetir 100 veces cada caso por motor: misma traza semántica ordenada, tiempo/outcome y estados finales. Comparar Rust/C# por los mismos hechos, excluyendo métricas internas y orden de claves JSON; no se comparte implementación ni generador de protocolos. No se congela un nuevo contrato serializado para lograr esta comparación.

Todas las reglas con ID tienen correspondencia Rust/C# en la matriz. Ninguna prueba de esta candidata existe por el hecho de estar nombrada. Al comenzar implementación se obtiene baseline E1 ejecutada, luego un RED funcional mínimo de S1, GREEN y refactor proporcional. No se utiliza un fallo de compilación o de infraestructura como aceptación funcional. Los nuevos goldens solo se fijarán después de confirmar resultados contra esta candidata aprobada y los dos motores; los de E1 no se regeneran para acomodar E2.

Bloqueo residual de aprobación: aceptar C1 y las precisiones C2–C5 junto con esta candidata. No queda una fixture a inventar ni se requiere diseñar `presentation/v2` para iniciar S1. Esta entrega no verifica ejecutando motores: todos los resultados E2 de este documento son expectativas propuestas, no mediciones.
