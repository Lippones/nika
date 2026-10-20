//! O engine: WinDivert ⇄ NAT ⇄ WireGuard (RF-52 a RF-55, RF-58).
//!
//! Três threads de sistema e uma tarefa tokio, ligadas por canais:
//!
//! ```text
//! divert-net   ─ recv NETWORK ─▶ é do Discord? ─não─▶ devolve intacto
//!                                     │sim
//!                                     ▼ NAT origem, MSS, checksums
//!                                outbound_tx ──▶ [wg::Link] ──▶ UDP endpoint
//! divert-inject ◀─ inbound_rx ◀──────────────────┘ (decapsulado)
//!      │ desfaz NAT destino
//!      ▼ send NETWORK (entrada, interface de origem)
//! divert-sock  ─ eventos SOCKET ─▶ tabela (proto, porta) → PID
//! ```
//!
//! Tudo que não é do Discord custa um `recv` + um `send` e nenhuma alocação.

use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::sync::{mpsc, watch};

use super::divert::{self, Address, Api, Handle};
use super::owner;
use super::packet::{self, Headers, Proto};
use super::profile::Profile;
use super::wg::{Link, LinkStats};
use super::EngineStats;
use crate::error::{Error, Result};

/// Pacotes IP em trânsito entre as threads e a tarefa WireGuard. Cheio =
/// descartar, como faria uma fila de placa de rede.
const QUEUE: usize = 1024;
/// Buffer de recepção: o maior pacote IPv4 possível mais folga.
const RECV_BUFFER: usize = 65_535 + 64;
/// Quanto tempo uma consulta negativa (porta sem dono conhecido) vale.
const NEGATIVE_TTL: Duration = Duration::from_secs(2);
/// Entrada de NAT sem tráfego há tanto tempo some.
const NAT_IDLE: Duration = Duration::from_secs(600);

#[derive(Debug, Clone, Copy)]
struct NatEntry {
    local_ip: Ipv4Addr,
    if_idx: u32,
    sub_if_idx: u32,
    last_seen: Instant,
}

#[derive(Default)]
struct State {
    /// Toda porta local com dono conhecido (não só do Discord): assim uma
    /// porta de outro processo nunca dispara a varredura cara.
    ports: HashMap<(Proto, u16), u32>,
    /// PID → é Discord. Cache do `QueryFullProcessImageName`.
    pids: HashMap<u32, bool>,
    /// Portas que a varredura não achou, com o instante da consulta.
    negative: HashMap<(Proto, u16), Instant>,
    nat: HashMap<(Proto, u16), NatEntry>,
    last_prune: Option<Instant>,
}

struct Table {
    state: Mutex<State>,
}

impl Table {
    fn new() -> Self {
        let mut state = State::default();
        for owner in owner::snapshot() {
            state.ports.insert((owner.proto, owner.port), owner.pid);
        }
        log::info!("túnel: {} portas com dono na partida", state.ports.len());
        Self {
            state: Mutex::new(state),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().expect("table mutex")
    }

    /// RF-52: a porta local de um pacote de saída pertence ao Discord?
    fn is_discord_port(&self, proto: Proto, port: u16) -> bool {
        let key = (proto, port);
        let mut state = self.lock();

        let pid = match state.ports.get(&key) {
            Some(pid) => *pid,
            None => {
                if let Some(asked) = state.negative.get(&key) {
                    if asked.elapsed() < NEGATIVE_TTL {
                        return false;
                    }
                }
                // Corrida com o evento SOCKET, ou socket anterior ao túnel.
                match owner::lookup(proto, port) {
                    Some(pid) => {
                        state.ports.insert(key, pid);
                        state.negative.remove(&key);
                        pid
                    }
                    None => {
                        state.negative.insert(key, Instant::now());
                        return false;
                    }
                }
            }
        };

        match state.pids.get(&pid) {
            Some(known) => *known,
            None => {
                let discord = owner::is_discord(pid);
                if discord {
                    log::info!("túnel: processo {pid} é o Discord");
                }
                state.pids.insert(pid, discord);
                discord
            }
        }
    }

    fn port_opened(&self, proto: Proto, port: u16, pid: u32) {
        let mut state = self.lock();
        state.ports.insert((proto, port), pid);
        state.negative.remove(&(proto, port));
    }

    fn port_closed(&self, proto: Proto, port: u16, pid: u32) {
        let mut state = self.lock();
        // Só se ainda for o mesmo dono: a porta pode já ter sido reusada.
        if state.ports.get(&(proto, port)) == Some(&pid) {
            state.ports.remove(&(proto, port));
            state.nat.remove(&(proto, port));
        }
    }

    fn nat_record(
        &self,
        proto: Proto,
        port: u16,
        local_ip: Ipv4Addr,
        if_idx: u32,
        sub_if_idx: u32,
    ) {
        let mut state = self.lock();
        let now = Instant::now();
        let fresh = state
            .nat
            .insert(
                (proto, port),
                NatEntry {
                    local_ip,
                    if_idx,
                    sub_if_idx,
                    last_seen: now,
                },
            )
            .is_none();
        if fresh {
            log::debug!("túnel: novo fluxo {proto:?}/{port} de {local_ip}");
        }

        // Faxina barata, no máximo uma vez por minuto.
        if state
            .last_prune
            .map_or(true, |t| now.duration_since(t) > Duration::from_secs(60))
        {
            state
                .nat
                .retain(|_, e| now.duration_since(e.last_seen) < NAT_IDLE);
            state
                .negative
                .retain(|_, t| now.duration_since(*t) < NEGATIVE_TTL);
            state.last_prune = Some(now);
        }
    }

    fn nat_lookup(&self, proto: Proto, port: u16) -> Option<NatEntry> {
        let mut state = self.lock();
        let entry = state.nat.get_mut(&(proto, port))?;
        entry.last_seen = Instant::now();
        Some(*entry)
    }

    fn flows(&self) -> u32 {
        self.lock().nat.len() as u32
    }
}

pub struct Engine {
    network: Arc<Handle>,
    socket: Arc<Handle>,
    table: Arc<Table>,
    stop: watch::Sender<bool>,
    link_task: Option<tauri::async_runtime::JoinHandle<Result<()>>>,
    threads: Vec<std::thread::JoinHandle<()>>,
    link_stats: Arc<Mutex<LinkStats>>,
    /// Primeiro erro fatal de alguma thread; `stats()` o devolve.
    failure: Arc<Mutex<Option<String>>>,
}

impl Engine {
    pub async fn start(profile: Profile, driver_dir: &Path) -> Result<Self> {
        let api = Api::load(driver_dir)?;

        // O socket WireGuard primeiro: a porta dele entra no filtro, para os
        // nossos datagramas não voltarem para dentro do túnel.
        let link = Link::bind(&profile).await?;
        let wg_port = link.local_port()?;
        let link_stats = link.stats_handle();

        let network_filter =
            format!("outbound and !loopback and (tcp or (udp and udp.SrcPort != {wg_port}))");
        let network = Arc::new(api.open(&network_filter, divert::LAYER_NETWORK, 0)?);
        let socket = Arc::new(api.open(
            "tcp or udp",
            divert::LAYER_SOCKET,
            divert::FLAG_SNIFF | divert::FLAG_RECV_ONLY,
        )?);

        let table = Arc::new(Table::new());
        let failure = Arc::new(Mutex::new(None));
        let (stop, stop_rx) = watch::channel(false);
        let (outbound_tx, outbound_rx) = mpsc::channel::<Vec<u8>>(QUEUE);
        let (inbound_tx, inbound_rx) = mpsc::channel::<Vec<u8>>(QUEUE);

        let link_task = tauri::async_runtime::spawn(link.run(outbound_rx, inbound_tx, stop_rx));

        let mut threads = Vec::with_capacity(3);
        threads.push(spawn_thread("divert-net", {
            let network = network.clone();
            let table = table.clone();
            let failure = failure.clone();
            let tunnel_ip = profile.address;
            let mtu = profile.mtu;
            move || network_loop(&network, &table, outbound_tx, tunnel_ip, mtu, &failure)
        }));
        threads.push(spawn_thread("divert-sock", {
            let socket = socket.clone();
            let table = table.clone();
            move || socket_loop(&socket, &table)
        }));
        threads.push(spawn_thread("divert-inject", {
            let network = network.clone();
            let table = table.clone();
            move || inject_loop(&network, &table, inbound_rx)
        }));

        log::info!(
            "túnel ligado: Discord → {} (origem {}, MTU {})",
            profile.endpoint,
            profile.address,
            profile.mtu
        );

        Ok(Self {
            network,
            socket,
            table,
            stop,
            link_task: Some(link_task),
            threads,
            link_stats,
            failure,
        })
    }

    pub fn stats(&self) -> Result<EngineStats> {
        if let Some(message) = self.failure.lock().expect("failure mutex").clone() {
            return Err(Error::Divert(message));
        }
        Ok(EngineStats {
            link: *self.link_stats.lock().expect("stats mutex"),
            flows: self.table.flows(),
        })
    }

    /// RF-58. Ordem: parar a criptografia, acordar os `recv`, esperar as
    /// threads, fechar os handles (o Drop). O último handle descarrega o driver.
    pub async fn stop(mut self) {
        let _ = self.stop.send(true);
        self.network.shutdown();
        self.socket.shutdown();

        if let Some(task) = self.link_task.take() {
            let _ = tokio::time::timeout(Duration::from_secs(3), task).await;
        }

        let threads = std::mem::take(&mut self.threads);
        let _ = tokio::task::spawn_blocking(move || {
            for thread in threads {
                let _ = thread.join();
            }
        })
        .await;

        log::info!("túnel desligado");
    }
}

fn spawn_thread(name: &str, body: impl FnOnce() + Send + 'static) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name(name.to_string())
        .spawn(body)
        .expect("criar thread do túnel")
}

// ============================ Saída ========================================

enum Verdict {
    /// Não é nosso: devolver ao kernel como veio.
    PassThrough,
    /// Foi para o túnel, ou descartado de propósito (IPv6, maior que o MTU).
    Consumed,
}

fn network_loop(
    network: &Handle,
    table: &Table,
    outbound: mpsc::Sender<Vec<u8>>,
    tunnel_ip: Ipv4Addr,
    mtu: u16,
    failure: &Mutex<Option<String>>,
) {
    let mut buf = vec![0u8; RECV_BUFFER];
    let mut oversize_logged = Instant::now() - Duration::from_secs(3600);

    loop {
        let (len, addr) = match network.recv(&mut buf) {
            Ok(Some(pair)) => pair,
            Ok(None) => return,
            Err(err) => {
                *failure.lock().expect("failure mutex") = Some(err.to_string());
                return;
            }
        };
        if len == 0 {
            continue;
        }
        let packet = &mut buf[..len];

        let verdict = classify_outbound(
            packet,
            &addr,
            table,
            &outbound,
            tunnel_ip,
            mtu,
            &mut oversize_logged,
        );
        if let Verdict::PassThrough = verdict {
            network.send(packet, &addr);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn classify_outbound(
    packet: &mut [u8],
    addr: &Address,
    table: &Table,
    outbound: &mpsc::Sender<Vec<u8>>,
    tunnel_ip: Ipv4Addr,
    mtu: u16,
    oversize_logged: &mut Instant,
) -> Verdict {
    if addr.loopback() || !addr.outbound() {
        return Verdict::PassThrough;
    }

    // T-06: IPv6 do Discord some; o Chromium cai para IPv4, que vai pelo túnel.
    if addr.ipv6() {
        return match ipv6_source(packet) {
            Some((proto, port)) if table.is_discord_port(proto, port) => Verdict::Consumed,
            _ => Verdict::PassThrough,
        };
    }

    let Some(headers) = packet::parse(packet) else {
        return Verdict::PassThrough;
    };
    if !table.is_discord_port(headers.proto, headers.src_port) {
        return Verdict::PassThrough;
    }
    // T-05: LAN, DNS do roteador, multicast… seguem pela rede local.
    if !packet::is_public(headers.dst) {
        return Verdict::PassThrough;
    }
    if headers.total_len > usize::from(mtu) {
        if oversize_logged.elapsed() > Duration::from_secs(60) {
            log::warn!(
                "túnel: pacote {:?} de {} bytes maior que o MTU {mtu}; descartado",
                headers.proto,
                headers.total_len
            );
            *oversize_logged = Instant::now();
        }
        return Verdict::Consumed;
    }

    let (if_idx, sub_if_idx) = addr.network();
    table.nat_record(
        headers.proto,
        headers.src_port,
        headers.src,
        if_idx,
        sub_if_idx,
    );

    let body = &mut packet[..headers.total_len];
    packet::clamp_mss(body, &headers, mtu.saturating_sub(40));
    packet::rewrite_source(body, &headers, tunnel_ip);

    // Offload: a placa faria o checksum, mas o pacote não vai passar por ela.
    let transport_valid = match headers.proto {
        Proto::Tcp => addr.tcp_checksum_valid(),
        Proto::Udp => addr.udp_checksum_valid(),
    };
    if !addr.ip_checksum_valid() || !transport_valid {
        let rewritten = Headers {
            src: tunnel_ip,
            ..headers
        };
        if !addr.ip_checksum_valid() {
            packet::recompute_ip_checksum(body, &rewritten);
        }
        if !transport_valid {
            packet::recompute_transport_checksum(body, &rewritten);
        }
    }

    // Fila cheia: a tarefa WireGuard não acompanha; descartar é o certo.
    let _ = outbound.try_send(body.to_vec());
    Verdict::Consumed
}

/// `(proto, porta de origem)` de um IPv6 sem cabeçalhos de extensão. Com
/// extensão, `None` — e o pacote passa, o que é o lado seguro de errar.
fn ipv6_source(packet: &[u8]) -> Option<(Proto, u16)> {
    if packet.len() < 44 || packet[0] >> 4 != 6 {
        return None;
    }
    let proto = match packet[6] {
        6 => Proto::Tcp,
        17 => Proto::Udp,
        _ => return None,
    };
    Some((proto, u16::from_be_bytes([packet[40], packet[41]])))
}

// ============================ Entrada ======================================

fn inject_loop(network: &Handle, table: &Table, mut inbound: mpsc::Receiver<Vec<u8>>) {
    while let Some(mut packet) = inbound.blocking_recv() {
        let Some(headers) = packet::parse(&packet) else {
            continue;
        };
        let Some(entry) = table.nat_lookup(headers.proto, headers.dst_port) else {
            // Resposta para uma porta que o Discord já fechou.
            continue;
        };
        packet::rewrite_destination(&mut packet, &headers, entry.local_ip);
        let addr = Address::inbound(entry.if_idx, entry.sub_if_idx);
        network.send(&packet[..headers.total_len], &addr);
    }
}

// ============================ Sockets ======================================

fn socket_loop(socket: &Handle, table: &Table) {
    loop {
        let addr = match socket.recv_event() {
            Ok(Some(addr)) => addr,
            Ok(None) => return,
            Err(err) => {
                // Sem eventos, o fallback por tabela ainda funciona; não é fatal.
                log::warn!("túnel: eventos de socket indisponíveis: {err}");
                return;
            }
        };
        let event = addr.socket();
        let proto = match event.protocol {
            6 => Proto::Tcp,
            17 => Proto::Udp,
            _ => continue,
        };
        match addr.event() {
            divert::event::SOCKET_BIND
            | divert::event::SOCKET_CONNECT
            | divert::event::SOCKET_LISTEN
            | divert::event::SOCKET_ACCEPT => {
                table.port_opened(proto, event.local_port, event.process_id)
            }
            divert::event::SOCKET_CLOSE => {
                table.port_closed(proto, event.local_port, event.process_id)
            }
            _ => {}
        }
    }
}
