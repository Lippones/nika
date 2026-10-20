//! Erro único do app. Tudo que chega ao frontend passa por aqui e vira string.

use serde::{Serialize, Serializer};

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    // --- túnel (docs/discord-tunnel.md §7.4) ---
    #[error("o convite é inválido: {0}")]
    TunnelProfile(String),

    #[error("nenhum convite guardado — cole o convite que você recebeu")]
    TunnelNoProfile,

    #[error("a saída precisa de permissão de administrador — reabra o Nika como administrador")]
    NeedsElevation,

    #[error("o componente da saída (WinDivert) não foi incluído neste build")]
    TunnelComponentMissing,

    #[error("a saída do Discord só funciona no Windows")]
    TunnelUnsupported,

    #[error("WinDivert: {0}")]
    #[cfg_attr(not(windows), allow(dead_code))]
    Divert(String),

    #[error("erro de E/S: {0}")]
    Io(#[from] std::io::Error),

    #[error("erro ao ler/gravar JSON: {0}")]
    Json(#[from] serde_json::Error),

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
