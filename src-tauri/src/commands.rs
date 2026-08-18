//! Superfície de IPC exposta ao webview.
//!
//! Os comandos são casca: validam, delegam para [`crate::actions`] ou para o
//! ControlPort e devolvem tipos serializáveis. Espelhados em `src/lib/ipc.ts`.

use tauri::{AppHandle, State};

use crate::actions;
use crate::autostart;
use crate::config::Config;
use crate::control::info::{self, Circuit, Traffic};
use crate::discord::{self, DiscordStatus, Mode};
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

    if ports_changed {
        // RF-38: o `drover.ini` guarda o endereço, não a porta — precisa
        // acompanhar. Falhar aqui não invalida a troca de portas.
        if let Err(err) = discord::sync_ports(&app, &config) {
            log::warn!("não consegui atualizar o proxy do Discord: {err}");
        }
        let _ = state.discord.refresh(&config);
    }

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

/// RF-35: o estado vem sempre de uma leitura nova do disco e do registro.
#[tauri::command]
pub fn discord_status(state: State<'_, AppState>) -> Result<DiscordStatus> {
    state.discord.refresh(&state.config.get())
}

/// RF-30. `close_discord` e `relaunch` chegam de confirmação explícita na UI:
/// matar o app de outra pessoa nunca pode ser efeito colateral (D-05).
#[tauri::command]
pub async fn discord_install(
    app: AppHandle,
    state: State<'_, AppState>,
    mode: Mode,
    close_discord: bool,
    relaunch: bool,
) -> Result<DiscordStatus> {
    if mode == Mode::Off {
        return Err(Error::InvalidConfig(
            "escolha HTTP ou SOCKS5 para o proxy do Discord".into(),
        ));
    }

    // RF-36: instalar com o Tor fora do ar entrega um Discord que não conecta.
    if state.status.get().phase != Phase::Connected {
        return Err(Error::NotConnected);
    }

    let mut config = state.config.get();
    let shim = discord::component::shim_path(&app).ok_or(Error::ShimMissing)?;
    let shim_hash = discord::component::sha256_file(&shim);

    let installs = discord::discover::installs(shim_hash.as_deref());
    if installs.is_empty() {
        return Err(Error::DiscordNotFound);
    }

    let was_running = discord::process::running();
    if was_running {
        if !close_discord {
            return Err(Error::DiscordRunning);
        }
        state
            .logs
            .push("[nika] fechando o Discord para instalar o proxy");
        discord::process::close().await?;
    }

    discord::install::apply(&installs, mode, &config, &shim)?;

    config.discord.mode = mode;
    config.discord.allow_close = close_discord;
    state.config.set(config.clone())?;
    state.logs.push(format!(
        "[nika] proxy do Discord instalado em {} pasta(s)",
        installs.iter().map(|i| i.app_dirs.len()).sum::<usize>()
    ));

    if relaunch && was_running {
        reopen(&installs);
    }

    state.discord.refresh(&config)
}

/// RF-31.
#[tauri::command]
pub async fn discord_uninstall(
    state: State<'_, AppState>,
    close_discord: bool,
    relaunch: bool,
) -> Result<DiscordStatus> {
    let mut config = state.config.get();
    let installs = discord::discover::installs(None);
    if installs.is_empty() {
        return Err(Error::DiscordNotFound);
    }

    let was_running = discord::process::running();
    if was_running {
        if !close_discord {
            return Err(Error::DiscordRunning);
        }
        state
            .logs
            .push("[nika] fechando o Discord para remover o proxy");
        discord::process::close().await?;
    }

    discord::install::remove(&installs)?;

    config.discord.mode = Mode::Off;
    state.config.set(config.clone())?;
    state.logs.push("[nika] proxy do Discord removido");

    if relaunch && was_running {
        reopen(&installs);
    }

    state.discord.refresh(&config)
}

/// RF-29: usado depois de trocar as portas, quando o Discord precisa reler o ini.
#[tauri::command]
pub async fn discord_relaunch(state: State<'_, AppState>) -> Result<DiscordStatus> {
    let installs = discord::discover::installs(None);
    if installs.is_empty() {
        return Err(Error::DiscordNotFound);
    }

    if discord::process::running() {
        discord::process::close().await?;
    }
    reopen(&installs);

    state.discord.refresh(&state.config.get())
}

/// Reabre o que foi fechado. Um sabor que não volta não invalida a operação —
/// os arquivos já estão no lugar; o usuário abre pelo atalho.
fn reopen(installs: &[discord::Install]) {
    for install in installs {
        if let Err(err) = discord::process::relaunch(install) {
            log::warn!("não consegui reabrir o {}: {err}", install.label);
        }
    }
}

/// RF-08. A cópia passa pelo core para o webview não precisar de permissão de
/// clipboard.
#[tauri::command]
pub fn copy_text(app: AppHandle, text: String) -> Result<()> {
    actions::copy(&app, &text)
}

/// Esconde a janela na bandeja. É o botão de fechar da UI: sem moldura nativa,
/// não há X do sistema (o `CloseRequested` do Alt+F4 continua caindo no mesmo
/// destino, em `lib.rs`).
#[tauri::command]
pub fn hide_window(app: AppHandle) {
    crate::window::hide(&app);
}

/// Minimiza a janela sem moldura.
#[tauri::command]
pub fn minimize_window(app: AppHandle) {
    crate::window::minimize(&app);
}

#[tauri::command]
pub async fn quit(app: AppHandle, state: State<'_, AppState>) -> Result<()> {
    actions::quit(app, &state).await;
    Ok(())
}
