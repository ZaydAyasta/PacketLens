# Especificación Normativa Ejecutable — Experimento E1 (v1.0 Final)

**Identificador:** `SPEC-E1-v1.0`

**Estado:** Normativo / Congelado para Implementación Dual (Rust y C#)

**Alcance:** Motor DES Headless Nivel 2 (Ethernet, ARP, IPv4, ICMP Echo, Switch L2, Enlaces Físicos)

---

## 1. Constantes Normativas del Sistema

Ambas implementaciones deben definir exactamente estas constantes enteras sin punto flotante:

| Constante | Valor Exacto | Unidad | Descripción Normativa |
| --- | --- | --- | --- |
| `LINK_PROPAGATION_DELAY_NS` | `1_000` | ns | Tiempo que tarda una trama en cruzar un enlace físico (`LinkTransmit` $\rightarrow$ `FrameArrived`). |
| `DEVICE_PROCESSING_DELAY_NS` | `100` | ns | Tiempo interno desde la recepción de un evento (`FrameArrived`) hasta la emisión de la trama resultante o desencolado ARP. La originación de un comando externo (`SendPing`) en el nodo origen tiene retardo `0 ns`. |
| `ARP_REQUEST_TIMEOUT_NS` | `1_000_000_000` | ns (1 s) | Tiempo de espera por cada intento de `ARP Request` antes de reintentar o abortar. |
| `ARP_MAX_ATTEMPTS` | `3` | intentos | Número máximo de solicitudes ARP emitidas (1 inicial + 2 reintentos) antes de declarar fallo de resolución. |
| `ARP_CACHE_TTL_NS` | `60_000_000_000` | ns (60 s) | Tiempo de vida de una entrada en `ArpCache` desde su creación o actualización por paquete ARP. |
| `MAC_TABLE_TTL_NS` | `300_000_000_000` | ns (300 s) | Tiempo de vida de una entrada en la tabla CAM del Switch desde el último aprendizaje de esa MAC de origen. |
| `MAX_ZERO_DELAY_EVENTS` | `1_000` | eventos | Límite de eventos consecutivos procesados en un mismo `VirtualTime` antes de disparar `HaltedBySafetyGuard(ZeroDelayLoop)`. |
| `MAX_PENDING_QUEUE_SIZE` | `100_000` | eventos | Límite máximo de eventos alojados simultáneamente en el `EventScheduler` antes de disparar `HaltedBySafetyGuard(QueueCapacityExceeded)`. |
| `DEFAULT_TRACE_CAPACITY` | `10_000` | registros | Capacidad por defecto del buffer circular de trazas (`TraceRingBuffer`). Configurable al instanciar el motor. |

---

## 2. Contratos y Tipos Públicos

Ningún tipo público expone claves generacionales (`SlotMap` keys o punteros internos). Toda interacción externa usa identificadores semánticos basados en cadenas o enteros secuenciales.

### 2.1. Identificadores y Primitivas de Red

```
DeviceRef(String)          // Ej: "pc1", "sw1"
InterfaceName(String)      // Ej: "eth0", "port1"
LinkRef(String)            // Ej: "link1"
EndpointRef { device: DeviceRef, interface: InterfaceName }

VirtualTime(u64)           // Nanosegundos desde t = 0
TraceSeq(u64)              // Secuencia monótona estricta (1, 2, 3...) por cada registro emitido al TraceBuffer
MacAddress([u8; 6])        // Constante BROADCAST = FF:FF:FF:FF:FF:FF
Ipv4Address([u8; 4])       // Con longitud de prefijo u8 (ej. 24) en configuración de interfaz

```

### 2.2. Modelo Completo de PDUs (Inmutables y Clones Exactos en Trazas)

A diferencia de versiones preliminares que guardaban solo resúmenes, **cada registro de traza conserva el valor íntegro de la PDU** para permitir inspección completa de cabeceras en cualquier momento:

```
EthernetFrame {
    src_mac: MacAddress,
    dst_mac: MacAddress,
    payload: EthernetPayload
}

enum EthernetPayload {
    Arp(ArpPacket),
    Ipv4(Ipv4Packet)
}

ArpPacket {
    operation: ArpOperation,   // Request | Reply
    sender_mac: MacAddress,
    sender_ip: Ipv4Address,
    target_mac: MacAddress,    // 00:00:00:00:00:00 en Request
    target_ip: Ipv4Address
}

Ipv4Packet {
    src_ip: Ipv4Address,
    dst_ip: Ipv4Address,
    ttl: u8,                   // Valor inicial por defecto en Host: 64
    payload: Ipv4Payload
}

enum Ipv4Payload {
    Icmp(IcmpPacket)
}

IcmpPacket {
    kind: IcmpKind,            // EchoRequest | EchoReply | DestinationHostUnreachable | TimeExceeded
    identifier: u16,
    sequence: u16
}

```

### 2.3. Comandos Públicos (`EngineCommand`)

1. `CreateHost { device: DeviceRef, interfaces: Vec<InterfaceName> }`
2. `CreateL2Switch { device: DeviceRef, ports: Vec<InterfaceName> }`
3. `ConfigureInterface { endpoint: EndpointRef, mac: MacAddress, ipv4: Option<(Ipv4Address, u8)>, admin_up: bool }`
4. `SetInterfaceAdminState { endpoint: EndpointRef, admin_up: bool }`
5. `ConnectLink { link: LinkRef, a: EndpointRef, b: EndpointRef }`
6. `DisconnectLink { link: LinkRef }`
7. `SendPing { source: DeviceRef, target_ip: Ipv4Address, identifier: u16, sequence: u16 }`
8. `StepEvents { max_steps: usize } -> ExecutionReport`
9. `AdvanceUntil { target_time: VirtualTime, max_steps_guard: usize } -> ExecutionReport`
10. `RunUntilQuiescent { max_steps_guard: usize } -> ExecutionReport`

---

## 3. Semántica Normativa del Motor, Tiempo y Protocolos

### 3.1. Clasificación de Eventos Internos y Orden Determinista

El `EventScheduler` ordena por `(at: VirtualTime, sequence: u64)` en orden ascendente estricto (Min-Heap). Los eventos internos se dividen en dos categorías formales:

1. **Eventos de Tráfico (`TrafficEvent`):**
* `FrameArrived { link_instance_id: InternalLinkKey, dst_iface: InternalIfaceKey, frame: EthernetFrame }`


2. **Eventos de Temporizador (`TimerEvent`):**
* `ArpResolveRetryTimer { node: InternalNodeKey, target_ip: Ipv4Address, attempt: u8, resolution_epoch: u64 }`
* `ArpEntryExpireTimer { node: InternalNodeKey, ip: Ipv4Address, entry_epoch: u64 }`
* `MacEntryExpireTimer { node: InternalNodeKey, mac: MacAddress, entry_epoch: u64 }`



**Regla de Determinismo en Inundación (Switch Flooding) y Desencolado:**

* Cuando un switch inunda una trama, debe iterar sus puertos en el **orden lexicográfico de `InterfaceName**` (o en el orden exacto de declaración del vector `ports` al crear el switch, siempre fijo), programando los envíos en ese orden determinista.
* Toda colección interna que se itere para generar eventos debe ser ordenada (`BTreeMap` / `SortedDictionary` o `Vec`). Prohibido iterar sobre `HashMap` / `Dictionary` con semilla aleatoria para emitir eventos.

---

### 3.2. Semántica Formal de los Tres Modos de Avance Temporal (Corrección #2)

| Operación | Condición de Parada y Reloj Virtual Final (`self.now`) | Cómo trata los `TimerEvent` (Expiración ARP/MAC y Reintentos) |
| --- | --- | --- |
| **`StepEvents(max_steps)`** | Extrae y procesa hasta `max_steps` eventos de cualquier tipo de la cola. Si la cola se vacía antes de alcanzar `max_steps`, **el reloj virtual permanece en el timestamp del último evento procesado** (o en el `self.now` previo si se procesaron `0` eventos) y retorna `Outcome::Idle`. | Procesa cualquier evento en el tope de la cola (`TrafficEvent` o `TimerEvent`) indistintamente. |
| **`AdvanceUntil(target_time, max_steps_guard)`** | Requiere `target_time >= self.now` (de lo contrario devuelve error `TargetTimeInPast`). Extrae y procesa todos los eventos cuyo `ev.at <= target_time`. Si no se dispara un límite antes, cuando el próximo evento en la cola tiene `ev.at > target_time` (o la cola está vacía), **el motor avanza explícitamente `self.now = target_time**` y retorna `Outcome::TargetTimeReached`. | Dispara todos los temporizadores (incluidos `ArpEntryExpireTimer` y `MacEntryExpireTimer`) cuyo timestamp sea `<= target_time`. |
| **`RunUntilQuiescent(max_steps_guard)`** | Define **Quiescencia (Reposo de Tráfico)** como el estado donde no existen `TrafficEvent` pendientes ni resoluciones ARP activas (`ArpResolveRetryTimer` pendientes). Ignora los timers pasivos de envejecimiento de caché a largo plazo (`ArpEntryExpireTimer` y `MacEntryExpireTimer`) como condición de continuación: es decir, si en la cola **solo** quedan timers de expiración de caché (`ArpEntryExpireTimer` / `MacEntryExpireTimer`), el motor **se detiene inmediatamente sin avanzar el reloj hasta los 60 s / 300 s** y retorna `Outcome::Quiescent`. (Si durante el avance hacia un `TrafficEvent` o `ArpResolveRetryTimer` se cruza el tiempo de un timer de expiración, sí se procesa en su orden cronológico). | Procesa `ArpResolveRetryTimer` (porque una resolución ARP pendiente es una transacción de tráfico activa), pero se detiene en cuanto solo restan timers pasivos de limpieza de tablas (`ExpireTimer`). |

---

### 3.3. Separación Estricta de Resultados de Ejecución (Corrección #5)

`BudgetExhausted` y `HaltedBySafetyGuard` **nunca son intercambiables**:

```
enum ExecutionOutcome {
    Idle,                                      // Cola totalmente vacía
    Quiescent,                                 // Sin tráfico ni resoluciones ARP en curso (solo quedan timers de expiración de tablas)
    TargetTimeReached,                         // Se alcanzó exactamente el target_time de AdvanceUntil
    BudgetExhausted,                           // Se procesaron exactamente max_steps / max_steps_guard eventos válidos y aún quedan eventos activos
    HaltedBySafetyGuard(SafetyHaltReason)      // Aborto por violación de invariante de seguridad
}

enum SafetyHaltReason {
    ZeroDelayLoop { at: VirtualTime, events_at_same_time: usize },
    QueueCapacityExceeded { pending_count: usize, limit: usize }
}

```

* **`BudgetExhausted`:** Es un corte de paginación normal controlado por el llamador. Si el llamador pide `StepEvents(max_steps: 1_000)` en medio de un bucle de switches y no se viola ningún límite físico del motor, el resultado obligatorio es **`BudgetExhausted`** con `processed_events == 1_000`.
* **`HaltedBySafetyGuard(ZeroDelayLoop)`:** Solo se dispara si se procesan más de `MAX_ZERO_DELAY_EVENTS` (`1_000`) eventos consecutivos **sin que `VirtualTime` avance ni un solo nanosegundo** (`ev.at == last_ev.at`). En un bucle físico de switches L2 donde cada salto suma `100 ns + 1_000 ns`, el tiempo sí avanza; por tanto, nunca dispara `ZeroDelayLoop`.
* **`HaltedBySafetyGuard(QueueCapacityExceeded)`:** Solo se dispara si `scheduler.len()` intenta superar `MAX_PENDING_QUEUE_SIZE` (`100_000`).

---

### 3.4. Máquina de Estados ARP: Reintentos, Fallo y Política de TTL (Correcciones #3 y #6)

#### A. Resolución ARP, Reintentos y Limpieza de Paquetes Pendientes

Cuando un Host necesita enviar un `Ipv4Packet` hacia `next_hop_ip` y no existe entrada válida en `arp_cache`:

1. **Encolamiento:** El paquete se añade a `pending_arp_queue[next_hop_ip]`.
2. **Si no había una resolución activa para `next_hop_ip`:**
* Se crea un estado de resolución activa con `attempt = 1` e identificador de época `resolution_epoch = next_epoch()`.
* Se transmite inmediatamente el `ARP Request` #1 (con el retardo correspondiente al contexto: `0 ns` si proviene de comando externo en `t_0`, o `100 ns` si proviniera de reenvío).
* Se programa `ArpResolveRetryTimer { node, target_ip: next_hop_ip, attempt: 1, resolution_epoch }` para `now + ARP_REQUEST_TIMEOUT_NS` (`+ 1_000_000_000 ns`).


3. **Al dispararse `ArpResolveRetryTimer`:**
* Primero se verifica si el timer sigue vigente (`active_resolution.epoch == timer.resolution_epoch`). Si la resolución ya se completó antes, el timer es obsoleto y se ignora sin emitir observaciones.
* Si sigue vigente y **`attempt < ARP_MAX_ATTEMPTS` (es decir, `attempt` es 1 o 2):**
* Se incrementa `attempt += 1`.
* Si la interfaz de salida está `admin_up` y tiene enlace conectado, se transmite un nuevo `ARP Request` (en `now`, ya que el propio evento del timer ocurre en el nodo) y se emite `FrameTransmitted`.
* Se programa el siguiente `ArpResolveRetryTimer` con el nuevo `attempt` para `now + 1_000_000_000 ns`.


* Si sigue vigente y **`attempt == ARP_MAX_ATTEMPTS` (expiró el intento #3 sin respuesta en `t_0 + 3_000_000_000 ns`):**
* Se elimina la resolución activa.
* Se extraen y eliminan **todos** los paquetes acumulados en `pending_arp_queue[next_hop_ip]`.
* Por cada paquete eliminado (en orden FIFO), se emite el registro de traza `PacketDropped { node, layer: L3, pdu: Ipv4(pkt), reason: DropReason::ArpResolutionFailed }` y, si el paquete pertenecía a un `SendPing` originado en este host, se marca la sesión ICMP como fallida emitiendo `PingFailed { host, identifier, sequence, reason: PingFailureReason::ArpTimeout, at: now }`.





#### B. Política de TTL e Invalidación de Timers Obsoletos (Lazy Epoch Validation)

* **Regla de NO-Renovación por Uso (Hit de Lectura):** Consultar una entrada en `ArpCache` para encapsular un paquete IPv4 saliente **NO renueva su TTL**. Una entrada ARP solo se crea o renueva su TTL (`now + 60_000_000_000 ns`) cuando se procesa un paquete ARP entrante válido (`ARP Request` dirigido a la IP del host o `ARP Reply`).
* **Regla de Renovación en Tabla MAC del Switch:** Aprender la `src_mac` de una trama entrante en un puerto del switch **sí actualiza** su puerto asociado y renueva su expiración a `now + 300_000_000_000 ns`.
* **Invalidación de Timers por Época (`entry_epoch: u64`):** Cada entrada en `ArpCache` y `MacTable` almacena un entero `epoch: u64` que se incrementa cada vez que esa clave se inserta o actualiza. El evento `ArpEntryExpireTimer` / `MacEntryExpireTimer` lleva copiado ese `entry_epoch`. Cuando el timer llega al tope de la cola en el futuro:
* Si la entrada ya no existe o su `current_entry.epoch != timer.entry_epoch`, el timer es obsoleto y **se descarta silenciosamente** sin borrar la entrada actualizada ni emitir eventos de observación.



---

### 3.5. Invalidación de Enlaces en Vuelo (Corrección #4)

Comprobar únicamente que la interfaz destino existe o tiene un cable conectado **es insuficiente** (el usuario pudo desconectar `link1` mientras había una trama en vuelo y conectar un nuevo `link2` en el mismo puerto antes de que se cumplieran los `1_000 ns`).

**Regla Normativa:**

1. Cada enlace conectado recibe una clave generacional única (`link_instance_id: InternalLinkKey` de `SlotMap`) en el instante de ejecutarse `ConnectLink`.
2. Cuando una interfaz transmite una trama, el evento programado en el scheduler es:
`FrameArrived { link_instance_id, dst_iface, frame }`
3. Al llegar `FrameArrived` en `t + 1_000 ns`, el motor verifica tres condiciones:
* Que `link_instance_id` **siga existiendo exactamente en el `SlotMap` de enlaces activos** (`topology.links.contains_key(link_instance_id)`).
* Que `dst_iface` siga existiendo y su `attached_link == Some(link_instance_id)`.
* Que `dst_iface.admin_up == true`.


4. Si el enlace fue desconectado o reemplazado (`!contains_key(link_instance_id)`), la trama murió en el cable destruido y se registra `PacketDropped { reason: DropReason::LinkDisconnectedInFlight }`. Si `dst_iface.admin_up == false`, se registra `PacketDropped { reason: DropReason::IngressInterfaceDown }`.

---

### 3.6. Modelo de Trazabilidad, Retención de PDUs y Capacidad (Corrección #7)

El motor mantiene un `TraceRingBuffer` con capacidad máxima `capacity` (por defecto `10_000`):

```
struct TraceBufferSnapshot {
    total_emitted_ever: u64,       // Contador total de registros generados desde t = 0
    dropped_by_capacity: u64,      // Cuántos registros antiguos fueron sobrescritos (total_emitted - records.len())
    oldest_available_seq: Option<TraceSeq>,
    newest_available_seq: Option<TraceSeq>,
    records: Vec<TraceRecord>
}

struct TraceRecord {
    seq: TraceSeq,                 // 1, 2, 3... estrictamente secuencial sin huecos
    at: VirtualTime,
    event: ObservableEvent
}

enum ObservableEvent {
    FrameTransmitted { link: LinkRef, from: EndpointRef, to: EndpointRef, arrives_at: VirtualTime, frame: EthernetFrame },
    FrameDelivered { link: LinkRef, at_endpoint: EndpointRef, frame: EthernetFrame },
    PacketDropped { at_device: DeviceRef, at_interface: Option<InterfaceName>, pdu: DroppedPdu, reason: DropReason },
    ArpPacketQueued { at_device: DeviceRef, target_ip: Ipv4Address, packet: Ipv4Packet },
    TableUpdated { at_device: DeviceRef, update: TableUpdateDetail },
    PingCompleted { host: DeviceRef, target_ip: Ipv4Address, identifier: u16, sequence: u16, rtt_ns: u64 },
    PingFailed { host: DeviceRef, target_ip: Ipv4Address, identifier: u16, sequence: u16, reason: PingFailureReason }
}

```

* Cuando `records.len() == capacity` y se emite un nuevo `TraceRecord`, se elimina el más antiguo (`pop_front()`), se inserta el nuevo (`push_back()`) con `seq = total_emitted_ever + 1`, y `dropped_by_capacity` se incrementa en `1`. Así el consumidor sabe con exactitud matemática cuántos eventos históricos se descartaron por límite de memoria (`oldest_available_seq == dropped_by_capacity + 1`).

---

## 4. Los Seis Escenarios de Referencia y Trazas Calculadas

### Topología Base Común (Escenarios 1 a 5)

* **Nodos:** `pc1`, `pc2`, `pc3` (Hosts con interfaz `eth0`) y `sw1` (Switch L2 con puertos `port1`, `port2`, `port3`, `port4`).
* **Configuración:**
* `pc1:eth0`: MAC `00:00:00:00:00:01`, IP `192.168.1.10/24`, `admin_up = true`
* `pc2:eth0`: MAC `00:00:00:00:00:02`, IP `192.168.1.20/24`, `admin_up = true`
* `pc3:eth0`: MAC `00:00:00:00:00:03`, IP `192.168.1.30/24`, `admin_up = true`
* `sw1:port1..port4`: `admin_up = true`


* **Enlaces Iniciales:**
* `link1`: `pc1:eth0` $\leftrightarrow$ `sw1:port1`
* `link2`: `pc2:eth0` $\leftrightarrow$ `sw1:port2`
* `link3`: `pc3:eth0` $\leftrightarrow$ `sw1:port3`
* (`sw1:port4` desconectado)



---

### Escenario 1: Resolución ARP Exitosa y Encolamiento de Paquete

* **Precondiciones:** Topología Base en `t = 0`. Tablas ARP y MAC vacías.
* **Acción:**
1. `SendPing { source: "pc1", target_ip: 192.168.1.20, identifier: 1, sequence: 1 }`
2. `AdvanceUntil { target_time: 4_300, max_steps_guard: 100 }`


* **Traza Cronológica Justificada (0 a 4.300 ns):**
* **`t = 0 ns`:** `pc1` origina el paquete IPv4/ICMP (`0 ns` por ser origen de comando). Al no tener ARP para `192.168.1.20`, encola el paquete (`ArpPacketQueued`) y transmite `ARP Request` broadcast por `link1` (`FrameTransmitted`, llega en `1_000 ns`). Programa `ArpResolveRetryTimer(attempt: 1)` para `t = 1_000_000_000 ns`.
* **`t = 1_000 ns`:** `ARP Request` llega a `sw1:port1` (`FrameDelivered`). `sw1` aprende `00:00:00:00:00:01 -> port1` (`TableUpdated`, expira en `t = 300_000_001_000 ns`). Programa salida inundada tras `+100 ns` de procesamiento.
* **`t = 1_100 ns`:** `sw1` transmite `ARP Request` por `port2` (`link2`) y `port3` (`link3`) en ese orden determinista (`FrameTransmitted`, llegan en `2_100 ns`).
* **`t = 2_100 ns`:**
* Entrega en `pc2:eth0` (`FrameDelivered`): `pc2` aprende `192.168.1.10 -> 00:00:00:00:00:01` en su caché ARP (`TableUpdated`, expira en `t = 60_000_002_100 ns`) y programa `ARP Reply` tras `+100 ns` de procesamiento.
* Entrega en `pc3:eth0` (`FrameDelivered`): `pc3` descarta la trama por IP destino distinta (`PacketDropped { reason: ArpTargetIpMismatch }`).


* **`t = 2_200 ns`:** `pc2:eth0` transmite `ARP Reply` unicast hacia `00:00:00:00:00:01` por `link2` (`FrameTransmitted`, llega en `3_200 ns`).
* **`t = 3_200 ns`:** `ARP Reply` llega a `sw1:port2` (`FrameDelivered`). `sw1` aprende `00:00:00:00:00:02 -> port2` (`TableUpdated`, expira en `t = 300_000_003_200 ns`). Como conoce el puerto de `00:00:00:00:00:01` (`port1`), programa reenvío unicast tras `+100 ns`.
* **`t = 3_300 ns`:** `sw1:port1` transmite `ARP Reply` por `link1` (`FrameTransmitted`, llega en `4_300 ns`).
* **`t = 4_300 ns`:** `ARP Reply` llega a `pc1:eth0` (`FrameDelivered`). `pc1` registra `192.168.1.20 -> 00:00:00:00:00:02` en su caché ARP (`TableUpdated`, expira en `t = 60_000_004_300 ns`), invalida la resolución ARP pendiente y programa la salida del paquete ICMP desencolado tras `+100 ns` de procesamiento (`t = 4_400 ns`).


* **Aserciones Exactas al terminar `AdvanceUntil(4_300)`:**
* `outcome == TargetTimeReached` y `current_time == 4_300`.
* `pc1` tiene `192.168.1.20 -> 00:00:00:00:00:02` en su caché ARP y su `pending_arp_queue` está vacía.
* `sw1` tiene exactamente 2 entradas MAC (`port1` y `port2`).



---

### Escenario 2: Ping Exitoso en Frío (`8.700 ns`) y Segundo Ping en Caliente (`4.300 ns`) — *(Corrección #1)*

* **Precondiciones:** Continuación del Escenario 1 (o ejecutando desde `t = 0` con `SendPing(seq: 1)` seguido de `RunUntilQuiescent`).
* **Acción Parte A (Completar Ping #1):** Ejecutar `RunUntilQuiescent { max_steps_guard: 100 }`.
* **Traza Cronológica Justificada del Tramo ICMP (4.400 a 8.700 ns):**
* **`t = 4_400 ns`:** Cumplidos los `100 ns` de procesamiento en `pc1` tras recibir el `ARP Reply` en `4_300 ns`, `pc1:eth0` transmite `ICMP Echo Request (seq=1)` hacia `00:00:00:00:00:02` (`FrameTransmitted`, llega en `5_400 ns`).
* **`t = 5_400 ns`:** Llega a `sw1:port1` (`FrameDelivered`). `sw1` refresca el TTL de la MAC `..:01` y programa reenvío unicast a `port2` tras `+100 ns`.
* **`t = 5_500 ns`:** `sw1:port2` transmite `ICMP Echo Request` (`FrameTransmitted`, llega en `6_500 ns`).
* **`t = 6_500 ns`:** Llega a `pc2:eth0` (`FrameDelivered`). `pc2` procesa el Echo Request (`+100 ns`), consulta su caché ARP (donde ya existe `192.168.1.10 -> ..:01` sin renovar su TTL) y programa el `ICMP Echo Reply`.
* **`t = 6_600 ns`:** `pc2:eth0` transmite `ICMP Echo Reply (seq=1)` (`FrameTransmitted`, llega en `7_600 ns`).
* **`t = 7_600 ns`:** Llega a `sw1:port2` (`FrameDelivered`). `sw1` procesa (`+100 ns`) y programa salida por `port1`.
* **`t = 7_700 ns`:** `sw1:port1` transmite `ICMP Echo Reply` (`FrameTransmitted`, llega en `8_700 ns`).
* **`t = 8_700 ns`:** Llega a `pc1:eth0` (`FrameDelivered`). `pc1` emite `PingCompleted { identifier: 1, sequence: 1, rtt_ns: 8_700 }`. La cola queda sin eventos de tráfico ni resoluciones ARP activas; el motor se detiene en `t = 8_700 ns` con `Outcome::Quiescent`.


* **Acción Parte B (Segundo Ping con ARP Caliente en `t = 10_000 ns`):**
1. `AdvanceUntil { target_time: 10_000, max_steps_guard: 100 }` (avanza el reloj en vacío hasta `10_000 ns`).
2. `SendPing { source: "pc1", target_ip: 192.168.1.20, identifier: 1, sequence: 2 }`.
3. `RunUntilQuiescent { max_steps_guard: 100 }`.


* **Justificación Matemática del RTT en Caliente (`4_300 ns`):**
* `t = 10_000 ns`: `pc1` origina y transmite `ICMP Echo Request (seq=2)` en `0 ns` (cache ARP hit).
* `t = 11_000 ns`: Llega a `sw1:port1` (`+1_000 ns` enlace 1).
* `t = 11_100 ns`: Sale de `sw1:port2` (`+100 ns` procesamiento switch).
* `t = 12_100 ns`: Llega a `pc2:eth0` (`+1_000 ns` enlace 2).
* `t = 12_200 ns`: Sale `ICMP Echo Reply (seq=2)` de `pc2:eth0` (`+100 ns` procesamiento en `pc2`).
* `t = 13_200 ns`: Llega a `sw1:port2` (`+1_000 ns` enlace 2).
* `t = 13_300 ns`: Sale de `sw1:port1` (`+100 ns` procesamiento switch).
* `t = 14_300 ns`: Llega a `pc1:eth0` (`+1_000 ns` enlace 1).
* **Suma total:** $4 \times 1\,000\text{ ns (enlaces)} + 3 \times 100\text{ ns (sw1 ida + pc2 respuesta + sw1 vuelta)} = \mathbf{4\,300\text{ ns}}$.


* **Aserciones Exactas Parte B:**
* `outcome == Quiescent`, `current_time == 14_300`, `PingCompleted.rtt_ns == 4_300`.
* `pc3` no recibió ninguna trama durante la Parte B.



---

### Escenario 3: Resolución ARP Fallida, Reintentos, Limpieza de Cola e Invalidación de Enlace en Vuelo

* **Sub-escenario 3A (Interfaz Destino Administrativamente Caída y Agotamiento de ARP):**
* **Precondición:** Topología Base en `t = 0`, pero ejecutando `SetInterfaceAdminState { endpoint: "pc2:eth0", admin_up: false }`.
* **Acción:** `SendPing { source: "pc1", target_ip: 192.168.1.20, identifier: 1, sequence: 1 }` seguido de `RunUntilQuiescent { max_steps_guard: 500 }`.
* **Traza y Aserciones Exactas:**
* Intento #1 de `ARP Request` sale de `pc1` en `t = 0 ns`, llega a `pc2:eth0` en `t = 2_100 ns` y es descartado con `PacketDropped { at_device: "pc2", reason: IngressInterfaceDown }`.
* Intento #2 (`ArpResolveRetryTimer` `attempt: 1`) se dispara en `t = 1_000_000_000 ns` ($1\text{ s}$) y transmite el 2.º `ARP Request`.
* Intento #3 (`ArpResolveRetryTimer` `attempt: 2`) se dispara en `t = 2_000_000_000 ns` ($2\text{ s}$) y transmite el 3.º `ARP Request`.
* Expiración final (`ArpResolveRetryTimer` `attempt: 3`) se dispara en **`t = 3_000_000_000 ns` ($3\text{ s}$)**: `pc1` vacía su `pending_arp_queue[192.168.1.20]`, emite `PacketDropped { at_device: "pc1", reason: ArpResolutionFailed }` para el paquete ICMP pendiente y emite `PingFailed { reason: ArpTimeout }` en `t = 3_000_000_000 ns`.
* El motor termina en `Outcome::Quiescent` exactamente en **`t = 3_000_000_000 ns`** con `pending_arp_queue` completamente vacía.




* **Sub-escenario 3B (Invalidación por Reemplazo de Enlace con Trama en Vuelo — Corrección #4):**
* **Precondición:** Topología Base en `t = 0`.
* **Acción:**
1. `SendPing` de `pc1` a `pc2` en `t = 0` (la trama `ARP Request` sale hacia `link1` con llegada prevista en `t = 1_000 ns`).
2. `AdvanceUntil { target_time: 500, max_steps_guard: 10 }` (la trama está a mitad de camino en `link1`).
3. En `t = 500 ns`, ejecutar `DisconnectLink { link: "link1" }` e inmediatamente reconectar un nuevo enlace `ConnectLink { link: "link1_new", a: "pc1:eth0", b: "sw1:port1" }`.
4. `AdvanceUntil { target_time: 1_500, max_steps_guard: 10 }`.


* **Aserción Exacta:** En `t = 1_000 ns`, aunque `sw1:port1` está conectado a `link1_new`, el evento `FrameArrived` lleva la clave generacional del extinto `link1`. El motor detecta la discrepancia de identidad del enlace, **no entrega la trama a `sw1**` y emite `PacketDropped { reason: LinkDisconnectedInFlight }` en `t = 1_000 ns`.



---

### Escenario 4: Migración de Puerto Físico en Switch L2 (Actualización de Tabla CAM)

* **Precondiciones:** Ejecutar el Escenario 1 y la Parte A del Escenario 2 hasta `t = 8_700 ns` (`sw1` asocia `00:00:00:00:00:02 -> port2`).
* **Acción:**
1. `AdvanceUntil { target_time: 20_000, max_steps_guard: 100 }`.
2. `DisconnectLink { link: "link2" }`.
3. `ConnectLink { link: "link4", a: "pc2:eth0", b: "sw1:port4" }`.
4. `SendPing { source: "pc2", target_ip: 192.168.1.10, identifier: 2, sequence: 1 }`.
5. `RunUntilQuiescent { max_steps_guard: 100 }`.


* **Aserciones Exactas:**
* En `t = 21_000 ns`, cuando el `ICMP Echo Request` de `pc2` entra por `sw1:port4`, `sw1` actualiza la entrada de `00:00:00:00:00:02` hacia `port4` con nuevo TTL `300_000_021_000 ns` e incrementa su `entry_epoch`.
* La tabla MAC de `sw1` sigue teniendo exactamente **2 entradas totales** (sin entradas duplicadas para `00:00:00:00:00:02`).
* El `ICMP Echo Reply` de `pc1` es reenviado por `sw1` de forma unicast hacia `port4` y completa el ping en `t = 24_300 ns` (`RTT = 4_300 ns`).



---

### Escenario 5: Expiración Determinista de Caché ARP y No-Renovación por Uso

* **Precondiciones:** Completar el Escenario 1 y 2 completo (incluyendo el segundo ping en `t = 10_000 ns`, que terminó en `t = 14_300 ns`).
* Recordatorio de estado: La entrada ARP `192.168.1.20 -> 00:00:00:00:00:02` en `pc1` fue creada en `t = 4_300 ns`. Su uso en `t = 10_000 ns` **no renovó** su TTL, por lo que su expiración sigue programada exactamente en **`t = 60_000_004_300 ns`**.


* **Acción y Aserciones:**
1. `AdvanceUntil { target_time: 60_000_004_299, max_steps_guard: 100 }` (`1 ns` antes de expirar):
* **Aserción 5.1:** La entrada `192.168.1.20` **sigue presente** en la caché ARP de `pc1`.


2. `AdvanceUntil { target_time: 60_000_004_300, max_steps_guard: 100 }` (instante exacto de expiración):
* **Aserción 5.2:** Se procesa `ArpEntryExpireTimer`, la entrada `192.168.1.20` **es eliminada** de la caché ARP de `pc1` y se emite `TableUpdated { at_device: "pc1", update: ArpEntryExpired(192.168.1.20) }` con timestamp `60_000_004_300`.


3. En `t = 61_000_000_000 ns`, ejecutar `AdvanceUntil(61_000_000_000)`, lanzar `SendPing(source: "pc1", target_ip: 192.168.1.20, id: 1, seq: 3)` y `RunUntilQuiescent(100)`:
* **Aserción 5.3:** Como la caché ARP de `pc1` expiró (pero la de `sw1` dura `300 s` y sigue caliente), `pc1` debe emitir nuevamente un `ARP Request` broadcast antes de enviar el ICMP.





---

### Escenario 6: Ciclo de Capa 2, Presupuesto de Pasos y Truncamiento Controlado de Trazas

* **Precondiciones:** Instanciar el motor con capacidad de traza reducida `trace_capacity = 500`. Topología con `pc1` (`192.168.1.10/24`), `sw1` (4 puertos) y `sw2` (4 puertos):
* `link1`: `pc1:eth0` $\leftrightarrow$ `sw1:port1`
* `link_loop_a`: `sw1:port2` $\leftrightarrow$ `sw2:port2`
* `link_loop_b`: `sw1:port3` $\leftrightarrow$ `sw2:port3`


* **Acción:**
1. `SendPing { source: "pc1", target_ip: 192.168.1.99, identifier: 9, sequence: 1 }` (IP inexistente que genera un `ARP Request` broadcast).
2. `StepEvents { max_steps: 1_000 }`.


* **Aserciones Exactas:**
1. `outcome == ExecutionOutcome::BudgetExhausted` (y **NO** `HaltedBySafetyGuard`, ya que el tiempo virtual avanza en cada salto y la cola no supera `100_000` eventos).
2. `processed_events == 1_000`.
3. Al consultar `get_trace_snapshot()`:
* `records.len() == 500` (respetando la cota de memoria configurada).
* `dropped_by_capacity == total_emitted_ever - 500` (con `dropped_by_capacity > 0`).
* `oldest_available_seq == Some(TraceSeq(dropped_by_capacity + 1))` y `newest_available_seq == Some(TraceSeq(total_emitted_ever))`.





---

## 5. Reglas de Comparación Agnéstica entre Rust y C# (Criterio de Aceptación)

Para comparar los prototipos de Rust y C# sin depender de nombres de variables ni estructuras internas, ambos ejecutables/tests deben exponer un serializador de reporte JSON canónico por escenario (`ScenarioCanonicalReport`) que será idéntico byte a byte (o campo a campo):

```json
{
  "scenario_id": "E1-S2",
  "final_virtual_time_ns": 14300,
  "execution_outcome": "Quiescent",
  "total_internal_events_processed": 15,
  "trace_summary": {
    "total_emitted_ever": 25,
    "dropped_by_capacity": 0,
    "records": [
      {
        "seq": 1,
        "at_ns": 0,
        "kind": "ArpPacketQueued",
        "device": "pc1",
        "details": "target=192.168.1.20 icmp_seq=1"
      }
    ]
  }
}

```

### Criterios de Aceptación Iguales para Ambos Lenguajes

1. **Identidad de Trazas:** En los Escenarios 1 a 6, ambos motores deben producir **exactamente el mismo `final_virtual_time_ns`, el mismo `execution_outcome`, el mismo número de eventos internos procesados y la misma secuencia ordenada de `TraceRecord**`.
2. **Determinismo Intra-Lenguaje:** Ejecutar cualquiera de los escenarios 100 veces seguidas en el mismo lenguaje debe producir el mismo hash SHA-256 sobre el JSON de salida.
3. **Cero Dependencias de Tiempo Real:** Ninguno de los motores puede invocar relojes del sistema operativo (`std::time::Instant` / `DateTime.UtcNow`) ni generadores aleatorios sin semilla dentro de la lógica de simulación.

Con esta especificación congelada, ambos agentes implementadores cuentan con todas las reglas matemáticas, temporales y de estado necesarias para escribir el código de `spike_e1_rust` y `spike_e1_csharp` sin supuestos implícitos.