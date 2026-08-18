//! Detectar, fechar e reabrir o Discord (RF-28/29).
//!
//! Fechar é matar o processo, não `WM_CLOSE` (D-05): o Discord minimiza para a
//! bandeja quando recebe o fechamento da janela, então esperar saída graciosa
//! só travaria a instalação. Nunca acontece sem confirmação explícita do
//! usuário — quem decide é o comando, não este módulo.

use std::time::Duration;

use super::Install;
use crate::error::{Error, Result};

/// O Discord tem vários processos com o mesmo nome; os arquivos só destravam
/// quando todos saem.
const CLOSE_TIMEOUT: Duration = Duration::from_secs(8);
const POLL: Duration = Duration::from_millis(100);

pub fn running() -> bool {
    !pids().is_empty()
}

pub async fn close() -> Result<()> {
    for pid in pids() {
        terminate(pid);
    }

    let deadline = tokio::time::Instant::now() + CLOSE_TIMEOUT;
    while tokio::time::Instant::now() < deadline {
        if !running() {
            return Ok(());
        }
        tokio::time::sleep(POLL).await;
    }

    Err(Error::DiscordRunning)
}

/// Reabre pelo `Update.exe`, que é como o atalho do menu Iniciar faz (D-06).
/// O processo nasce solto: não pode ser filho do Nika, senão o Job Object que
/// protege o `tor.exe` levaria o Discord junto ao sair.
pub fn relaunch(install: &Install) -> Result<()> {
    let mut command = match &install.update_exe {
        Some(update) => {
            let mut command = std::process::Command::new(update);
            command.arg("--processStart").arg(install.exe_name);
            command
        }
        None => {
            let newest = install
                .app_dirs
                .first()
                .ok_or(Error::DiscordNotFound)?
                .path
                .join(install.exe_name);
            std::process::Command::new(newest)
        }
    };

    detach(&mut command);
    command.spawn()?;
    Ok(())
}

#[cfg(windows)]
fn detach(command: &mut std::process::Command) {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const DETACHED_PROCESS: u32 = 0x0000_0008;

    command.creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS);
}

#[cfg(not(windows))]
fn detach(_command: &mut std::process::Command) {}

#[cfg(windows)]
pub fn pids() -> Vec<u32> {
    use super::Flavor;
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    let mut pids = Vec::new();

    // SAFETY: o snapshot é fechado em todos os caminhos e `entry` é zerado com
    // `dwSize` preenchido, como a API exige.
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            log::warn!(
                "CreateToolhelp32Snapshot falhou; não dá para saber se o Discord está aberto"
            );
            return pids;
        }

        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

        let mut ok = Process32FirstW(snapshot, &mut entry);
        while ok != 0 {
            if Flavor::from_exe_name(&exe_name(&entry.szExeFile)).is_some() {
                pids.push(entry.th32ProcessID);
            }
            ok = Process32NextW(snapshot, &mut entry);
        }

        CloseHandle(snapshot);
    }

    pids
}

#[cfg(windows)]
fn exe_name(raw: &[u16]) -> String {
    let len = raw.iter().position(|&c| c == 0).unwrap_or(raw.len());
    String::from_utf16_lossy(&raw[..len])
}

#[cfg(windows)]
fn terminate(pid: u32) {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};

    // SAFETY: o handle é fechado logo abaixo e só é usado se for válido.
    unsafe {
        let handle = OpenProcess(PROCESS_TERMINATE, 0, pid);
        if handle.is_null() {
            // Processo já saiu entre o snapshot e agora: nada a fazer.
            return;
        }

        if TerminateProcess(handle, 0) == 0 {
            log::warn!("não consegui encerrar o processo {pid} do Discord");
        }

        CloseHandle(handle);
    }
}

#[cfg(not(windows))]
pub fn pids() -> Vec<u32> {
    Vec::new()
}

#[cfg(not(windows))]
fn terminate(_pid: u32) {}
