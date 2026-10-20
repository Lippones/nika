//! Perfil WireGuard: o `.conf` que o `nika-peer` gera no servidor (RF-50).
//!
//! Formato INI de duas seções. Aceitamos o subconjunto que o cliente oficial
//! aceita e que faz sentido aqui; chaves desconhecidas são ignoradas, não
//! recusadas, para que um `.conf` exportado de outro lugar também sirva.
//!
//! Linhas de comentário `# Nika-Server:` e `# Nika-Peer:` são o rótulo do
//! servidor e o nome de quem recebeu o convite; o `nika-peer` as escreve e a UI
//! as mostra ("convite para joao · servidor do Filipe"). O mesmo conteúdo
//! também viaja como **convite de uma linha**, `nika://convite/<base64url>`,
//! que não quebra numa mensagem de Discord/WhatsApp — ver [`decode_invite`].
//!
//! ```ini
//! # Nika-Server: Filipe · Santiago
//! # Nika-Peer: joao
//! [Interface]
//! PrivateKey = <base64>
//! Address = 10.66.0.2/32, fd4e:696b:61::2/128
//! DNS = 1.1.1.1
//! MTU = 1380
//!
//! [Peer]
//! PublicKey = <base64>
//! PresharedKey = <base64>
//! Endpoint = 1.2.3.4:51820
//! AllowedIPs = 0.0.0.0/0, ::/0
//! PersistentKeepalive = 25
//! ```

use std::fmt;
use std::net::{Ipv4Addr, SocketAddr, ToSocketAddrs};

use base64::Engine;
use serde::Serialize;

use crate::error::{Error, Result};

/// MTU do túnel quando o perfil não diz. O `install.sh` fixa 1380 no servidor
/// e o `nika-peer` escreve o mesmo valor no perfil; este é só o fallback.
pub const DEFAULT_MTU: u16 = 1380;

/// Prefixo do convite de uma linha.
pub const INVITE_PREFIX: &str = "nika://convite/";

/// Converte o que a pessoa colou no texto do `.conf`: um convite de uma linha
/// é decodificado; um `.conf` passa direto. Espaços e quebras em volta são
/// tolerados, porque é assim que texto chega de uma mensagem.
pub fn decode_invite(text: &str) -> Result<String> {
    let trimmed = text.trim();
    let Some(payload) = trimmed.strip_prefix(INVITE_PREFIX) else {
        return Ok(trimmed.to_string());
    };
    let payload: String = payload.chars().filter(|c| !c.is_whitespace()).collect();
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload.trim_end_matches('='))
        .map_err(|_| profile_error("o convite está incompleto ou foi alterado ao copiar"))?;
    String::from_utf8(bytes)
        .map_err(|_| profile_error("o convite está incompleto ou foi alterado ao copiar"))
}

/// O inverso de [`decode_invite`]: o que o `nika-peer` imprime, reproduzido
/// aqui para os testes provarem que os dois lados concordam.
#[cfg(test)]
pub fn encode_invite(conf: &str) -> String {
    format!(
        "{INVITE_PREFIX}{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(conf.trim())
    )
}

/// Uma chave de 32 bytes (privada, pública ou pré-compartilhada).
#[derive(Clone, PartialEq, Eq)]
pub struct Key(pub [u8; 32]);

impl Key {
    fn parse(value: &str, field: &str) -> Result<Self> {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(value.trim())
            .map_err(|_| profile_error(format!("{field} não é base64 válido")))?;
        let bytes: [u8; 32] = bytes
            .try_into()
            .map_err(|_| profile_error(format!("{field} precisa ter 32 bytes")))?;
        Ok(Self(bytes))
    }

    pub fn to_base64(&self) -> String {
        base64::engine::general_purpose::STANDARD.encode(self.0)
    }
}

/// Nunca imprimir o valor: a chave privada passa por aqui.
impl fmt::Debug for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Key(…)")
    }
}

#[derive(Debug, Clone)]
pub struct Profile {
    pub private_key: Key,
    pub peer_public_key: Key,
    pub preshared_key: Option<Key>,
    /// Endereço IPv4 do túnel — a origem que os pacotes do Discord passam a ter.
    pub address: Ipv4Addr,
    pub endpoint: SocketAddr,
    pub mtu: u16,
    pub persistent_keepalive: Option<u16>,
    /// `# Nika-Server:` — como o dono chamou a saída ("Filipe · Santiago").
    pub server_label: Option<String>,
    /// `# Nika-Peer:` — para quem este convite foi gerado.
    pub peer_name: Option<String>,
}

/// O que a UI pode ver do perfil: nada que seja segredo.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSummary {
    pub address: String,
    pub endpoint: String,
    pub peer_public_key: String,
    pub mtu: u16,
    pub server_label: Option<String>,
    pub peer_name: Option<String>,
}

impl Profile {
    pub fn summary(&self) -> ProfileSummary {
        ProfileSummary {
            address: self.address.to_string(),
            endpoint: self.endpoint.to_string(),
            peer_public_key: self.peer_public_key.to_base64(),
            mtu: self.mtu,
            server_label: self.server_label.clone(),
            peer_name: self.peer_name.clone(),
        }
    }

    /// Parse e validação. Erros são frases para a UI, apontando o campo.
    pub fn parse(text: &str) -> Result<Self> {
        let mut section = String::new();
        let mut private_key = None;
        let mut peer_public_key = None;
        let mut preshared_key = None;
        let mut address = None;
        let mut endpoint = None;
        let mut mtu = None;
        let mut keepalive = None;
        let mut server_label = None;
        let mut peer_name = None;

        for raw in text.lines() {
            let line = raw.trim();
            if let Some(comment) = line.strip_prefix('#') {
                // Só os dois rótulos do Nika; qualquer outro comentário é ignorado.
                if let Some((key, value)) = comment.split_once(':') {
                    let value = value.trim();
                    match key.trim().to_ascii_lowercase().as_str() {
                        "nika-server" if !value.is_empty() => server_label = Some(clip(value)),
                        "nika-peer" if !value.is_empty() => peer_name = Some(clip(value)),
                        _ => {}
                    }
                }
                continue;
            }
            if line.is_empty() || line.starts_with(';') {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                section = line[1..line.len() - 1].trim().to_ascii_lowercase();
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                return Err(profile_error(format!("linha sem `=`: {line}")));
            };
            let key = key.trim().to_ascii_lowercase();
            let value = value.trim();

            match (section.as_str(), key.as_str()) {
                ("interface", "privatekey") => private_key = Some(Key::parse(value, "PrivateKey")?),
                ("interface", "address") => address = Some(parse_address(value)?),
                ("interface", "mtu") => {
                    mtu = Some(
                        value
                            .parse::<u16>()
                            .ok()
                            .filter(|m| (576..=9000).contains(m))
                            .ok_or_else(|| {
                                profile_error("MTU precisa ser um número entre 576 e 9000")
                            })?,
                    );
                }
                ("peer", "publickey") => peer_public_key = Some(Key::parse(value, "PublicKey")?),
                ("peer", "presharedkey") => {
                    preshared_key = Some(Key::parse(value, "PresharedKey")?)
                }
                ("peer", "endpoint") => endpoint = Some(parse_endpoint(value)?),
                ("peer", "persistentkeepalive") => {
                    let secs = value
                        .parse::<u16>()
                        .map_err(|_| profile_error("PersistentKeepalive precisa ser um número"))?;
                    keepalive = (secs > 0).then_some(secs);
                }
                // DNS, AllowedIPs, Table, ListenPort, FwMark…: o Nika decide as
                // rotas pelo processo, não por IP, e não configura DNS.
                _ => {}
            }
        }

        Ok(Self {
            private_key: private_key
                .ok_or_else(|| profile_error("falta PrivateKey em [Interface]"))?,
            peer_public_key: peer_public_key
                .ok_or_else(|| profile_error("falta PublicKey em [Peer]"))?,
            preshared_key,
            address: address.ok_or_else(|| profile_error("falta Address em [Interface]"))?,
            endpoint: endpoint.ok_or_else(|| profile_error("falta Endpoint em [Peer]"))?,
            mtu: mtu.unwrap_or(DEFAULT_MTU),
            persistent_keepalive: keepalive,
            server_label,
            peer_name,
        })
    }
}

/// Rótulos vêm de quem gerou o convite; a UI tem uma linha para mostrá-los.
fn clip(value: &str) -> String {
    value.chars().take(48).collect()
}

/// `Address` pode listar vários; precisamos de exatamente um IPv4, e o resto
/// (o IPv6 que o `nika-peer` inclui) é ignorado — IPv6 não entra no túnel.
fn parse_address(value: &str) -> Result<Ipv4Addr> {
    value
        .split(',')
        .map(str::trim)
        .filter_map(|entry| entry.split('/').next())
        .find_map(|ip| ip.trim().parse::<Ipv4Addr>().ok())
        .ok_or_else(|| profile_error("Address precisa ter um endereço IPv4 (ex.: 10.66.0.2/32)"))
}

/// `host:porta`. Um nome DNS é resolvido agora, uma vez; o `nika-peer` sempre
/// escreve IP, então na prática isto só valida.
fn parse_endpoint(value: &str) -> Result<SocketAddr> {
    if let Ok(addr) = value.parse::<SocketAddr>() {
        return Ok(addr);
    }
    let (host, port) = value
        .rsplit_once(':')
        .ok_or_else(|| profile_error("Endpoint precisa ser host:porta"))?;
    port.parse::<u16>()
        .map_err(|_| profile_error("porta do Endpoint inválida"))?;
    let host = host.trim_matches(['[', ']']);
    (host, port.parse::<u16>().unwrap_or(0))
        .to_socket_addrs()
        .ok()
        .and_then(|mut addrs| addrs.find(|a| a.is_ipv4()))
        .ok_or_else(|| profile_error(format!("não consegui resolver o Endpoint {value}")))
}

fn profile_error(message: impl Into<String>) -> Error {
    Error::TunnelProfile(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
# Nika-Server: Filipe · Santiago
# Nika-Peer: joao
[Interface]
PrivateKey = yAnz5TF+lXXJte14tji3zlMNq+hd2rYUIgJBgB3fBmk=
Address = 10.66.0.2/32, fd4e:696b:61::2/128
DNS = 1.1.1.1
MTU = 1380

[Peer]
PublicKey = xTIBA5rboUvnH4htodjb6e697QjLERt1NAB4mZqp8Dg=
PresharedKey = FpCyhws9cxwWoV4xELtfJvjJN+zQVRPISllRWgeopVE=
Endpoint = 203.0.113.9:51820
AllowedIPs = 0.0.0.0/0, ::/0
PersistentKeepalive = 25
";

    #[test]
    fn parses_the_profile_nika_peer_writes() {
        let profile = Profile::parse(SAMPLE).unwrap();
        assert_eq!(profile.address, Ipv4Addr::new(10, 66, 0, 2));
        assert_eq!(profile.endpoint, "203.0.113.9:51820".parse().unwrap());
        assert_eq!(profile.mtu, 1380);
        assert_eq!(profile.persistent_keepalive, Some(25));
        assert!(profile.preshared_key.is_some());
        assert_eq!(
            profile.peer_public_key.to_base64(),
            "xTIBA5rboUvnH4htodjb6e697QjLERt1NAB4mZqp8Dg="
        );
        assert_eq!(profile.server_label.as_deref(), Some("Filipe · Santiago"));
        assert_eq!(profile.peer_name.as_deref(), Some("joao"));
    }

    #[test]
    fn one_line_invite_round_trips_and_tolerates_message_noise() {
        let invite = encode_invite(SAMPLE);
        assert!(invite.starts_with(INVITE_PREFIX));
        assert!(!invite.contains('\n'));
        // Colado com espaços e uma quebra no meio (coisa de app de mensagem).
        let (a, b) = invite.split_at(invite.len() / 2);
        let noisy = format!("  {a}\n{b} \n");
        assert_eq!(decode_invite(&noisy).unwrap(), SAMPLE.trim());
        assert!(Profile::parse(&decode_invite(&noisy).unwrap()).is_ok());
        // Um .conf cru passa direto.
        assert_eq!(decode_invite(SAMPLE).unwrap(), SAMPLE.trim());
        // Convite truncado ao copiar: ou o base64 quebra, ou o .conf sai
        // incompleto; nos dois casos é um erro legível, nunca um perfil torto.
        for cut in [invite.len() - 7, invite.len() - 30, invite.len() - 61] {
            let truncated = &invite[..cut];
            let outcome = decode_invite(truncated).and_then(|conf| Profile::parse(&conf));
            assert!(outcome.is_err(), "convite cortado em {cut} deveria falhar");
        }
    }

    #[test]
    fn defaults_when_optional_fields_are_missing() {
        let text = SAMPLE
            .lines()
            .filter(|l| {
                !l.starts_with("MTU")
                    && !l.starts_with("PersistentKeepalive")
                    && !l.starts_with("PresharedKey")
            })
            .collect::<Vec<_>>()
            .join("\n");
        let profile = Profile::parse(&text).unwrap();
        assert_eq!(profile.mtu, DEFAULT_MTU);
        assert_eq!(profile.persistent_keepalive, None);
        assert!(profile.preshared_key.is_none());
    }

    #[test]
    fn rejects_short_keys_and_missing_fields() {
        let bad_key = SAMPLE.replace("yAnz5TF+lXXJte14tji3zlMNq+hd2rYUIgJBgB3fBmk=", "AAAA");
        assert!(Profile::parse(&bad_key)
            .unwrap_err()
            .to_string()
            .contains("PrivateKey"));

        let no_endpoint = SAMPLE.replace("Endpoint = 203.0.113.9:51820", "");
        assert!(Profile::parse(&no_endpoint)
            .unwrap_err()
            .to_string()
            .contains("Endpoint"));

        let v6_only = SAMPLE.replace(
            "Address = 10.66.0.2/32, fd4e:696b:61::2/128",
            "Address = fd4e::2/128",
        );
        assert!(Profile::parse(&v6_only)
            .unwrap_err()
            .to_string()
            .contains("IPv4"));
    }

    #[test]
    fn the_private_key_never_shows_up_in_debug_or_summary() {
        let profile = Profile::parse(SAMPLE).unwrap();
        let debug = format!("{profile:?}");
        assert!(!debug.contains("yAnz5TF"));
        let summary = serde_json::to_string(&profile.summary()).unwrap();
        assert!(!summary.contains("yAnz5TF"));
        assert!(summary.contains("203.0.113.9:51820"));
    }
}
