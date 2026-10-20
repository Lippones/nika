//! Ações do app, compartilhadas entre a UI e o menu da bandeja.
//!
//! Tudo que existe nas duas superfícies mora aqui, para que o botão e o item de
//! menu nunca divirjam.

use tauri::AppHandle;

use crate::error::Result;
use crate::state::AppState;

/// RF-51. Mesmo caminho para o botão e para a bandeja.
pub async fn tunnel_start(state: &AppState) -> Result<()> {
    state.tunnel.start().await?;
    state.logs.push("[nika] saída do Discord ligada");
    Ok(())
}

pub async fn tunnel_stop(state: &AppState) -> Result<()> {
    state.tunnel.stop().await?;
    state.logs.push("[nika] saída do Discord desligada");
    Ok(())
}

/// RF-56. Se o Windows aceitou, este processo sai — o novo assume (ver
/// `main.rs` e `tunnel::elevation`). Se o usuário recusou o UAC, nada muda.
pub async fn relaunch_elevated(app: AppHandle, state: &AppState) -> Result<()> {
    crate::tunnel::relaunch_elevated(&[crate::tunnel::START_FLAG.to_string()])?;
    state
        .logs
        .push("[nika] reabrindo como administrador para ligar a saída");
    quit(app, state).await;
    Ok(())
}

/// Desliga a saída antes de derrubar o app: fechar os handles do WinDivert
/// descarrega o driver de forma limpa (RF-58).
pub async fn quit(app: AppHandle, state: &AppState) {
    let _ = state.tunnel.stop().await;
    app.exit(0);
}
