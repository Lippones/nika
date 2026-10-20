//! Saída própria para o Discord: túnel WireGuard só para os processos do
//! Discord (RF-50 a RF-63). Spec em `docs/discord-tunnel.md`.
//!
//! Camadas, de fora para dentro:
//!
//! - [`TunnelStore`] é o que comandos e bandeja enxergam: estado observável,
//!   perfil em disco, ligar/desligar;
//! - `engine` (só Windows) liga o WinDivert ao [`wg::Link`] com o NAT no meio;
//! - [`profile`], [`packet`] e [`wg`] são puros e testados no Linux.

pub mod packet;
pub mod profile;
pub mod wg;

#[cfg(windows)]
mod divert;
#[cfg(windows)]
mod elevation;
#[cfg(windows)]
mod engine;
#[cfg(windows)]
mod owner;
#[cfg(windows)]
pub use elevation::{is_elevated, relaunch_elevated, wait_for_pid};
#[cfg(windows)]
use engine::Engine;

#[cfg(not(windows))]
mod fallback;
#[cfg(not(windows))]
use fallback::Engine;
#[cfg(not(windows))]
pub use fallback::{is_elevated, relaunch_elevated, wait_for_pid};

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::error::{Error, Result};
use profile::{Profile, ProfileSummary};

/// Espelhado em `src/lib/ipc.ts`.
pub const EVENT_TUNNEL: &str = "nika://tunnel";

/// Argumento com que a instância elevada é lançada (RF-56): ela liga o túnel
/// assim que sobe, porque foi para isso que o usuário aceitou o UAC.
pub const START_FLAG: &str = "--tunnel";
/// `--after-pid <pid>`: esperar a instância antiga sair antes de inicializar.
pub const AFTER_PID_FLAG: &str = "--after-pid";

/// Nome do arquivo do perfil dentro da pasta de dados (T-10).
pub const PROFILE_FILE: &str = "tunnel.conf";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    Off,
    Starting,
    Up,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Component {
    /// `WinDivert.dll`/`WinDivert64.sys` não empacotados neste build.
    #[default]
    Missing,
    Ready,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelStatus {
    pub phase: Phase,
    pub component: Component,
    /// O processo atual é administrador — sem isso o driver não abre.
    pub elevated: bool,
    /// Só Windows tem WinDivert; em outro SO a faixa explica e para.
    pub supported: bool,
    pub profile: Option<ProfileSummary>,
    pub last_handshake_secs: Option<u64>,
    pub tx_bytes: u64,
    pub rx_bytes: u64,
    /// Portas do Discord com tradução ativa.
    pub flows: u32,
    pub error: Option<String>,
}

impl Default for TunnelStatus {
    fn default() -> Self {
        Self {
            phase: Phase::Off,
            component: Component::Missing,
            elevated: false,
            supported: cfg!(windows),
            profile: None,
            last_handshake_secs: None,
            tx_bytes: 0,
            rx_bytes: 0,
            flows: 0,
            error: None,
        }
    }
}

/// O que o engine reporta a cada segundo enquanto está no ar.
#[derive(Debug, Clone, Copy, Default)]
pub struct EngineStats {
    pub link: wg::LinkStats,
    pub flows: u32,
}

struct Inner {
    status: TunnelStatus,
    engine: Option<Engine>,
}

/// Fonte da verdade do túnel, no mesmo desenho de [`crate::state::StatusStore`]:
/// toda escrita emite para o webview.
#[derive(Clone)]
pub struct TunnelStore {
    app: AppHandle,
    profile_path: Arc<PathBuf>,
    /// Pasta com `WinDivert.dll` + `.sys`, se o build os trouxe.
    driver_dir: Option<Arc<PathBuf>>,
    inner: Arc<Mutex<Inner>>,
}

impl TunnelStore {
    pub fn new(app: AppHandle, profile_path: PathBuf, driver_dir: Option<PathBuf>) -> Self {
        let store = Self {
            app,
            profile_path: Arc::new(profile_path),
            driver_dir: driver_dir.map(Arc::new),
            inner: Arc::new(Mutex::new(Inner {
                status: TunnelStatus::default(),
                engine: None,
            })),
        };
        store.update(|_| {});
        store
    }

    /// Estado atual, com o que é derivável do ambiente relido agora: perfil em
    /// disco, componente, elevação. O que vem do engine fica como está.
    pub fn status(&self) -> TunnelStatus {
        self.update(|_| {})
    }

    fn update(&self, mutate: impl FnOnce(&mut TunnelStatus)) -> TunnelStatus {
        let snapshot = {
            let mut guard = self.inner.lock().expect("tunnel mutex");
            mutate(&mut guard.status);
            guard.status.component = if self.driver_dir.is_some() {
                Component::Ready
            } else {
                Component::Missing
            };
            guard.status.elevated = is_elevated();
            guard.status.profile = self.load_profile().ok().map(|p| p.summary());
            guard.status.clone()
        };

        crate::tray::apply(&self.app, &snapshot);
        if let Err(err) = self.app.emit(EVENT_TUNNEL, &snapshot) {
            log::warn!("falha ao emitir {EVENT_TUNNEL}: {err}");
        }
        snapshot
    }

    fn load_profile(&self) -> Result<Profile> {
        match std::fs::read_to_string(self.profile_path.as_path()) {
            Ok(text) => Profile::parse(&text),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Err(Error::TunnelNoProfile),
            Err(err) => Err(err.into()),
        }
    }

    pub fn has_profile(&self) -> bool {
        self.profile_path.is_file()
    }

    /// O que a UI mostra enquanto a pessoa cola: o resumo do convite ou o erro,
    /// sem tocar o disco.
    pub fn preview(text: &str) -> Result<ProfileSummary> {
        let conf = profile::decode_invite(text)?;
        Ok(Profile::parse(&conf)?.summary())
    }

    /// RF-50. Aceita o `.conf` ou o convite de uma linha; valida antes de
    /// gravar, e grava sempre o `.conf` decodificado.
    pub fn import(&self, text: &str) -> Result<TunnelStatus> {
        let conf = profile::decode_invite(text)?;
        let profile = Profile::parse(&conf)?;
        if let Some(parent) = self.profile_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        write_private(self.profile_path.as_path(), &conf)?;
        log::info!(
            "perfil WireGuard importado: {} via {}",
            profile.address,
            profile.endpoint
        );
        Ok(self.update(|status| status.error = None))
    }

    /// RF-60.
    pub fn forget(&self) -> Result<TunnelStatus> {
        if self.is_running() {
            return Err(Error::other("desligue a saída antes de esquecer o convite"));
        }
        match std::fs::remove_file(self.profile_path.as_path()) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(err.into()),
        }
        Ok(self.update(|status| status.error = None))
    }

    pub fn is_running(&self) -> bool {
        self.inner.lock().expect("tunnel mutex").engine.is_some()
    }

    /// RF-51. Os pré-requisitos viram erros distintos, para a UI oferecer a
    /// ação certa (reabrir elevado, importar perfil, recompilar com o driver).
    pub async fn start(&self) -> Result<TunnelStatus> {
        if self.is_running() {
            return Ok(self.status());
        }
        if !cfg!(windows) {
            return Err(Error::TunnelUnsupported);
        }
        let driver_dir = self
            .driver_dir
            .clone()
            .ok_or(Error::TunnelComponentMissing)?;
        if !is_elevated() {
            return Err(Error::NeedsElevation);
        }
        let profile = self.load_profile()?;

        self.update(|status| {
            status.phase = Phase::Starting;
            status.error = None;
            status.last_handshake_secs = None;
            status.tx_bytes = 0;
            status.rx_bytes = 0;
            status.flows = 0;
        });

        match Engine::start(profile, driver_dir.as_path()).await {
            Ok(engine) => {
                self.inner.lock().expect("tunnel mutex").engine = Some(engine);
                self.spawn_monitor();
                Ok(self.update(|status| status.phase = Phase::Up))
            }
            Err(err) => {
                let message = err.to_string();
                self.update(|status| {
                    status.phase = Phase::Failed;
                    status.error = Some(message);
                });
                Err(err)
            }
        }
    }

    pub async fn stop(&self) -> Result<TunnelStatus> {
        let engine = self.inner.lock().expect("tunnel mutex").engine.take();
        if let Some(engine) = engine {
            engine.stop().await;
        }
        Ok(self.update(|status| {
            status.phase = Phase::Off;
            status.flows = 0;
        }))
    }

    /// Copia estatísticas do engine para o estado uma vez por segundo, e
    /// detecta o engine morto por conta própria (socket, driver).
    fn spawn_monitor(&self) {
        let store = self.clone();
        tauri::async_runtime::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(1));
            loop {
                ticker.tick().await;
                let stats = {
                    let guard = store.inner.lock().expect("tunnel mutex");
                    match &guard.engine {
                        Some(engine) => engine.stats(),
                        None => return,
                    }
                };
                match stats {
                    Ok(stats) => {
                        store.update(|status| {
                            status.last_handshake_secs = stats.link.last_handshake_secs;
                            status.tx_bytes = stats.link.tx_bytes;
                            status.rx_bytes = stats.link.rx_bytes;
                            status.flows = stats.flows;
                        });
                    }
                    Err(err) => {
                        log::error!("túnel caiu: {err}");
                        let engine = store.inner.lock().expect("tunnel mutex").engine.take();
                        if let Some(engine) = engine {
                            engine.stop().await;
                        }
                        store.update(|status| {
                            status.phase = Phase::Failed;
                            status.error = Some(err.to_string());
                        });
                        return;
                    }
                }
            }
        });
    }
}

/// Grava o perfil legível só pelo usuário. No Windows a pasta `%APPDATA%` já
/// é por usuário; no Unix (CI) fechamos o modo por garantia.
fn write_private(path: &std::path::Path, text: &str) -> Result<()> {
    let tmp = path.with_extension("conf.tmp");
    std::fs::write(&tmp, text)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
    }
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(err.into()),
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}
