//! Pacotes IPv4 crus: o pouco que o NAT 1:1 precisa (RF-53 a RF-55).
//!
//! Não há pilha aqui. Só lemos cabeçalhos, trocamos um endereço e corrigimos
//! os checksums de forma incremental (RFC 1624) — o payload nunca é tocado.

// Fora do Windows só os testes usam este módulo: não há engine para chamá-lo.
#![cfg_attr(not(windows), allow(dead_code))]

use std::net::Ipv4Addr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Proto {
    Tcp,
    Udp,
}

impl Proto {
    fn from_number(number: u8) -> Option<Self> {
        match number {
            6 => Some(Proto::Tcp),
            17 => Some(Proto::Udp),
            _ => None,
        }
    }
}

/// Versão do IP no primeiro nibble; o que não é 4 nem 6 não nos interessa.
pub fn ip_version(packet: &[u8]) -> Option<u8> {
    packet.first().map(|b| b >> 4).filter(|v| *v == 4 || *v == 6)
}

/// Cabeçalhos de um pacote IPv4 TCP/UDP, já validados contra o tamanho.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Headers {
    pub proto: Proto,
    pub src: Ipv4Addr,
    pub dst: Ipv4Addr,
    pub src_port: u16,
    pub dst_port: u16,
    /// Tamanho do cabeçalho IP em bytes (20–60).
    pub ip_header_len: usize,
    /// `Length` do cabeçalho IP; pode ser menor que o buffer, nunca maior.
    pub total_len: usize,
}

/// `None` para o que não é IPv4 TCP/UDP inteiro — inclusive fragmentos que não
/// são o primeiro (não têm cabeçalho de transporte para ler a porta).
pub fn parse(packet: &[u8]) -> Option<Headers> {
    if ip_version(packet)? != 4 || packet.len() < 20 {
        return None;
    }
    let ip_header_len = usize::from(packet[0] & 0x0f) * 4;
    let total_len = usize::from(u16::from_be_bytes([packet[2], packet[3]]));
    if ip_header_len < 20 || total_len < ip_header_len + 4 || packet.len() < total_len {
        return None;
    }
    // Fragment offset diferente de zero: não é o primeiro fragmento.
    if u16::from_be_bytes([packet[6], packet[7]]) & 0x1fff != 0 {
        return None;
    }
    let proto = Proto::from_number(packet[9])?;
    let transport = &packet[ip_header_len..];
    let min_transport = match proto {
        Proto::Tcp => 20,
        Proto::Udp => 8,
    };
    if transport.len() < min_transport {
        return None;
    }

    Some(Headers {
        proto,
        src: Ipv4Addr::new(packet[12], packet[13], packet[14], packet[15]),
        dst: Ipv4Addr::new(packet[16], packet[17], packet[18], packet[19]),
        src_port: u16::from_be_bytes([transport[0], transport[1]]),
        dst_port: u16::from_be_bytes([transport[2], transport[3]]),
        ip_header_len,
        total_len,
    })
}

/// Troca o endereço de origem e corrige os checksums (saída → túnel).
pub fn rewrite_source(packet: &mut [u8], headers: &Headers, new_src: Ipv4Addr) {
    rewrite_address(packet, headers, 12, new_src);
}

/// Troca o endereço de destino e corrige os checksums (túnel → entrada).
pub fn rewrite_destination(packet: &mut [u8], headers: &Headers, new_dst: Ipv4Addr) {
    rewrite_address(packet, headers, 16, new_dst);
}

fn rewrite_address(packet: &mut [u8], headers: &Headers, offset: usize, new: Ipv4Addr) {
    let old: [u8; 4] = packet[offset..offset + 4].try_into().expect("4 bytes");
    let new = new.octets();
    if old == new {
        return;
    }
    packet[offset..offset + 4].copy_from_slice(&new);

    // Cabeçalho IP.
    let ip_csum = u16::from_be_bytes([packet[10], packet[11]]);
    let ip_csum = update_checksum(ip_csum, &old, &new);
    packet[10..12].copy_from_slice(&ip_csum.to_be_bytes());

    // Pseudo-cabeçalho de TCP/UDP inclui os endereços.
    let csum_offset = headers.ip_header_len
        + match headers.proto {
            Proto::Tcp => 16,
            Proto::Udp => 6,
        };
    let transport_csum = u16::from_be_bytes([packet[csum_offset], packet[csum_offset + 1]]);
    // UDP com checksum 0 significa "sem checksum"; tem que continuar 0.
    if headers.proto == Proto::Udp && transport_csum == 0 {
        return;
    }
    let transport_csum = update_checksum(transport_csum, &old, &new);
    packet[csum_offset..csum_offset + 2].copy_from_slice(&transport_csum.to_be_bytes());
}

/// RF-55 / T-12: num SYN de TCP, reduz a opção MSS para `max_mss` se ela for
/// maior. Devolve `true` se alterou.
pub fn clamp_mss(packet: &mut [u8], headers: &Headers, max_mss: u16) -> bool {
    if headers.proto != Proto::Tcp {
        return false;
    }
    let tcp = headers.ip_header_len;
    let flags = packet[tcp + 13];
    if flags & 0x02 == 0 {
        return false; // não é SYN
    }
    let data_offset = usize::from(packet[tcp + 12] >> 4) * 4;
    if data_offset < 20 || tcp + data_offset > packet.len() {
        return false;
    }

    let mut i = tcp + 20;
    let end = tcp + data_offset;
    while i < end {
        match packet[i] {
            0 => break,    // fim das opções
            1 => i += 1,   // NOP
            kind => {
                if i + 1 >= end {
                    break;
                }
                let len = usize::from(packet[i + 1]);
                if len < 2 || i + len > end {
                    break;
                }
                if kind == 2 && len == 4 {
                    let old = [packet[i + 2], packet[i + 3]];
                    let current = u16::from_be_bytes(old);
                    if current <= max_mss {
                        return false;
                    }
                    let new = max_mss.to_be_bytes();
                    packet[i + 2..i + 4].copy_from_slice(&new);
                    let csum_offset = tcp + 16;
                    let csum = u16::from_be_bytes([packet[csum_offset], packet[csum_offset + 1]]);
                    let csum = update_checksum(csum, &old, &new);
                    packet[csum_offset..csum_offset + 2].copy_from_slice(&csum.to_be_bytes());
                    return true;
                }
                i += len;
            }
        }
    }
    false
}

/// T-05: só isto vai para o túnel. Tudo que é local — rede privada, link-local,
/// loopback, multicast, broadcast, CGNAT — segue pela rede de sempre.
pub fn is_public(ip: Ipv4Addr) -> bool {
    let [a, b, _, _] = ip.octets();
    !(ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_multicast()
        || ip.is_broadcast()
        || ip.is_unspecified()
        || ip.is_documentation()
        || (a == 100 && (64..=127).contains(&b)) // 100.64.0.0/10, CGNAT
        || a == 0
        || a >= 240) // 240.0.0.0/4, reservado
}

// ============================ Checksums ====================================

/// Soma em complemento de um, 16 bits, sem o `!` final.
fn sum16(data: &[u8]) -> u32 {
    let mut sum = 0u32;
    let mut chunks = data.chunks_exact(2);
    for chunk in &mut chunks {
        sum += u32::from(u16::from_be_bytes([chunk[0], chunk[1]]));
    }
    if let [last] = chunks.remainder() {
        sum += u32::from(*last) << 8;
    }
    sum
}

fn fold(mut sum: u32) -> u16 {
    while sum >> 16 != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    sum as u16
}

/// RFC 1624, eq. 3: `HC' = ~(~HC + ~m + m')`, para `old` e `new` do mesmo
/// tamanho (par de bytes).
fn update_checksum(checksum: u16, old: &[u8], new: &[u8]) -> u16 {
    debug_assert_eq!(old.len(), new.len());
    let mut sum = u32::from(!checksum);
    sum += sum16(new);
    // ~m, palavra a palavra: a soma de complementos de 16 bits.
    let mut old_words = old.chunks_exact(2);
    for chunk in &mut old_words {
        sum += u32::from(!u16::from_be_bytes([chunk[0], chunk[1]]));
    }
    !fold(sum)
}

fn checksum(data: &[u8]) -> u16 {
    !fold(sum16(data))
}

/// Recalcula o checksum do cabeçalho IPv4 do zero.
pub fn recompute_ip_checksum(packet: &mut [u8], headers: &Headers) {
    packet[10] = 0;
    packet[11] = 0;
    let csum = checksum(&packet[..headers.ip_header_len]);
    packet[10..12].copy_from_slice(&csum.to_be_bytes());
}

/// Recalcula o checksum de TCP/UDP do zero, com o pseudo-cabeçalho.
///
/// Necessário quando a placa de rede faria o cálculo (checksum offload): o
/// pacote que o WinDivert entrega na saída traz só o parcial do pseudo-
/// cabeçalho, e ele vai para o túnel sem passar pela placa.
pub fn recompute_transport_checksum(packet: &mut [u8], headers: &Headers) {
    let offset = headers.ip_header_len
        + match headers.proto {
            Proto::Tcp => 16,
            Proto::Udp => 6,
        };
    packet[offset] = 0;
    packet[offset + 1] = 0;

    let transport = &packet[headers.ip_header_len..headers.total_len];
    let mut pseudo = [0u8; 12];
    pseudo[..4].copy_from_slice(&headers.src.octets());
    pseudo[4..8].copy_from_slice(&headers.dst.octets());
    pseudo[9] = match headers.proto {
        Proto::Tcp => 6,
        Proto::Udp => 17,
    };
    pseudo[10..12].copy_from_slice(&(transport.len() as u16).to_be_bytes());

    let mut csum = !fold(sum16(&pseudo) + sum16(transport));
    // Em UDP, 0 significa "sem checksum"; o valor calculado 0 é enviado como 0xffff.
    if headers.proto == Proto::Udp && csum == 0 {
        csum = 0xffff;
    }
    packet[offset..offset + 2].copy_from_slice(&csum.to_be_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Monta um pacote IPv4 + TCP/UDP com checksums corretos, por cálculo completo.
    fn build(proto: Proto, src: Ipv4Addr, dst: Ipv4Addr, tcp_flags: u8, options: &[u8], payload: &[u8]) -> Vec<u8> {
        let transport_len = match proto {
            Proto::Tcp => 20 + options.len(),
            Proto::Udp => 8,
        } + payload.len();
        let total = 20 + transport_len;
        let mut p = vec![0u8; total];
        p[0] = 0x45;
        p[2..4].copy_from_slice(&(total as u16).to_be_bytes());
        p[6] = 0x40; // DF
        p[8] = 64;
        p[9] = match proto { Proto::Tcp => 6, Proto::Udp => 17 };
        p[12..16].copy_from_slice(&src.octets());
        p[16..20].copy_from_slice(&dst.octets());
        let ipc = checksum(&p[..20]);
        p[10..12].copy_from_slice(&ipc.to_be_bytes());

        let t = 20;
        p[t..t + 2].copy_from_slice(&50000u16.to_be_bytes());
        p[t + 2..t + 4].copy_from_slice(&443u16.to_be_bytes());
        match proto {
            Proto::Tcp => {
                p[t + 12] = (((20 + options.len()) / 4) as u8) << 4;
                p[t + 13] = tcp_flags;
                p[t + 20..t + 20 + options.len()].copy_from_slice(options);
                p[t + 20 + options.len()..].copy_from_slice(payload);
            }
            Proto::Udp => {
                p[t + 4..t + 6].copy_from_slice(&(transport_len as u16).to_be_bytes());
                p[t + 8..].copy_from_slice(payload);
            }
        }
        let tc = transport_checksum(&p);
        let off = t + match proto { Proto::Tcp => 16, Proto::Udp => 6 };
        p[off..off + 2].copy_from_slice(&tc.to_be_bytes());
        p
    }

    /// Checksum de transporte completo, por um caminho independente do código
    /// de produção (o pseudo-cabeçalho é montado aqui de novo).
    fn transport_checksum(p: &[u8]) -> u16 {
        let h = parse(p).unwrap();
        let mut copy = p.to_vec();
        let off = h.ip_header_len + match h.proto { Proto::Tcp => 16, Proto::Udp => 6 };
        copy[off] = 0;
        copy[off + 1] = 0;
        let transport = &copy[h.ip_header_len..h.total_len];
        let mut pseudo = Vec::new();
        pseudo.extend_from_slice(&h.src.octets());
        pseudo.extend_from_slice(&h.dst.octets());
        pseudo.push(0);
        pseudo.push(p[9]);
        pseudo.extend_from_slice(&(transport.len() as u16).to_be_bytes());
        let sum = sum16(&pseudo) + sum16(transport);
        !fold(sum)
    }

    #[test]
    fn full_recompute_repairs_offloaded_checksums() {
        for proto in [Proto::Tcp, Proto::Udp] {
            let mut p = build(proto, LAN, REMOTE, 0x18, &[], b"offloaded by the nic");
            let h = parse(&p).unwrap();
            // Simula o que a placa receberia: checksums lixo.
            p[10] = 0xde;
            p[11] = 0xad;
            let off = h.ip_header_len + match proto { Proto::Tcp => 16, Proto::Udp => 6 };
            p[off] = 0xbe;
            p[off + 1] = 0xef;
            recompute_ip_checksum(&mut p, &h);
            recompute_transport_checksum(&mut p, &h);
            assert!(ip_checksum_ok(&p), "{proto:?}: IP");
            assert!(transport_checksum_ok(&p), "{proto:?}: transporte");
        }
    }

    fn ip_checksum_ok(p: &[u8]) -> bool {
        fold(sum16(&p[..20])) == 0xffff
    }

    fn transport_checksum_ok(p: &[u8]) -> bool {
        let h = parse(p).unwrap();
        let off = h.ip_header_len + match h.proto { Proto::Tcp => 16, Proto::Udp => 6 };
        let stored = u16::from_be_bytes([p[off], p[off + 1]]);
        stored == transport_checksum(p)
    }

    const LAN: Ipv4Addr = Ipv4Addr::new(192, 168, 0, 23);
    const TUN: Ipv4Addr = Ipv4Addr::new(10, 66, 0, 2);
    const REMOTE: Ipv4Addr = Ipv4Addr::new(162, 159, 128, 233);

    #[test]
    fn parses_ports_and_addresses() {
        let p = build(Proto::Udp, LAN, REMOTE, 0, &[], b"hello");
        let h = parse(&p).unwrap();
        assert_eq!(h.proto, Proto::Udp);
        assert_eq!((h.src, h.dst), (LAN, REMOTE));
        assert_eq!((h.src_port, h.dst_port), (50000, 443));
        assert_eq!(h.total_len, p.len());
    }

    #[test]
    fn rejects_what_is_not_ipv4_tcp_udp() {
        assert!(parse(&[0x60; 40]).is_none()); // IPv6
        let mut icmp = build(Proto::Udp, LAN, REMOTE, 0, &[], b"");
        icmp[9] = 1;
        assert!(parse(&icmp).is_none());
        let mut frag = build(Proto::Udp, LAN, REMOTE, 0, &[], b"x");
        frag[7] = 0x10; // offset != 0
        assert!(parse(&frag).is_none());
        assert!(parse(&build(Proto::Udp, LAN, REMOTE, 0, &[], b"")[..15]).is_none());
    }

    #[test]
    fn source_rewrite_keeps_both_checksums_valid() {
        for proto in [Proto::Tcp, Proto::Udp] {
            let mut p = build(proto, LAN, REMOTE, 0x18, &[], b"payload bytes here");
            let h = parse(&p).unwrap();
            rewrite_source(&mut p, &h, TUN);
            assert_eq!(parse(&p).unwrap().src, TUN);
            assert!(ip_checksum_ok(&p), "{proto:?}: IP");
            assert!(transport_checksum_ok(&p), "{proto:?}: transporte");
        }
    }

    #[test]
    fn destination_rewrite_undoes_the_nat() {
        let original = build(Proto::Udp, REMOTE, LAN, 0, &[], b"reply");
        let mut p = build(Proto::Udp, REMOTE, TUN, 0, &[], b"reply");
        let h = parse(&p).unwrap();
        rewrite_destination(&mut p, &h, LAN);
        assert_eq!(p, original);
    }

    #[test]
    fn udp_without_checksum_stays_without_checksum() {
        let mut p = build(Proto::Udp, LAN, REMOTE, 0, &[], b"x");
        p[26] = 0;
        p[27] = 0;
        let h = parse(&p).unwrap();
        rewrite_source(&mut p, &h, TUN);
        assert_eq!(&p[26..28], &[0, 0]);
        assert!(ip_checksum_ok(&p));
    }

    #[test]
    fn mss_is_clamped_only_on_syn_and_only_downwards() {
        // MSS 1460, NOP, NOP, SACK permitted.
        let options = [2, 4, 0x05, 0xb4, 1, 1, 4, 2];
        let mut syn = build(Proto::Tcp, LAN, REMOTE, 0x02, &options, b"");
        let h = parse(&syn).unwrap();
        assert!(clamp_mss(&mut syn, &h, 1340));
        assert_eq!(&syn[42..44], &1340u16.to_be_bytes());
        assert!(transport_checksum_ok(&syn));
        // Já menor: não mexe.
        assert!(!clamp_mss(&mut syn, &h, 1340));

        let mut ack = build(Proto::Tcp, LAN, REMOTE, 0x10, &options, b"");
        let h = parse(&ack).unwrap();
        assert!(!clamp_mss(&mut ack, &h, 1340));
        assert_eq!(&ack[42..44], &[0x05, 0xb4]);
    }

    #[test]
    fn public_means_routable_on_the_internet() {
        assert!(is_public(REMOTE));
        assert!(is_public(Ipv4Addr::new(1, 1, 1, 1)));
        for local in [
            Ipv4Addr::new(192, 168, 0, 1),
            Ipv4Addr::new(10, 66, 0, 1),
            Ipv4Addr::new(172, 16, 5, 5),
            Ipv4Addr::new(100, 64, 0, 1),
            Ipv4Addr::new(127, 0, 0, 1),
            Ipv4Addr::new(169, 254, 1, 1),
            Ipv4Addr::new(224, 0, 0, 251),
            Ipv4Addr::new(255, 255, 255, 255),
            Ipv4Addr::new(0, 0, 0, 0),
        ] {
            assert!(!is_public(local), "{local}");
        }
    }

    #[test]
    fn incremental_update_matches_full_recompute() {
        let mut header = build(Proto::Udp, LAN, REMOTE, 0, &[], b"")[..20].to_vec();
        let stored = u16::from_be_bytes([header[10], header[11]]);
        let new = TUN.octets();
        let updated = update_checksum(stored, &LAN.octets(), &new);
        header[12..16].copy_from_slice(&new);
        header[10] = 0;
        header[11] = 0;
        assert_eq!(updated, checksum(&header));
    }
}
