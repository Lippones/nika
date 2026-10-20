//! Resolução dos caminhos usados pelo app.
//!
//! Layout em disco (Windows):
//!
//! ```text
//! %APPDATA%\dev.nika.tortray\
//!   config.json          preferências
//!   tunnel.conf          convite importado (contém a chave privada)
//! ```
//!
//! O identificador `dev.nika.tortray` vem da primeira versão e fica como está:
//! trocá-lo mudaria a pasta de dados e a identidade do instalador.

use std::path::{Path, PathBuf};

use tauri::path::BaseDirectory;
use tauri::{AppHandle, Manager};

use crate::error::Result;

#[derive(Debug, Clone)]
pub struct Paths {
    /// Preferências do usuário.
    pub config_file: PathBuf,
    /// Convite WireGuard importado (docs/discord-tunnel.md T-10).
    pub tunnel_profile: PathBuf,
    /// Pasta com `WinDivert.dll` + `WinDivert64.sys`; `None` se o build não
    /// trouxe o driver (T-07) — a UI mostra "componente ausente".
    pub windivert_dir: Option<PathBuf>,
}

impl Paths {
    pub fn resolve(app: &AppHandle) -> Result<Self> {
        let root = app.path().app_data_dir()?;

        Ok(Self {
            config_file: root.join("config.json"),
            tunnel_profile: root.join(crate::tunnel::PROFILE_FILE),
            windivert_dir: resource(app, "resources/windivert/WinDivert.dll")
                .filter(|dll| dll.with_file_name("WinDivert64.sys").is_file())
                .and_then(|dll| dll.parent().map(Path::to_path_buf)),
        })
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        if let Some(parent) = self.config_file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(())
    }
}

/// Um recurso empacotado só conta se existir de fato — em desenvolvimento o
/// driver pode não ter sido baixado ainda.
fn resource(app: &AppHandle, relative: &str) -> Option<PathBuf> {
    app.path()
        .resolve(relative, BaseDirectory::Resource)
        .ok()
        .filter(|path| path.is_file())
}
