import type { PresentationEvent } from '../contracts/presentation'

export interface PedagogicalExplanation {
  readonly summary: string
  readonly rationale: string
}

/**
 * This catalogue is presentation-only. It never changes the engine snapshot
 * and every narrative states either an emitted fact or an explicitly marked
 * didactic reading of that fact.
 */
export function explainEvent(event: PresentationEvent, previous: PresentationEvent | null): PedagogicalExplanation {
  const actor = event.actor.device_id.toUpperCase()

  if (event.kind === 'arp_packet_queued') {
    const ip = event.pdu?.pdu.kind === 'ipv4' ? event.pdu.pdu.dst_ip : 'el destino'
    return {
      summary: `${actor} encola el paquete IPv4/ICMP hacia ${ip} mientras espera resolución ARP.`,
      rationale: 'Hecho del motor: la PDU observada está en disposición queued y contiene IPv4/ICMP. Esta vista no afirma que un ARP Request esté dentro del buffer.',
    }
  }

  if (event.table_delta?.kind === 'mac_learned') {
    const context = previous?.protocols.includes('ipv4') || previous?.protocols.includes('icmp')
      ? ' La entrada aparece inmediatamente después de tráfico IPv4/ICMP en la traza; esa vecindad es contexto didáctico, no una relación causal emitida por el motor.'
      : ''
    return {
      summary: `${actor} actualiza su tabla CAM: ${event.table_delta.mac} → ${event.table_delta.port_interface_id}.`,
      rationale: `Hecho del motor: actualización de la tabla CAM mediante delta mac_learned y protocolo del evento ethernet.${context}`,
    }
  }

  if (event.table_delta?.kind === 'arp_learned') {
    return {
      summary: `${actor} actualiza caché ARP: ${event.table_delta.ip} → ${event.table_delta.mac}.`,
      rationale: 'Hecho del motor: delta arp_learned. La explicación didáctica no añade campos de cabecera no instrumentados.',
    }
  }

  if (event.kind === 'packet_dropped') {
    return {
      summary: `${actor} registra un descarte: ${event.drop_reason ?? 'sin razón codificada'}.`,
      rationale: 'Hecho del motor: packet_dropped y su razón emitida. No se infiere una decisión adicional por capa OSI.',
    }
  }

  const ping = event.ping
  if (event.kind === 'ping_completed' && ping !== null && ping.rtt_ns !== null) {
    return {
      summary: `${actor} completa el ping hacia ${ping.target_ip} en ${formatNs(ping.rtt_ns)}.`,
      rationale: 'Hecho del motor: ping_completed con RTT emitido por el motor.',
    }
  }

  return {
    summary: `${actor}: ${event.kind.replaceAll('_', ' ')}.`,
    rationale: 'Evento observable emitido por el motor. Las explicaciones didácticas no añaden causalidad al contrato.',
  }
}

export const PDU_FIELD_NOTES: Readonly<Record<string, string>> = {
  ethernet_src: 'Dirección MAC emitida por el motor para la trama Ethernet observada.',
  ethernet_dst: 'Dirección MAC emitida por el motor para la trama Ethernet observada.',
  ethernet_type: 'Interpretación didáctica del tipo de carga indicado por el discriminante semántico del contrato.',
  ethernet_fcs: 'E1 no emite ni calcula FCS/CRC32.',
  arp_operation: 'Operación ARP emitida por el motor.',
  arp_sender: 'Dirección de emisor emitida por el motor.',
  arp_target: 'Dirección de destino emitida por el motor.',
  arp_context: 'Parámetro de formato ARP presentado como contexto didáctico; no es un campo instrumentado por E1.',
  ipv4_addresses: 'Direcciones IPv4 emitidas por el motor.',
  ipv4_ttl: 'TTL semántico emitido por el motor.',
  ipv4_context: 'Contexto de formato IPv4; E1 no instrumenta IHL ni checksum.',
  icmp_kind: 'Clase ICMP emitida por el motor.',
  icmp_sequence: 'Identificador o secuencia emitidos por el motor como cadena decimal.',
  icmp_context: 'Contexto de formato ICMP; E1 no instrumenta código ni checksum.',
}

function formatNs(value: string): string {
  const ns = BigInt(value)
  if (ns < 1000n) return `${ns} ns`
  return `${ns / 1000n}.${(ns % 1000n).toString().padStart(3, '0')} µs`
}
