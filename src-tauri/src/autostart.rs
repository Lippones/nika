//! Início automático com o Windows (RF-20).
//!
//! O plugin usa `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` — chave de
//! usuário, sem privilégio de administrador — e adiciona o argumento
//! [`crate::AUTOSTART_FLAG`], que faz o app subir escondido na bandeja (RF-21).

use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;

use crate::error::{Error, Result};

pub fn apply(app: &AppHandle, enabled: bool) -> Result<()> {
    let manager = app.autolaunch();

    let outcome = if enabled {
        manager.enable()
    } else {
        manager.disable()
    };

    outcome.map_err(|err| Error::other(format!("não consegui ajustar o início automático: {err}")))
}
