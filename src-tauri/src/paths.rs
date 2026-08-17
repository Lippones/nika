//! Resolução de todos os caminhos usados pelo app.
//!
//! Layout em disco (Windows):
//!
//! ```text
//! %APPDATA%\dev.nika.tortray\
//!   config.json          configuração do usuário (RF-24)
//!   torrc                gerado a cada start (§8.2 do PRD)
//!   tor\                 DataDirectory do tor
//!     control_auth_cookie
//! ```

use std::path::PathBuf;

use tauri::path::BaseDirectory;
use tauri::{AppHandle, Manager};

use crate::error::{Error, Result};

#[derive(Debug, Clone)]
pub struct Paths {
    /// `DataDirectory` do tor.
    pub tor_data: PathBuf,
    /// Arquivo de configuração do tor, regerado a cada start.
    pub torrc: PathBuf,
    /// Cookie de autenticação do ControlPort (escrito pelo tor).
    pub cookie: PathBuf,
    /// Configuração do usuário.
    pub config_file: PathBuf,
    /// Bases de GeoIP do bundle; ausentes se o bundle não foi baixado.
    pub geoip: Option<PathBuf>,
    pub geoip6: Option<PathBuf>,
}

impl Paths {
    pub fn resolve(app: &AppHandle) -> Result<Self> {
        let root = app.path().app_data_dir()?;
        let tor_data = root.join("tor");

        Ok(Self {
            cookie: tor_data.join("control_auth_cookie"),
            torrc: root.join("torrc"),
            config_file: root.join("config.json"),
            geoip: resource(app, "resources/geoip"),
            geoip6: resource(app, "resources/geoip6"),
            tor_data,
        })
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        std::fs::create_dir_all(&self.tor_data)?;
        Ok(())
    }
}

/// Um recurso empacotado só conta se existir de fato — em desenvolvimento o
/// bundle do Tor pode não ter sido baixado ainda, e o tor roda sem GeoIP.
fn resource(app: &AppHandle, relative: &str) -> Option<PathBuf> {
    app.path()
        .resolve(relative, BaseDirectory::Resource)
        .ok()
        .filter(|path| path.is_file())
}

/// Localiza o `tor.exe`.
///
/// O PRD previa declarar o tor como `externalBin`, mas o Expert Bundle não é um
/// binário solto: o `tor.exe` carrega DLLs que precisam estar no mesmo
/// diretório, e `externalBin` só instala um arquivo. Por isso o bundle inteiro
/// vai como recurso em `resources/tor/`. Os caminhos de sidecar continuam na
/// lista para quem preferir o outro layout.
pub fn tor_binary(app: &AppHandle) -> Result<PathBuf> {
    let file_name = format!("tor{}", std::env::consts::EXE_SUFFIX);
    let mut candidates = Vec::new();

    if let Ok(bundled) = app.path().resolve(
        format!("resources/tor/{file_name}"),
        BaseDirectory::Resource,
    ) {
        candidates.push(bundled);
    }

    let exe_path = std::env::current_exe()?;
    if let Some(exe_dir) = exe_path.parent() {
        candidates.push(exe_dir.join(&file_name));
        candidates.push(exe_dir.join(format!(
            "tor-{}{}",
            env!("TARGET_TRIPLE"),
            std::env::consts::EXE_SUFFIX
        )));
    }

    candidates
        .iter()
        .find(|path| path.is_file())
        .cloned()
        .ok_or_else(|| Error::TorBinaryMissing {
            searched: candidates
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", "),
        })
}
