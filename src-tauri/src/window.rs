//! Helpers da janela principal.

use tauri::{AppHandle, Manager};

pub const MAIN: &str = "main";

/// Traz a janela para frente, venha o pedido do tray, do segundo processo
/// (RF-04) ou do próprio app.
pub fn show(app: &AppHandle) {
    let Some(window) = app.get_webview_window(MAIN) else {
        log::warn!("janela `{MAIN}` não existe");
        return;
    };

    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();
}
