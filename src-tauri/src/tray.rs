//! Ícone e menu da bandeja.
//!
//! O ícone diz o estado da saída do Discord; o menu tem o mesmo par
//! ligar/desligar da faixa, mais abrir e sair.

use tauri::image::Image;
use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

use crate::actions;
use crate::error::Result;
use crate::state::AppState;
use crate::tunnel::{Phase, TunnelStatus};
use crate::window;

const TRAY_ID: &str = "nika";

mod id {
    pub const OPEN: &str = "open";
    pub const START: &str = "tunnel-start";
    pub const STOP: &str = "tunnel-stop";
    pub const QUIT: &str = "quit";
}

/// Itens que mudam conforme a saída liga ou desliga.
struct DynamicItems {
    start: MenuItem<Wry>,
    stop: MenuItem<Wry>,
}

pub fn build(app: &AppHandle) -> Result<()> {
    let open = MenuItem::with_id(app, id::OPEN, "Abrir", true, None::<&str>)?;
    let start = MenuItem::with_id(app, id::START, "Ligar saída do Discord", true, None::<&str>)?;
    let stop = MenuItem::with_id(
        app,
        id::STOP,
        "Desligar saída do Discord",
        false,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, id::QUIT, "Sair", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &open,
            &PredefinedMenuItem::separator(app)?,
            &start,
            &stop,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon(Phase::Off))
        .tooltip("Nika — saída desligada")
        .menu(&menu)
        // Clique esquerdo abre a janela; o menu fica no clique direito.
        .show_menu_on_left_click(false)
        .on_menu_event(on_menu_event)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                window::show(tray.app_handle());
            }
        })
        .build(app)?;

    app.manage(DynamicItems { start, stop });

    Ok(())
}

/// Reflete o estado da saída no ícone, na dica e nos itens de menu.
pub fn apply(app: &AppHandle, status: &TunnelStatus) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_icon(Some(icon(status.phase)));
        let _ = tray.set_tooltip(Some(tooltip(status)));
    }

    let Some(items) = app.try_state::<DynamicItems>() else {
        return;
    };

    let up = matches!(status.phase, Phase::Starting | Phase::Up);
    let _ = items.start.set_enabled(!up);
    let _ = items.stop.set_enabled(up);
}

fn tooltip(status: &TunnelStatus) -> String {
    match status.phase {
        Phase::Off => "Nika — saída desligada".into(),
        Phase::Starting => "Nika — ligando a saída".into(),
        Phase::Up => match status.last_handshake_secs {
            Some(secs) if secs > 180 => "Nika — saída sem resposta do servidor".into(),
            _ => "Nika — saída no ar".into(),
        },
        Phase::Failed => "Nika — a saída falhou".into(),
    }
}

/// Ícones são embutidos no binário: nada de I/O para trocar de estado.
fn icon(phase: Phase) -> Image<'static> {
    let bytes: &[u8] = match phase {
        Phase::Off => include_bytes!("../icons/tray-stopped.png"),
        Phase::Starting => include_bytes!("../icons/tray-connecting.png"),
        Phase::Up => include_bytes!("../icons/tray-connected.png"),
        Phase::Failed => include_bytes!("../icons/tray-error.png"),
    };

    Image::from_bytes(bytes).expect("ícone da bandeja inválido")
}

fn on_menu_event(app: &AppHandle, event: MenuEvent) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let state = state.inner().clone();
    let app = app.clone();

    match event.id().as_ref() {
        id::OPEN => window::show(&app),
        id::START => spawn(async move { actions::tunnel_start(&state).await }),
        id::STOP => spawn(async move { actions::tunnel_stop(&state).await }),
        id::QUIT => {
            tauri::async_runtime::spawn(async move { actions::quit(app, &state).await });
        }
        other => log::warn!("item de menu desconhecido: {other}"),
    }
}

/// O menu não tem para onde devolver erro: o que dá para fazer é registrar.
fn spawn(action: impl std::future::Future<Output = Result<()>> + Send + 'static) {
    tauri::async_runtime::spawn(async move {
        if let Err(err) = action.await {
            log::error!("ação da bandeja falhou: {err}");
        }
    });
}
