//! Eventos assíncronos (código 650) do ControlPort.

use super::protocol::{field, Reply};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AsyncEvent {
    /// `STATUS_CLIENT ... BOOTSTRAP PROGRESS=n TAG=t SUMMARY="..."` (RF-11).
    Bootstrap {
        progress: u8,
        tag: String,
        summary: String,
    },
    /// Qualquer outro evento assinado; mantido para o painel de log.
    Other(String),
}

impl AsyncEvent {
    pub fn parse(reply: &Reply) -> Option<Self> {
        Self::from_line(reply.first())
    }

    /// Também serve para o `GETINFO status/bootstrap-phase`, que devolve uma
    /// linha no mesmo formato do evento.
    pub fn from_line(line: &str) -> Option<Self> {
        if line.is_empty() {
            return None;
        }

        if !line.contains("BOOTSTRAP") {
            return Some(Self::Other(line.to_string()));
        }

        let progress = field(line, "PROGRESS")?.parse::<u8>().ok()?;
        Some(Self::Bootstrap {
            progress: progress.min(100),
            tag: field(line, "TAG").unwrap_or_default(),
            summary: field(line, "SUMMARY").unwrap_or_default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reply(line: &str) -> Reply {
        Reply {
            code: 650,
            lines: vec![line.to_string()],
        }
    }

    #[test]
    fn parses_bootstrap_progress() {
        let event = AsyncEvent::parse(&reply(
            r#"STATUS_CLIENT NOTICE BOOTSTRAP PROGRESS=80 TAG=conn_dir SUMMARY="Connecting to a relay""#,
        ));

        assert_eq!(
            event,
            Some(AsyncEvent::Bootstrap {
                progress: 80,
                tag: "conn_dir".into(),
                summary: "Connecting to a relay".into(),
            })
        );
    }

    #[test]
    fn falls_back_to_other() {
        let event = AsyncEvent::parse(&reply("STATUS_CLIENT NOTICE CIRCUIT_ESTABLISHED"));
        assert!(matches!(event, Some(AsyncEvent::Other(_))));
    }
}
