//! Superfície de IPC exposta ao webview.
//!
//! Os comandos são casca: validam, delegam para [`crate::actions`] ou para o
//! ControlPort e devolvem tipos serializáveis. Espelhados em `src/lib/ipc.ts`.

use tauri::{AppHandle, State};

use crate::actions;
use crate::autostart;
use crate::config::Config;
use crate::control::info::{self, Circuit, Traffic};
use crate::error::{Error, Result};
use crate::state::{AppState, Phase, TorStatus};

#[tauri::command]
pub fn get_status(state: State<'_, AppState>) -> TorStatus {
    state.status.get()
}

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> Config {
    state.config.get()
}

/// Persiste a configuração e aplica o que der para aplicar na hora. Trocar
/// portas com o Tor no ar reinicia o processo — é a única forma de valer.
#[tauri::command]
pub async fn set_config(
    app: AppHandle,
    state: State<'_, AppState>,
    config: Config,
) -> Result<Config> {
    let previous = state.config.get();
    state.config.set(config.clone())?;

    if config.autostart != previous.autostart {
        autostart::apply(&app, config.autostart)?;
    }

    let ports_changed = (config.socks_port, config.http_port, config.control_port)
        != (
            previous.socks_port,
            previous.http_port,
            previous.control_port,
        );

    if ports_changed && state.status.get().phase.is_active() {
        state.logs.push("[nika] portas mudaram; reiniciando o tor");
        actions::connect(&state).await?;
    }

    Ok(config)
}

#[tauri::command]
pub async fn connect(state: State<'_, AppState>) -> Result<()> {
    actions::connect(&state).await
}

#[tauri::command]
pub async fn disconnect(state: State<'_, AppState>) -> Result<()> {
    actions::disconnect(&state).await
}

#[tauri::command]
pub async fn new_identity(state: State<'_, AppState>) -> Result<()> {
    actions::new_identity(&state).await
}

#[tauri::command]
pub fn get_logs(state: State<'_, AppState>) -> Vec<String> {
    state.logs.snapshot()
}

/// RF-14. `None` enquanto nenhum circuito de uso geral estiver pronto.
#[tauri::command]
pub async fn get_circuit(state: State<'_, AppState>) -> Result<Option<Circuit>> {
    let client = state.control.require()?;
    info::active_circuit(&client).await
}

/// RF-16.
#[tauri::command]
pub async fn get_traffic(state: State<'_, AppState>) -> Result<Traffic> {
    let client = state.control.require()?;
    info::traffic(&client).await
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExitIp {
    pub ip: String,
    pub is_tor: bool,
}

#[derive(Debug, serde::Deserialize)]
struct CheckResponse {
    #[serde(rename = "IsTor")]
    is_tor: bool,
    #[serde(rename = "IP")]
    ip: String,
}

/// RF-15: verificação do IP de saída, **sempre sob demanda**.
///
/// Nunca é disparada no boot: uma requisição previsível a cada início do
/// Windows é um sinal a mais para quem observa a rede.
#[tauri::command]
pub async fn check_exit_ip(state: State<'_, AppState>) -> Result<ExitIp> {
    if state.status.get().phase != Phase::Connected {
        return Err(Error::NotConnected);
    }

    let config = state.config.get();
    // `socks5h` = o DNS também resolve pelo Tor; sem isso vaza o hostname.
    let proxy = reqwest::Proxy::all(format!("socks5h://127.0.0.1:{}", config.socks_port))?;
    let client = reqwest::Client::builder()
        .proxy(proxy)
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let response: CheckResponse = client
        .get("https://check.torproject.org/api/ip")
        .send()
        .await?
        .json()
        .await?;

    Ok(ExitIp {
        ip: response.ip,
        is_tor: response.is_tor,
    })
}

/// RF-08. A cópia passa pelo core para o webview não precisar de permissão de
/// clipboard.
#[tauri::command]
pub fn copy_text(app: AppHandle, text: String) -> Result<()> {
    actions::copy(&app, &text)
}

#[tauri::command]
pub async fn quit(app: AppHandle, state: State<'_, AppState>) -> Result<()> {
    actions::quit(app, &state).await;
    Ok(())
}
