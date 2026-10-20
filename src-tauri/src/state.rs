//! O que é compartilhado entre comandos, bandeja e tarefas.

use crate::config::ConfigStore;
use crate::logs::LogBuffer;
use crate::tunnel::TunnelStore;

/// Estado gerenciado pelo Tauri, injetado nos comandos.
///
/// Clonar é barato (só handles compartilhados) e permite levar o estado para
/// dentro de tarefas `'static`, como as ações disparadas pelo menu da bandeja.
#[derive(Clone)]
pub struct AppState {
    pub config: ConfigStore,
    pub logs: LogBuffer,
    pub tunnel: TunnelStore,
}
