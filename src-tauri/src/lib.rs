//! Nika — proxy Tor na bandeja do Windows.
//!
//! O core é dividido em camadas bem estreitas:
//!
//! - [`supervisor`] é dono do `tor.exe` (subir, vigiar, reiniciar, matar);
//! - [`control`] fala o protocolo do ControlPort;
//! - [`state`] guarda o estado observável e o propaga para UI e bandeja;
//! - [`commands`]/[`tray`] são só superfícies para as mesmas [`actions`].

mod actions;
mod autostart;
mod commands;
mod config;
mod control;
mod discord;
mod error;
mod logs;
mod paths;
mod platform;
mod ports;
mod state;
mod supervisor;
mod torrc;
mod tray;
mod tunnel;
mod window;

use tauri::{Manager, RunEvent, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;

use crate::config::ConfigStore;
use crate::discord::DiscordStore;
use crate::logs::LogBuffer;
use crate::paths::Paths;
use crate::state::{AppState, ControlSlot, StatusStore};
use crate::tunnel::TunnelStore;

/// Argumento com que o Windows nos chama pela chave `Run` (RF-21).
pub const AUTOSTART_FLAG: &str = "--autostart";

pub use tunnel::{wait_for_pid, AFTER_PID_FLAG};

pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    tauri::Builder::default()
        // RF-04: registrar antes de tudo, para que a segunda instância morra cedo.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            window::show(app);
        }))
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec![AUTOSTART_FLAG]),
        ))
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(setup)
        .on_window_event(|window, event| {
            // RF-22: o X esconde na bandeja; sair é só pelo menu do tray.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::get_config,
            commands::set_config,
            commands::connect,
            commands::disconnect,
            commands::new_identity,
            commands::get_logs,
            commands::get_circuit,
            commands::get_traffic,
            commands::check_exit_ip,
            commands::discord_status,
            commands::discord_install,
            commands::discord_uninstall,
            commands::discord_relaunch,
            commands::tunnel_status,
            commands::tunnel_preview,
            commands::tunnel_import,
            commands::tunnel_forget,
            commands::tunnel_start,
            commands::tunnel_stop,
            commands::relaunch_elevated,
            commands::copy_text,
            commands::hide_window,
            commands::minimize_window,
            commands::quit,
        ])
        .build(tauri::generate_context!())
        .expect("falha ao inicializar o Tauri")
        .run(|app, event| {
            // Último recurso antes do Job Object: pedir saída limpa do tor.
            if let RunEvent::Exit = event {
                if let Some(state) = app.try_state::<AppState>() {
                    let supervisor = state.supervisor.clone();
                    let tunnel = state.tunnel.clone();
                    tauri::async_runtime::block_on(async move {
                        // RF-58: fechar os handles do WinDivert antes de sair
                        // descarrega o driver de forma limpa.
                        let _ = tunnel.stop().await;
                        supervisor.shutdown().await;
                    });
                }
            }
        });
}

/// RF-39. Silencioso de propósito: é manutenção, não uma ação do usuário.
fn reapply_discord(
    app: &tauri::AppHandle,
    settings: &crate::config::Config,
    store: &DiscordStore,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let status = store.refresh(settings)?;

    let pending = settings.discord.reapply_on_start
        && settings.discord.mode != discord::Mode::Off
        && status.component == discord::Component::Ready
        && !status.running
        && status
            .installs
            .iter()
            .flat_map(|install| &install.app_dirs)
            .any(|dir| !dir.installed);

    if !pending {
        return Ok(());
    }

    let shim = discord::component::shim_path(app).ok_or(crate::error::Error::ShimMissing)?;
    discord::install::apply(&status.installs, settings.discord.mode, settings, &shim)?;
    log::info!("proxy do Discord reaplicado depois de um update");
    store.refresh(settings)?;

    Ok(())
}

fn setup(app: &mut tauri::App) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let handle = app.handle().clone();

    let paths = Paths::resolve(&handle)?;
    paths.ensure_dirs()?;

    let config = ConfigStore::load(paths.config_file.clone());
    let status = StatusStore::new(handle.clone());
    let logs = LogBuffer::new(handle.clone());
    let control = ControlSlot::default();
    let supervisor = supervisor::spawn(
        handle.clone(),
        config.clone(),
        status.clone(),
        logs.clone(),
        control.clone(),
    );

    let discord = DiscordStore::new(handle.clone());
    let tunnel = TunnelStore::new(
        handle.clone(),
        paths.tunnel_profile.clone(),
        paths.windivert_dir.clone(),
    );

    app.manage(AppState {
        config: config.clone(),
        status: status.clone(),
        logs: logs.clone(),
        control,
        supervisor: supervisor.clone(),
        discord: discord.clone(),
        tunnel: tunnel.clone(),
    });

    tray::build(&handle)?;
    // Sincroniza ícone e menu com o estado inicial.
    status.update(|_| {});

    let settings = config.get();

    // A config é a fonte da verdade: alguém pode ter mexido no registro por fora.
    if let Err(err) = autostart::apply(&handle, settings.autostart) {
        log::warn!("{err}");
    }

    // RF-21: lançado pelo Windows, nasce escondido na bandeja.
    if !std::env::args().any(|arg| arg == AUTOSTART_FLAG) {
        window::show(&handle);
    }

    // RF-39: um update do Discord cria uma pasta `app-*` nova e sem os nossos
    // arquivos. A DLL já se recopia sozinha ao ver o Discord subir, mas isso só
    // vale enquanto a antiga carregar — aqui é a rede de segurança.
    {
        let discord = discord.clone();
        let settings = settings.clone();
        let handle = handle.clone();
        tauri::async_runtime::spawn_blocking(move || {
            if let Err(err) = reapply_discord(&handle, &settings, &discord) {
                log::warn!("revalidação do proxy do Discord falhou: {err}");
            }
        });
    }

    // RF-51/RF-56: a instância elevada nasce com `--tunnel` porque o usuário
    // aceitou o UAC para isso; `autoStart` cobre quem já abre o Nika elevado.
    // Sem elevação não há o que fazer no boot — a faixa oferece o botão.
    let wants_tunnel = std::env::args().any(|arg| arg == tunnel::START_FLAG)
        || (settings.tunnel.auto_start && tunnel::is_elevated());
    if wants_tunnel && tunnel.has_profile() {
        let tunnel = tunnel.clone();
        tauri::async_runtime::spawn(async move {
            match tunnel.start().await {
                Ok(_) => logs.push("[nika] saída própria do Discord ligada"),
                Err(err) => {
                    logs.push(format!("[nika] não consegui ligar a saída própria: {err}"));
                    log::error!("túnel no boot falhou: {err}");
                }
            }
        });
    }

    // RF-23.
    if settings.auto_connect {
        tauri::async_runtime::spawn(async move {
            if let Err(err) = supervisor.start().await {
                // O erro já está refletido no estado; aqui é só rastro.
                log::error!("conexão automática falhou: {err}");
            }
        });
    }

    Ok(())
}
