//! Parsing do protocolo do ControlPort do Tor (control-spec).
//!
//! Uma resposta é uma sequência de linhas `<código><sep><texto>`, onde `sep` é:
//! `-` linha intermediária, `+` linha intermediária seguida de bloco de dados
//! terminado por `.`, e ` ` (espaço) última linha da resposta.

use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    pub code: u16,
    /// Uma entrada por linha da resposta; blocos de dados vêm anexados à sua
    /// linha de origem, separados por `\n`.
    pub lines: Vec<String>,
}

impl Reply {
    /// Códigos 2xx são sucesso; qualquer outra coisa vira erro tipado.
    pub fn into_result(self) -> Result<Self> {
        if (200..300).contains(&self.code) {
            return Ok(self);
        }

        Err(Error::ControlRejected {
            code: self.code,
            message: self.lines.join("; "),
        })
    }

    /// Valor de uma linha no formato `chave=valor`, como usado por `GETINFO`.
    pub fn value(&self, key: &str) -> Option<&str> {
        self.lines
            .iter()
            .find_map(|line| line.strip_prefix(key)?.strip_prefix('='))
    }

    pub fn first(&self) -> &str {
        self.lines.first().map(String::as_str).unwrap_or_default()
    }
}

/// Máquina de estados que monta [`Reply`]s a partir de linhas cruas.
#[derive(Debug, Default)]
pub struct ReplyParser {
    code: u16,
    lines: Vec<String>,
    in_data_block: bool,
}

impl ReplyParser {
    /// Devolve `Some` quando a linha fecha uma resposta completa.
    pub fn push(&mut self, raw: &str) -> Option<Reply> {
        let line = raw.trim_end_matches(['\r', '\n']);

        if self.in_data_block {
            if line == "." {
                self.in_data_block = false;
            } else {
                // Dot-stuffing: uma linha de dados iniciada por `.` vem duplicada.
                let content = line.strip_prefix('.').unwrap_or(line);
                if let Some(last) = self.lines.last_mut() {
                    last.push('\n');
                    last.push_str(content);
                }
            }
            return None;
        }

        let code = line.get(..3).and_then(|c| c.parse::<u16>().ok())?;
        let separator = line.as_bytes().get(3).copied()?;
        self.code = code;
        self.lines
            .push(line.get(4..).unwrap_or_default().to_string());

        match separator {
            b'+' => {
                self.in_data_block = true;
                None
            }
            b' ' => Some(Reply {
                code: self.code,
                lines: std::mem::take(&mut self.lines),
            }),
            // `-`: ainda há linhas por vir.
            _ => None,
        }
    }
}

/// Divide uma linha do protocolo em tokens, respeitando valores entre aspas.
///
/// `SUMMARY="Connecting to directory server"` vira um único token.
pub fn tokenize(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut escaped = false;

    for ch in line.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
        } else if ch == '\\' && quoted {
            escaped = true;
        } else if ch == '"' {
            quoted = !quoted;
        } else if ch.is_whitespace() && !quoted {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
        } else {
            current.push(ch);
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

/// Valor de um campo `CHAVE=valor` dentro de uma linha do protocolo.
pub fn field(line: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    tokenize(line)
        .into_iter()
        .find_map(|token| token.strip_prefix(&prefix).map(str::to_string))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_all(lines: &[&str]) -> Vec<Reply> {
        let mut parser = ReplyParser::default();
        lines.iter().filter_map(|l| parser.push(l)).collect()
    }

    #[test]
    fn parses_single_line_reply() {
        let replies = parse_all(&["250 OK\r\n"]);
        assert_eq!(
            replies,
            vec![Reply {
                code: 250,
                lines: vec!["OK".into()]
            }]
        );
    }

    #[test]
    fn parses_multiline_reply() {
        let replies = parse_all(&["250-version=0.4.8.12", "250 OK"]);
        assert_eq!(replies.len(), 1);
        assert_eq!(replies[0].value("version"), Some("0.4.8.12"));
    }

    #[test]
    fn parses_data_block() {
        let replies = parse_all(&[
            "250+circuit-status=",
            "1 BUILT $AAA~um",
            "2 BUILT $BBB~dois",
            ".",
            "250 OK",
        ]);
        assert_eq!(replies.len(), 1);
        assert_eq!(
            replies[0].value("circuit-status"),
            Some("\n1 BUILT $AAA~um\n2 BUILT $BBB~dois")
        );
    }

    #[test]
    fn non_2xx_becomes_error() {
        let reply = Reply {
            code: 515,
            lines: vec!["Authentication failed".into()],
        };
        assert!(reply.into_result().is_err());
    }

    #[test]
    fn field_handles_quoted_values() {
        let line = r#"STATUS_CLIENT NOTICE BOOTSTRAP PROGRESS=25 TAG=loading SUMMARY="Loading relay descriptors""#;
        assert_eq!(field(line, "PROGRESS").as_deref(), Some("25"));
        assert_eq!(
            field(line, "SUMMARY").as_deref(),
            Some("Loading relay descriptors")
        );
        assert_eq!(field(line, "AUSENTE"), None);
    }
}
