//! Conversa com o ControlPort do Tor (§8.3 do PRD).

pub mod client;
pub mod events;
pub mod info;
pub mod protocol;

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::Path;
use std::time::Duration;

use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::time::{sleep, Instant};

pub use client::ControlClient;
pub use events::AsyncEvent;

use crate::error::{Error, Result};

/// Intervalo entre tentativas enquanto o tor ainda está subindo.
const RETRY_INTERVAL: Duration = Duration::from_millis(250);

pub type Connection = (ControlClient, mpsc::Receiver<AsyncEvent>);

/// Conecta, autentica por cookie e assina os eventos de bootstrap.
///
/// Fica tentando até `timeout` porque o ControlPort e o arquivo de cookie só
/// existem alguns instantes depois do processo do tor nascer.
pub async fn establish(control_port: u16, cookie: &Path, timeout: Duration) -> Result<Connection> {
    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), control_port);
    let deadline = Instant::now() + timeout;

    loop {
        match handshake(address, cookie).await {
            Ok(connection) => return Ok(connection),
            Err(err) if Instant::now() >= deadline => {
                log::warn!("desisti de conectar no ControlPort: {err}");
                return Err(Error::ControlTimeout);
            }
            Err(err) => {
                log::debug!("ControlPort ainda não respondeu ({err}); tentando de novo");
                sleep(RETRY_INTERVAL).await;
            }
        }
    }
}

async fn handshake(address: SocketAddr, cookie_path: &Path) -> Result<Connection> {
    // O cookie é gerado pelo tor no DataDirectory; sem ele não há autenticação
    // possível — e nunca usamos senha (RF-12).
    let cookie = tokio::fs::read(cookie_path).await?;
    let stream = TcpStream::connect(address).await?;
    let (client, events) = ControlClient::open(stream).await;

    client
        .send(&format!("AUTHENTICATE {}", to_hex(&cookie)))
        .await?;
    client.send("SETEVENTS STATUS_CLIENT").await?;

    Ok((client, events))
}

fn to_hex(bytes: &[u8]) -> String {
    use std::fmt::Write;

    bytes.iter().fold(String::new(), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_cookie_as_lowercase_hex() {
        assert_eq!(to_hex(&[0x00, 0x0f, 0xa5, 0xff]), "000fa5ff");
    }
}
