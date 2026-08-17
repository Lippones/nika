//! Ações do app, compartilhadas entre a UI e o menu da bandeja.
//!
//! Tudo que existe nas duas superfícies mora aqui, para que o botão e o item de
//! menu nunca divirjam.

use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;

use crate::error::{Error, Result};
use crate::state::AppState;

pub async fn connect(state: &AppState) -> Result<()> {
    state.supervisor.start().await
}

pub async fn disconnect(state: &AppState) -> Result<()> {
    state.supervisor.stop().await
}

/// RF-13. Vale lembrar na UI: conexões já abertas seguem no circuito antigo.
pub async fn new_identity(state: &AppState) -> Result<()> {
    state.control.require()?.signal("NEWNYM").await?;
    state
        .logs
        .push("[nika] nova identidade solicitada (SIGNAL NEWNYM)");
    Ok(())
}

pub fn copy(app: &AppHandle, text: &str) -> Result<()> {
    app.clipboard().write_text(text.to_string()).map_err(|err| {
        Error::other(format!(
            "não consegui copiar para a área de transferência: {err}"
        ))
    })
}

/// Encerra o tor antes de derrubar o app, para não depender só do Job Object.
pub async fn quit(app: AppHandle, state: &AppState) {
    state.supervisor.shutdown().await;
    app.exit(0);
}
