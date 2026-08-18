//! Proxy do Nika dentro do app do Discord (RF-27 a RF-41).
//!
//! O Discord não tem configuração de proxy e ignora o proxy de sistema. A via
//! usada aqui é a do [discord-drover]: um `version.dll` ao lado do
//! `Discord.exe` que, ao ser carregado, injeta `--proxy-server` na linha de
//! comando do Chromium e responde `http_proxy` para o lado Node.
//!
//! O papel do Nika é só o de instalador: achar as pastas, conferir o binário,
//! escrever o `drover.ini`, copiar, remover — e dizer a verdade sobre o que
//! está em disco. Ver `docs/discord-proxy.md`.
//!
//! [discord-drover]: https://github.com/hdrover/discord-drover

pub mod component;
pub mod discover;
pub mod install;
pub mod process;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::config::Config;
use crate::error::Result;

/// Espelhado em `src/lib/ipc.ts`.
pub const EVENT_DISCORD: &str = "nika://discord";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Flavor {
    Stable,
    Canary,
    Ptb,
}

impl Flavor {
    pub const ALL: [Flavor; 3] = [Flavor::Stable, Flavor::Canary, Flavor::Ptb];

    /// Nome usado tanto na chave do registro quanto na pasta de `%LOCALAPPDATA%`.
    pub fn key(self) -> &'static str {
        match self {
            Flavor::Stable => "Discord",
            Flavor::Canary => "DiscordCanary",
            Flavor::Ptb => "DiscordPTB",
        }
    }

    pub fn exe_name(self) -> &'static str {
        match self {
            Flavor::Stable => "Discord.exe",
            Flavor::Canary => "DiscordCanary.exe",
            Flavor::Ptb => "DiscordPTB.exe",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Flavor::Stable => "Discord",
            Flavor::Canary => "Discord Canary",
            Flavor::Ptb => "Discord PTB",
        }
    }

    // Fora do Windows só os testes chamam — não há processo para inspecionar.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn from_exe_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|flavor| flavor.exe_name().eq_ignore_ascii_case(name))
    }
}

/// Modo pedido pelo usuário. `TorHttp` é o padrão (D-03): o caminho SOCKS5 do
/// drover reescreve o `CONNECT` dentro do hook de `send`, o que é mais frágil
/// do que deixar Chromium e Node falarem CONNECT nativo com o `HTTPTunnelPort`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    #[default]
    Off,
    TorHttp,
    TorSocks,
}

impl Mode {
    pub fn proxy_url(self, config: &Config) -> Option<String> {
        match self {
            Mode::Off => None,
            Mode::TorHttp => Some(config.http_url()),
            Mode::TorSocks => Some(config.socks_url()),
        }
    }

    /// Modo correspondente ao que está gravado no `drover.ini` (D-07: o estado
    /// vem do disco, não do que o app acha que instalou).
    pub fn from_proxy(url: &str) -> Self {
        let url = url.trim().to_ascii_lowercase();
        if url.starts_with("socks5://") {
            Mode::TorSocks
        } else if url.starts_with("http://") || url.starts_with("https://") {
            Mode::TorHttp
        } else {
            Mode::Off
        }
    }
}

/// Uma pasta `app-1.0.9186` — é nela que os arquivos entram.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppDir {
    pub path: std::path::PathBuf,
    /// `1.0.9186`, extraída do nome da pasta. Vazia se o nome fugir do padrão.
    pub version: String,
    /// `version.dll` presente **e** com o hash do componente que conhecemos.
    pub installed: bool,
    /// Valor de `proxy =` lido do `drover.ini`, se houver.
    pub proxy: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Install {
    pub flavor: Flavor,
    pub label: &'static str,
    pub base_dir: std::path::PathBuf,
    pub exe_name: &'static str,
    /// `Update.exe`, quando existe: é como o atalho do menu Iniciar abre o app.
    pub update_exe: Option<std::path::PathBuf>,
    /// Da versão mais nova para a mais antiga.
    pub app_dirs: Vec<AppDir>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Component {
    /// O shim não foi empacotado neste build (dev/Linux ou build Tor-only).
    #[default]
    Missing,
    /// `version.dll` empacotado e pronto para instalar.
    Ready,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DiscordStatus {
    pub component: Component,
    pub component_version: Option<String>,
    pub installs: Vec<Install>,
    /// Algum processo do Discord vivo — instalar e remover exigem que não haja.
    pub running: bool,
    /// Modo lido do disco, não o da config.
    pub effective: Mode,
    /// Instalado, mas apontando para porta diferente da config atual (RF-38).
    pub stale: bool,
}

/// Varredura completa. Barata: `read_dir` + registro + um SHA-256 por pasta.
pub fn scan(app: &AppHandle, config: &Config) -> DiscordStatus {
    // Hash do shim empacotado, calculado uma vez: é a régua para decidir se a
    // `version.dll` de cada pasta é a nossa.
    let shim = component::shim_path(app);
    let shim_hash = shim.as_deref().and_then(component::sha256_file);

    let installs = discover::installs(shim_hash.as_deref());
    let dirs = || installs.iter().flat_map(|install| install.app_dirs.iter());

    let effective = dirs()
        .find(|dir| dir.installed)
        .and_then(|dir| dir.proxy.as_deref())
        .map(Mode::from_proxy)
        .unwrap_or_default();

    let expected = effective.proxy_url(config);
    let stale = is_stale(&installs, expected.as_deref());

    DiscordStatus {
        component: component::state(app),
        component_version: shim.is_some().then(|| component::VERSION.to_string()),
        running: process::running(),
        effective,
        stale,
        installs,
    }
}

/// RF-38: alguma pasta instalada aponta para endereço diferente do que a config
/// atual produz. Sem endereço esperado (proxy desligado) nada está velho.
fn is_stale(installs: &[Install], expected: Option<&str>) -> bool {
    let Some(expected) = expected else {
        return false;
    };

    installs
        .iter()
        .flat_map(|install| &install.app_dirs)
        .any(|dir| dir.installed && dir.proxy.as_deref() != Some(expected))
}

/// Ponte entre o disco e a UI, no mesmo desenho de
/// [`crate::state::StatusStore`]: quem lê emite, e ninguém faz polling.
///
/// Aqui não há cache — o estado do proxy mora na pasta do Discord, e guardar
/// uma cópia criaria uma segunda verdade, que envelhece a cada update do app
/// (D-07). A varredura é barata o bastante para ser sempre nova.
#[derive(Clone)]
pub struct DiscordStore {
    app: AppHandle,
}

impl DiscordStore {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }

    pub fn refresh(&self, config: &Config) -> Result<DiscordStatus> {
        let status = scan(&self.app, config);

        if let Err(err) = self.app.emit(EVENT_DISCORD, &status) {
            log::warn!("falha ao emitir {EVENT_DISCORD}: {err}");
        }

        Ok(status)
    }
}

/// RF-38: as portas do Tor mudaram; o `drover.ini` de cada pasta instalada
/// precisa acompanhar. A DLL só lê o ini quando carrega, então quem estiver com
/// o Discord aberto continua no endereço antigo até reiniciar — é o que a UI
/// informa a partir de `stale`.
pub fn sync_ports(app: &AppHandle, config: &Config) -> Result<()> {
    let status = scan(app, config);
    if status.effective == Mode::Off {
        return Ok(());
    }

    install::rewrite_ini(&status.installs, status.effective, config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_comes_from_the_scheme_on_disk() {
        assert_eq!(Mode::from_proxy("http://127.0.0.1:9080"), Mode::TorHttp);
        assert_eq!(
            Mode::from_proxy(" socks5://127.0.0.1:9050 "),
            Mode::TorSocks
        );
        assert_eq!(Mode::from_proxy(""), Mode::Off);
    }

    #[test]
    fn proxy_url_follows_the_configured_ports() {
        let config = Config {
            http_port: 9081,
            ..Config::default()
        };
        assert_eq!(
            Mode::TorHttp.proxy_url(&config).as_deref(),
            Some("http://127.0.0.1:9081")
        );
        assert_eq!(Mode::Off.proxy_url(&config), None);
    }

    fn app_dir(installed: bool, proxy: Option<&str>) -> AppDir {
        AppDir {
            path: std::path::PathBuf::from("app-1.0.9186"),
            version: "1.0.9186".into(),
            installed,
            proxy: proxy.map(str::to_string),
        }
    }

    fn install_with(dirs: Vec<AppDir>) -> Install {
        Install {
            flavor: Flavor::Stable,
            label: Flavor::Stable.label(),
            base_dir: std::path::PathBuf::from("Discord"),
            exe_name: Flavor::Stable.exe_name(),
            update_exe: None,
            app_dirs: dirs,
        }
    }

    #[test]
    fn stale_when_the_installed_address_lost_the_port() {
        let installs = vec![install_with(vec![app_dir(
            true,
            Some("http://127.0.0.1:9080"),
        )])];

        assert!(!is_stale(&installs, Some("http://127.0.0.1:9080")));
        assert!(is_stale(&installs, Some("http://127.0.0.1:9081")));
        // Proxy desligado: não há endereço esperado, nada a comparar.
        assert!(!is_stale(&installs, None));
    }

    #[test]
    fn a_folder_without_our_dll_does_not_make_it_stale() {
        let installs = vec![install_with(vec![app_dir(false, None)])];
        assert!(!is_stale(&installs, Some("http://127.0.0.1:9080")));
    }

    #[test]
    fn recognizes_every_discord_executable() {
        assert_eq!(Flavor::from_exe_name("discord.exe"), Some(Flavor::Stable));
        assert_eq!(Flavor::from_exe_name("DiscordPTB.exe"), Some(Flavor::Ptb));
        assert_eq!(Flavor::from_exe_name("chrome.exe"), None);
    }
}
