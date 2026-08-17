//! Erro único do app. Tudo que chega ao frontend passa por aqui e vira string.

use serde::{Serialize, Serializer};

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("a porta {port} ({role}) já está em uso — outro Tor rodando? Tente {suggestion} nas configurações")]
    PortInUse {
        role: &'static str,
        port: u16,
        suggestion: u16,
    },

    #[error("binário do Tor não encontrado. Rode `scripts/fetch-tor.ps1` antes de compilar (procurei em: {searched})")]
    TorBinaryMissing { searched: String },

    #[error("o Tor não está conectado")]
    NotConnected,

    #[error("tempo esgotado ao conectar no ControlPort")]
    ControlTimeout,

    #[error("a conexão com o ControlPort foi encerrada")]
    ControlClosed,

    #[error("ControlPort respondeu {code}: {message}")]
    ControlRejected { code: u16, message: String },

    #[error("configuração inválida: {0}")]
    InvalidConfig(String),

    #[error("erro de E/S: {0}")]
    Io(#[from] std::io::Error),

    #[error("erro ao ler/gravar JSON: {0}")]
    Json(#[from] serde_json::Error),

    #[error("erro de rede: {0}")]
    Http(#[from] reqwest::Error),

    #[error("{0}")]
    Tauri(#[from] tauri::Error),

    #[error("{0}")]
    Other(String),
}

impl Error {
    pub fn other(message: impl Into<String>) -> Self {
        Self::Other(message.into())
    }
}

/// O frontend recebe apenas a mensagem — o discriminante não interessa à UI.
impl Serialize for Error {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}
