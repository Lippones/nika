//! Superfície de IPC exposta ao webview.
//!
//! Os comandos são casca: validam, delegam para [`crate::actions`] ou para o
//! [`crate::tunnel::TunnelStore`] e devolvem tipos serializáveis. Espelhados
//! em `src/lib/ipc.ts`.

use tauri::{AppHandle, State};

use crate::actions;
use crate::autostart;
use crate::config::Config;
use crate::error::Result;
use crate::state::AppState;
use crate::tunnel::{profile::ProfileSummary, TunnelStatus, TunnelStore};

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> Config {
    state.config.get()
}

/// Persiste a configuração e aplica o que der para aplicar na hora.
#[tauri::command]
pub fn set_config(app: AppHandle, state: State<'_, AppState>, config: Config) -> Result<Config> {
    let previous = state.config.get();
    state.config.set(config.clone())?;

    if config.autostart != previous.autostart {
        autostart::apply(&app, config.autostart)?;
    }

    Ok(config)
}

#[tauri::command]
pub fn get_logs(state: State<'_, AppState>) -> Vec<String> {
    state.logs.snapshot()
}

// ============================ Saída do Discord (docs/discord-tunnel.md §7.3) ==

#[tauri::command]
pub fn tunnel_status(state: State<'_, AppState>) -> TunnelStatus {
    state.tunnel.status()
}

/// Resumo do convite colado, sem gravar: é o que a tela mostra ao vivo.
#[tauri::command]
pub fn tunnel_preview(text: String) -> Result<ProfileSummary> {
    TunnelStore::preview(&text)
}

/// RF-50. O texto é o `.conf` ou o convite de uma linha; a validação acontece
/// antes de gravar.
#[tauri::command]
pub fn tunnel_import(state: State<'_, AppState>, text: String) -> Result<TunnelStatus> {
    state.tunnel.import(&text)
}

/// RF-60.
#[tauri::command]
pub fn tunnel_forget(state: State<'_, AppState>) -> Result<TunnelStatus> {
    state.tunnel.forget()
}

#[tauri::command]
pub async fn tunnel_start(state: State<'_, AppState>) -> Result<TunnelStatus> {
    actions::tunnel_start(&state).await?;
    Ok(state.tunnel.status())
}

#[tauri::command]
pub async fn tunnel_stop(state: State<'_, AppState>) -> Result<TunnelStatus> {
    actions::tunnel_stop(&state).await?;
    Ok(state.tunnel.status())
}

/// RF-56.
#[tauri::command]
pub async fn relaunch_elevated(app: AppHandle, state: State<'_, AppState>) -> Result<()> {
    actions::relaunch_elevated(app, &state).await
}

// ============================ Janela ========================================

/// Esconde a janela na bandeja. É o botão de fechar da UI: sem moldura nativa,
/// não há X do sistema (o `CloseRequested` do Alt+F4 cai no mesmo destino, em
/// `lib.rs`).
#[tauri::command]
pub fn hide_window(app: AppHandle) {
    crate::window::hide(&app);
}

#[tauri::command]
pub fn minimize_window(app: AppHandle) {
    crate::window::minimize(&app);
}

#[tauri::command]
pub async fn quit(app: AppHandle, state: State<'_, AppState>) -> Result<()> {
    actions::quit(app, &state).await;
    Ok(())
}
