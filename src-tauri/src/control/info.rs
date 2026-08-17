//! Consultas de diagnóstico ao ControlPort: circuito atual (RF-14) e
//! contadores de tráfego (RF-16).

use serde::Serialize;

use super::ControlClient;
use crate::error::Result;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Relay {
    pub nickname: String,
    pub fingerprint: String,
    /// Código ISO do país, quando o GeoIP do bundle está disponível.
    pub country: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Circuit {
    pub id: String,
    pub path: Vec<Relay>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Traffic {
    pub read: u64,
    pub written: u64,
}

/// Primeiro circuito de uso geral já construído — é por ele que o tráfego do
/// usuário sai. `None` quando ainda não há nenhum.
pub async fn active_circuit(client: &ControlClient) -> Result<Option<Circuit>> {
    let raw = client.get_info("circuit-status").await?;

    let Some(entry) = raw.lines().map(str::trim).find(|line| is_usable(line)) else {
        return Ok(None);
    };

    let mut fields = entry.split_whitespace();
    let id = fields.next().unwrap_or_default().to_string();
    let _status = fields.next();
    let path_spec = fields.next().unwrap_or_default();

    let mut path = Vec::new();
    for hop in path_spec.split(',').filter(|hop| !hop.is_empty()) {
        let (fingerprint, nickname) = split_hop(hop);
        path.push(Relay {
            country: country_of(client, &fingerprint).await,
            fingerprint,
            nickname,
        });
    }

    Ok(Some(Circuit { id, path }))
}

pub async fn traffic(client: &ControlClient) -> Result<Traffic> {
    Ok(Traffic {
        read: counter(client, "traffic/read").await?,
        written: counter(client, "traffic/written").await?,
    })
}

async fn counter(client: &ControlClient, key: &str) -> Result<u64> {
    Ok(client.get_info(key).await?.trim().parse().unwrap_or(0))
}

/// Circuitos prontos e destinados ao tráfego do usuário.
fn is_usable(line: &str) -> bool {
    let mut fields = line.split_whitespace();
    let _id = fields.next();
    fields.next() == Some("BUILT") && line.contains("PURPOSE=GENERAL")
}

/// Um salto vem como `$FINGERPRINT~Apelido` (ou `=Apelido` para relays
/// nomeados); o apelido é opcional.
fn split_hop(hop: &str) -> (String, String) {
    let hop = hop.trim_start_matches('$');
    match hop.split_once(['~', '=']) {
        Some((fingerprint, nickname)) => (fingerprint.to_string(), nickname.to_string()),
        None => (hop.to_string(), String::from("(sem nome)")),
    }
}

/// País do relay, via IP do consenso. Best-effort: qualquer falha aqui vira
/// `None` em vez de derrubar a consulta inteira.
async fn country_of(client: &ControlClient, fingerprint: &str) -> Option<String> {
    let consensus = client
        .get_info(&format!("ns/id/${fingerprint}"))
        .await
        .ok()?;
    // Linha "r <apelido> <id> <digest> <data> <hora> <ip> <orport> <dirport>"
    let address = consensus
        .lines()
        .find(|line| line.starts_with("r "))?
        .split_whitespace()
        .nth(6)?;

    let country = client
        .get_info(&format!("ip-to-country/{address}"))
        .await
        .ok()?;

    match country.trim() {
        "" | "??" => None,
        code => Some(code.to_uppercase()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_only_built_general_circuits() {
        assert!(is_usable("7 BUILT $AAA~um,$BBB~dois PURPOSE=GENERAL"));
        assert!(!is_usable("8 LAUNCHED $AAA~um PURPOSE=GENERAL"));
        assert!(!is_usable("9 BUILT $AAA~um PURPOSE=HS_CLIENT_INTRO"));
    }

    #[test]
    fn splits_hops() {
        assert_eq!(
            split_hop("$ABC123~Relay1"),
            ("ABC123".to_string(), "Relay1".to_string())
        );
        assert_eq!(
            split_hop("$ABC123"),
            ("ABC123".to_string(), "(sem nome)".to_string())
        );
    }
}
