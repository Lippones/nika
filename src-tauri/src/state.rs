//! Estado observável do app e o que é compartilhado entre os módulos.

use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::config::ConfigStore;
use crate::control::ControlClient;
use crate::discord::DiscordStore;
use crate::logs::LogBuffer;
use crate::supervisor::SupervisorHandle;
use crate::tray;

/// Nomes dos eventos empurrados para o webview. Espelhados em `src/lib/ipc.ts`.
pub const EVENT_STATUS: &str = "nika://status";
pub const EVENT_LOG: &str = "nika://log";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    /// Nada rodando, por escolha do usuário.
    Stopped,
    /// Processo subindo, ainda sem ControlPort.
    Starting,
    /// Conectado ao ControlPort, bootstrap em andamento.
    Bootstrapping,
    /// Bootstrap em 100%: proxy pronto.
    Connected,
    /// Caiu; nova tentativa agendada.
    Retrying,
    /// Desistimos — precisa de ação do usuário.
    Failed,
}

impl Phase {
    /// Estados em que o usuário quer o Tor no ar — inclui `Retrying`, senão
    /// não haveria como cancelar um ciclo de reconexão.
    pub fn is_active(self) -> bool {
        matches!(
            self,
            Self::Starting | Self::Bootstrapping | Self::Connected | Self::Retrying
        )
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TorStatus {
    pub phase: Phase,
    /// Progresso de bootstrap, 0–100 (RF-11).
    pub bootstrap: u8,
    /// Descrição da fase atual, vinda do próprio tor.
    pub summary: String,
    pub error: Option<String>,
    /// Quantas reinicializações automáticas já aconteceram (RF-03).
    pub attempt: u32,
}

impl Default for TorStatus {
    fn default() -> Self {
        Self {
            phase: Phase::Stopped,
            bootstrap: 0,
            summary: String::from("desconectado"),
            error: None,
            attempt: 0,
        }
    }
}

/// Fonte única da verdade sobre o estado do Tor.
///
/// Toda escrita passa por [`StatusStore::update`], que garante que a UI e o
/// ícone da bandeja nunca fiquem dessincronizados do estado real.
#[derive(Clone)]
pub struct StatusStore {
    current: Arc<Mutex<TorStatus>>,
    app: AppHandle,
}

impl StatusStore {
    pub fn new(app: AppHandle) -> Self {
        Self {
            current: Arc::new(Mutex::new(TorStatus::default())),
            app,
        }
    }

    pub fn get(&self) -> TorStatus {
        self.current.lock().expect("status mutex").clone()
    }

    pub fn update(&self, mutate: impl FnOnce(&mut TorStatus)) {
        let snapshot = {
            let mut guard = self.current.lock().expect("status mutex");
            mutate(&mut guard);
            guard.clone()
        };

        tray::apply(&self.app, &snapshot);
        if let Err(err) = self.app.emit(EVENT_STATUS, &snapshot) {
            log::warn!("falha ao emitir {EVENT_STATUS}: {err}");
        }
    }

    /// Volta ao estado inicial, preservando um erro se houver.
    pub fn reset(&self, phase: Phase, summary: &str) {
        self.update(|status| {
            status.phase = phase;
            status.bootstrap = 0;
            status.summary = summary.to_string();
        });
    }
}

/// Conexão viva com o ControlPort, ou nada quando o tor está fora do ar.
///
/// O supervisor é quem escreve; os comandos só leem.
#[derive(Clone, Default)]
pub struct ControlSlot {
    inner: Arc<Mutex<Option<ControlClient>>>,
}

impl ControlSlot {
    pub fn set(&self, client: Option<ControlClient>) {
        *self.inner.lock().expect("control mutex") = client;
    }

    pub fn get(&self) -> Option<ControlClient> {
        self.inner.lock().expect("control mutex").clone()
    }

    pub fn take(&self) -> Option<ControlClient> {
        self.inner.lock().expect("control mutex").take()
    }

    /// Cliente de controle ou erro pronto para devolver ao frontend.
    pub fn require(&self) -> crate::error::Result<ControlClient> {
        self.get().ok_or(crate::error::Error::NotConnected)
    }
}

/// Estado gerenciado pelo Tauri, injetado nos comandos.
///
/// Clonar é barato (só handles compartilhados) e permite levar o estado para
/// dentro de tarefas `'static`, como as ações disparadas pelo menu da bandeja.
#[derive(Clone)]
pub struct AppState {
    pub config: ConfigStore,
    pub status: StatusStore,
    pub logs: LogBuffer,
    pub control: ControlSlot,
    pub supervisor: SupervisorHandle,
    pub discord: DiscordStore,
}
