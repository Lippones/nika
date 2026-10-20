//! WireGuard em espaço de usuário (T-03, RF-59).
//!
//! Um [`Link`] é um par (socket UDP, `boringtun::Tunn`). Ele recebe pacotes IP
//! crus por um canal, encapsula e manda para o endpoint; o que chega do
//! endpoint é decapsulado e sai por outro canal. Quem produz e consome os
//! pacotes IP (o engine, via WinDivert) não sabe nada de criptografia; este
//! módulo não sabe nada de Windows — e por isso roda e é testado no Linux.

// Fora do Windows só os testes usam este módulo: não há engine para chamá-lo.
#![cfg_attr(not(windows), allow(dead_code))]

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use boringtun::noise::errors::WireGuardError;
use boringtun::noise::{Tunn, TunnResult};
use boringtun::x25519::{PublicKey, StaticSecret};
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, watch};

use super::profile::Profile;
use crate::error::{Error, Result};

/// Com que frequência o boringtun revisa seus timers (keepalive, rekey,
/// retransmissão de handshake). O cliente oficial usa 250 ms também.
const TIMER_TICK: Duration = Duration::from_millis(250);

/// Buffer para um datagrama/pacote. Maior que qualquer MTU; o custo é um
/// alloc por `Link`, não por pacote.
const BUFFER: usize = 65536;

/// Instantâneo do que a UI mostra (RF-51).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LinkStats {
    /// Segundos desde o último handshake completo; `None` = nunca.
    pub last_handshake_secs: Option<u64>,
    pub tx_bytes: u64,
    pub rx_bytes: u64,
}

pub struct Link {
    tunn: Tunn,
    socket: Arc<UdpSocket>,
    endpoint: SocketAddr,
    stats: Arc<Mutex<LinkStats>>,
}

impl Link {
    /// Abre o socket UDP e prepara o túnel. Nada é enviado até [`Link::run`].
    pub async fn bind(profile: &Profile) -> Result<Self> {
        let socket = UdpSocket::bind(if profile.endpoint.is_ipv4() {
            "0.0.0.0:0"
        } else {
            "[::]:0"
        })
        .await?;

        let private = StaticSecret::from(profile.private_key.0);
        let peer = PublicKey::from(profile.peer_public_key.0);
        let tunn = Tunn::new(
            private,
            peer,
            profile.preshared_key.as_ref().map(|key| key.0),
            profile.persistent_keepalive,
            local_index(),
            None,
        );

        Ok(Self {
            tunn,
            socket: Arc::new(socket),
            endpoint: profile.endpoint,
            stats: Arc::new(Mutex::new(LinkStats::default())),
        })
    }

    /// Porta UDP local. O engine a exclui do filtro do WinDivert: os nossos
    /// próprios datagramas WireGuard não podem voltar para dentro do túnel.
    pub fn local_port(&self) -> Result<u16> {
        Ok(self.socket.local_addr()?.port())
    }

    pub fn stats_handle(&self) -> Arc<Mutex<LinkStats>> {
        self.stats.clone()
    }

    /// Laço principal. Termina quando `stop` vira `true` ou quando um dos
    /// canais fecha. Erros de rede transitórios são registrados e seguem;
    /// só erro de socket irrecuperável derruba o laço.
    pub async fn run(
        mut self,
        mut outbound: mpsc::Receiver<Vec<u8>>,
        inbound: mpsc::Sender<Vec<u8>>,
        mut stop: watch::Receiver<bool>,
    ) -> Result<()> {
        let mut net_buf = vec![0u8; BUFFER];
        let mut dgram = vec![0u8; BUFFER];
        let mut ticker = tokio::time::interval(TIMER_TICK);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        // Começa o handshake sem esperar o primeiro pacote: a UI vê "Up" antes
        // de o Discord abrir uma conexão, e o `receiver count` do Discord não
        // paga a latência do handshake no primeiro pacote de mídia.
        self.pump_handshake(&mut net_buf).await;

        loop {
            tokio::select! {
                biased;

                changed = stop.changed() => {
                    if changed.is_err() || *stop.borrow() {
                        return Ok(());
                    }
                }

                Some(packet) = outbound.recv() => {
                    self.send_plain(&packet, &mut net_buf).await;
                }

                received = self.socket.recv_from(&mut dgram) => {
                    let (len, from) = match received {
                        Ok(pair) => pair,
                        // No Windows um ICMP "port unreachable" vira erro de
                        // recv (WSAECONNRESET); não é fatal.
                        Err(err) if is_transient(&err) => continue,
                        Err(err) => return Err(err.into()),
                    };
                    self.receive(from, &dgram[..len], &mut net_buf, &inbound).await;
                }

                _ = ticker.tick() => {
                    self.tick(&mut net_buf).await;
                }

                else => return Ok(()),
            }
        }
    }

    /// Um pacote IP do Discord → datagrama WireGuard.
    async fn send_plain(&mut self, packet: &[u8], buf: &mut [u8]) {
        match self.tunn.encapsulate(packet, buf) {
            TunnResult::WriteToNetwork(data) => {
                let data = data.to_vec();
                self.send_to_endpoint(&data).await;
            }
            TunnResult::Err(err) => log::debug!("wg: encapsulate: {err:?}"),
            // Sem sessão o pacote foi enfileirado e um handshake (se já não
            // houver um em curso) é o que `Done` significa aqui.
            _ => {}
        }
    }

    /// Um datagrama do endpoint → zero ou mais pacotes IP, e possivelmente
    /// respostas de handshake/pacotes enfileirados para enviar.
    async fn receive(
        &mut self,
        from: SocketAddr,
        datagram: &[u8],
        buf: &mut [u8],
        inbound: &mpsc::Sender<Vec<u8>>,
    ) {
        let mut result = self.tunn.decapsulate(Some(from.ip()), datagram, buf);
        let mut first = true;
        loop {
            match result {
                TunnResult::Done => break,
                TunnResult::Err(err) => {
                    if !matches!(err, WireGuardError::NoCurrentSession) {
                        log::debug!("wg: decapsulate: {err:?}");
                    }
                    break;
                }
                TunnResult::WriteToNetwork(data) => {
                    // Roaming: um datagrama válido de outro endereço move o
                    // endpoint, como o WireGuard de verdade faz.
                    if first && from != self.endpoint {
                        log::info!("wg: endpoint mudou para {from}");
                        self.endpoint = from;
                    }
                    let data = data.to_vec();
                    self.send_to_endpoint(&data).await;
                    // "Repeat the call with empty datagram until Done".
                    result = self.tunn.decapsulate(None, &[], buf);
                    first = false;
                    continue;
                }
                TunnResult::WriteToTunnelV4(packet, _) => {
                    if first && from != self.endpoint {
                        log::info!("wg: endpoint mudou para {from}");
                        self.endpoint = from;
                    }
                    // Fila cheia = o injetor não dá conta; descartar é o que
                    // uma placa de rede faria.
                    let _ = inbound.try_send(packet.to_vec());
                    break;
                }
                // IPv6 não entra no túnel (T-06); se chegar, some.
                TunnResult::WriteToTunnelV6(..) => break,
            }
        }
        self.refresh_stats();
    }

    async fn tick(&mut self, buf: &mut [u8]) {
        match self.tunn.update_timers(buf) {
            TunnResult::WriteToNetwork(data) => {
                let data = data.to_vec();
                self.send_to_endpoint(&data).await;
            }
            TunnResult::Err(WireGuardError::ConnectionExpired) => {
                // Sem resposta do servidor por muito tempo: o boringtun limpa
                // as sessões e o próximo pacote recomeça o handshake.
                log::warn!("wg: sessão expirou sem resposta do servidor");
            }
            TunnResult::Err(err) => log::debug!("wg: timers: {err:?}"),
            _ => {}
        }
        self.refresh_stats();
    }

    async fn pump_handshake(&mut self, buf: &mut [u8]) {
        if let TunnResult::WriteToNetwork(data) = self.tunn.format_handshake_initiation(buf, false)
        {
            let data = data.to_vec();
            self.send_to_endpoint(&data).await;
        }
    }

    async fn send_to_endpoint(&self, data: &[u8]) {
        if let Err(err) = self.socket.send_to(data, self.endpoint).await {
            if !is_transient(&err) {
                log::warn!("wg: envio para {} falhou: {err}", self.endpoint);
            }
        }
    }

    fn refresh_stats(&self) {
        let (since_handshake, tx, rx, _, _) = self.tunn.stats();
        let mut stats = self.stats.lock().expect("stats mutex");
        stats.last_handshake_secs = since_handshake.map(|d| d.as_secs());
        stats.tx_bytes = tx as u64;
        stats.rx_bytes = rx as u64;
    }
}

/// Índice local do túnel: 24 bits que identificam as nossas sessões para o
/// servidor. Só precisa não repetir entre dois `Link` vivos ao mesmo tempo, e
/// só há um — os nanossegundos do relógio bastam.
fn local_index() -> u32 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(1);
    nanos & 0x00ff_ffff
}

fn is_transient(err: &std::io::Error) -> bool {
    use std::io::ErrorKind;
    matches!(
        err.kind(),
        ErrorKind::ConnectionReset
            | ErrorKind::ConnectionRefused
            | ErrorKind::WouldBlock
            | ErrorKind::Interrupted
            | ErrorKind::NetworkUnreachable
            | ErrorKind::HostUnreachable
    )
}

impl From<WireGuardError> for Error {
    fn from(err: WireGuardError) -> Self {
        Error::other(format!("wireguard: {err:?}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tunnel::profile::Key;
    use std::net::Ipv4Addr;

    fn profile(private: [u8; 32], peer_public: [u8; 32], endpoint: SocketAddr) -> Profile {
        Profile {
            private_key: Key(private),
            peer_public_key: Key(peer_public),
            preshared_key: Some(Key([7u8; 32])),
            address: Ipv4Addr::new(10, 66, 0, 2),
            endpoint,
            mtu: 1380,
            persistent_keepalive: None,
            server_label: None,
            peer_name: None,
        }
    }

    fn ipv4_packet(payload: &[u8]) -> Vec<u8> {
        let total = 20 + 8 + payload.len();
        let mut p = vec![0u8; total];
        p[0] = 0x45;
        p[2..4].copy_from_slice(&(total as u16).to_be_bytes());
        p[8] = 64;
        p[9] = 17;
        p[12..16].copy_from_slice(&[10, 66, 0, 2]);
        p[16..20].copy_from_slice(&[10, 66, 0, 1]);
        p[24..26].copy_from_slice(&((8 + payload.len()) as u16).to_be_bytes());
        p[28..].copy_from_slice(payload);
        p
    }

    /// Dois `Link` em localhost: B inicia, A aprende o endereço de B pelo
    /// roaming, e um pacote IPv4 atravessa nas duas direções.
    #[tokio::test]
    async fn two_links_handshake_and_exchange_a_packet() {
        let a_secret = [0x11u8; 32];
        let b_secret = [0x22u8; 32];
        let a_public = PublicKey::from(&StaticSecret::from(a_secret)).to_bytes();
        let b_public = PublicKey::from(&StaticSecret::from(b_secret)).to_bytes();

        // A ainda não sabe onde B está: endpoint de mentira, corrigido pelo roaming.
        let a = Link::bind(&profile(a_secret, b_public, "127.0.0.1:9".parse().unwrap()))
            .await
            .unwrap();
        let a_addr: SocketAddr = format!("127.0.0.1:{}", a.local_port().unwrap())
            .parse()
            .unwrap();
        let b = Link::bind(&profile(b_secret, a_public, a_addr))
            .await
            .unwrap();

        let (a_out_tx, a_out_rx) = mpsc::channel(16);
        let (a_in_tx, mut a_in_rx) = mpsc::channel(16);
        let (b_out_tx, b_out_rx) = mpsc::channel(16);
        let (b_in_tx, mut b_in_rx) = mpsc::channel(16);
        let (stop_tx, stop_rx) = watch::channel(false);
        let a_stats = a.stats_handle();

        let a_task = tokio::spawn(a.run(a_out_rx, a_in_tx, stop_rx.clone()));
        let b_task = tokio::spawn(b.run(b_out_rx, b_in_tx, stop_rx));

        // B → A. O primeiro pacote espera o handshake dentro do boringtun.
        b_out_tx.send(ipv4_packet(b"ping")).await.unwrap();
        let got = tokio::time::timeout(Duration::from_secs(5), a_in_rx.recv())
            .await
            .expect("A deveria receber em 5 s")
            .unwrap();
        assert_eq!(got, ipv4_packet(b"ping"));

        // A → B, pelo endpoint aprendido.
        a_out_tx.send(ipv4_packet(b"pong")).await.unwrap();
        let got = tokio::time::timeout(Duration::from_secs(5), b_in_rx.recv())
            .await
            .expect("B deveria receber em 5 s")
            .unwrap();
        assert_eq!(got, ipv4_packet(b"pong"));

        let stats = *a_stats.lock().unwrap();
        assert!(stats.last_handshake_secs.is_some());
        assert!(stats.rx_bytes >= 32);

        stop_tx.send(true).unwrap();
        a_task.await.unwrap().unwrap();
        b_task.await.unwrap().unwrap();
    }
}
