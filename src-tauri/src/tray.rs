//! Ícone e menu da bandeja (RF-18, RF-19).

use tauri::image::Image;
use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

use crate::actions;
use crate::error::Result;
use crate::state::{AppState, Phase, TorStatus};
use crate::window;

const TRAY_ID: &str = "nika";

mod id {
    pub const OPEN: &str = "open";
    pub const CONNECT: &str = "connect";
    pub const DISCONNECT: &str = "disconnect";
    pub const NEW_IDENTITY: &str = "new-identity";
    pub const COPY_SOCKS: &str = "copy-socks";
    pub const COPY_HTTP: &str = "copy-http";
    pub const QUIT: &str = "quit";
}

/// Itens que mudam de estado conforme o Tor conecta ou cai.
struct DynamicItems {
    connect: MenuItem<Wry>,
    disconnect: MenuItem<Wry>,
    new_identity: MenuItem<Wry>,
}

pub fn build(app: &AppHandle) -> Result<()> {
    let open = MenuItem::with_id(app, id::OPEN, "Abrir", true, None::<&str>)?;
    let connect = MenuItem::with_id(app, id::CONNECT, "Conectar", true, None::<&str>)?;
    let disconnect = MenuItem::with_id(app, id::DISCONNECT, "Desconectar", false, None::<&str>)?;
    let new_identity = MenuItem::with_id(
        app,
        id::NEW_IDENTITY,
        "Nova identidade",
        false,
        None::<&str>,
    )?;
    let copy_socks = MenuItem::with_id(app, id::COPY_SOCKS, "Copiar SOCKS5", true, None::<&str>)?;
    let copy_http = MenuItem::with_id(app, id::COPY_HTTP, "Copiar HTTP", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, id::QUIT, "Sair", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &open,
            &PredefinedMenuItem::separator(app)?,
            &connect,
            &disconnect,
            &new_identity,
            &PredefinedMenuItem::separator(app)?,
            &copy_socks,
            &copy_http,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon(Phase::Stopped))
        .tooltip("Nika — desconectado")
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

    app.manage(DynamicItems {
        connect,
        disconnect,
        new_identity,
    });

    Ok(())
}

/// Reflete o estado do Tor no ícone, na dica e nos itens de menu (RF-18).
pub fn apply(app: &AppHandle, status: &TorStatus) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_icon(Some(icon(status.phase)));
        let _ = tray.set_tooltip(Some(tooltip(status)));
    }

    let Some(items) = app.try_state::<DynamicItems>() else {
        return;
    };

    let active = status.phase.is_active();
    let _ = items.connect.set_enabled(!active);
    let _ = items.disconnect.set_enabled(active);
    let _ = items
        .new_identity
        .set_enabled(status.phase == Phase::Connected);
}

fn tooltip(status: &TorStatus) -> String {
    match status.phase {
        Phase::Stopped => "Nika — desconectado".into(),
        Phase::Starting => "Nika — iniciando".into(),
        Phase::Bootstrapping => format!("Nika — conectando ({}%)", status.bootstrap),
        Phase::Connected => "Nika — conectado".into(),
        Phase::Retrying => format!("Nika — reconectando (tentativa {})", status.attempt),
        Phase::Failed => "Nika — erro".into(),
    }
}

/// Ícones são embutidos no binário: nada de I/O para trocar de estado.
fn icon(phase: Phase) -> Image<'static> {
    let bytes: &[u8] = match phase {
        Phase::Stopped => include_bytes!("../icons/tray-stopped.png"),
        Phase::Starting | Phase::Bootstrapping | Phase::Retrying => {
            include_bytes!("../icons/tray-connecting.png")
        }
        Phase::Connected => include_bytes!("../icons/tray-connected.png"),
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

        id::COPY_SOCKS => report(actions::copy(&app, &state.config.get().socks_url())),
        id::COPY_HTTP => report(actions::copy(&app, &state.config.get().http_url())),

        id::CONNECT => spawn(async move { actions::connect(&state).await }),
        id::DISCONNECT => spawn(async move { actions::disconnect(&state).await }),
        id::NEW_IDENTITY => spawn(async move { actions::new_identity(&state).await }),

        id::QUIT => {
            tauri::async_runtime::spawn(async move { actions::quit(app, &state).await });
        }

        other => log::warn!("item de menu desconhecido: {other}"),
    }
}

/// O menu não tem para onde devolver erro: o que dá para fazer é registrar.
fn spawn(action: impl std::future::Future<Output = Result<()>> + Send + 'static) {
    tauri::async_runtime::spawn(async move { report(action.await) });
}

fn report(result: Result<()>) {
    if let Err(err) = result {
        log::error!("ação da bandeja falhou: {err}");
    }
}
