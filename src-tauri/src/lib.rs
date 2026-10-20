//! Nika — saída do Discord por túnel entre amigos, na bandeja do Windows.
//!
//! O core é pequeno:
//!
//! - [`tunnel`] é o produto: WinDivert ⇄ NAT ⇄ WireGuard, só para os processos
//!   do Discord (docs/discord-tunnel.md);
//! - [`config`] guarda preferências; o convite fica em arquivo próprio;
//! - [`commands`]/[`tray`] são só superfícies para as mesmas [`actions`].

mod actions;
mod autostart;
mod commands;
mod config;
mod error;
mod logs;
mod paths;
mod state;
mod tray;
mod tunnel;
mod window;

use tauri::{Manager, RunEvent, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;

use crate::config::ConfigStore;
use crate::logs::LogBuffer;
use crate::paths::Paths;
use crate::state::AppState;
use crate::tunnel::TunnelStore;

/// Argumento com que o Windows nos chama pela chave `Run`.
pub const AUTOSTART_FLAG: &str = "--autostart";

pub use tunnel::{wait_for_pid, AFTER_PID_FLAG};

pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    tauri::Builder::default()
        // Registrar antes de tudo, para que a segunda instância morra cedo.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            window::show(app);
        }))
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec![AUTOSTART_FLAG]),
        ))
        .setup(setup)
        .on_window_event(|window, event| {
            // O X esconde na bandeja; sair é só pelo menu do tray.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::set_config,
            commands::get_logs,
            commands::tunnel_status,
            commands::tunnel_preview,
            commands::tunnel_import,
            commands::tunnel_forget,
            commands::tunnel_start,
            commands::tunnel_stop,
            commands::relaunch_elevated,
            commands::hide_window,
            commands::minimize_window,
            commands::quit,
        ])
        .build(tauri::generate_context!())
        .expect("falha ao inicializar o Tauri")
        .run(|app, event| {
            // RF-58: fechar os handles do WinDivert antes de sair descarrega o
            // driver de forma limpa.
            if let RunEvent::Exit = event {
                if let Some(state) = app.try_state::<AppState>() {
                    let tunnel = state.tunnel.clone();
                    tauri::async_runtime::block_on(async move {
                        let _ = tunnel.stop().await;
                    });
                }
            }
        });
}

fn setup(app: &mut tauri::App) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let handle = app.handle().clone();

    let paths = Paths::resolve(&handle)?;
    paths.ensure_dirs()?;

    let config = ConfigStore::load(paths.config_file.clone());
    let logs = LogBuffer::new(handle.clone());

    // A bandeja precisa existir antes do primeiro `update` do túnel, que a
    // sincroniza com o estado inicial.
    tray::build(&handle)?;

    let tunnel = TunnelStore::new(
        handle.clone(),
        paths.tunnel_profile.clone(),
        paths.windivert_dir.clone(),
    );

    app.manage(AppState {
        config: config.clone(),
        logs: logs.clone(),
        tunnel: tunnel.clone(),
    });

    let settings = config.get();

    // A config é a fonte da verdade: alguém pode ter mexido no registro por fora.
    if let Err(err) = autostart::apply(&handle, settings.autostart) {
        log::warn!("{err}");
    }

    // Lançado pelo Windows, nasce escondido na bandeja.
    if !std::env::args().any(|arg| arg == AUTOSTART_FLAG) {
        window::show(&handle);
    }

    // RF-51/RF-56: a instância elevada nasce com `--tunnel` porque o usuário
    // aceitou o UAC para isso; `autoStart` cobre quem já abre o Nika elevado.
    // Sem elevação não há o que fazer no boot — a faixa oferece o botão.
    let wants_tunnel = std::env::args().any(|arg| arg == tunnel::START_FLAG)
        || (settings.tunnel.auto_start && tunnel::is_elevated());
    if wants_tunnel && tunnel.has_profile() {
        tauri::async_runtime::spawn(async move {
            match tunnel.start().await {
                Ok(_) => logs.push("[nika] saída do Discord ligada"),
                Err(err) => {
                    logs.push(format!("[nika] não consegui ligar a saída: {err}"));
                    log::error!("saída no boot falhou: {err}");
                }
            }
        });
    }

    Ok(())
}
