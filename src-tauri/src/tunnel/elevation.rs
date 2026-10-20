//! Elevação (T-08, RF-56): saber se somos administrador e, se não, relançar o
//! Nika pedindo UAC — sem deixar duas instâncias vivas.
//!
//! Sequência: a instância comum chama [`relaunch_elevated`], que dispara o
//! `runas` com `--after-pid <nosso pid>` e, se o Windows aceitou, encerra o
//! app. A instância nova, ainda em `main`, chama [`wait_for_pid`] e só então
//! inicializa o Tauri — senão o plugin de instância única a mataria por ver a
//! antiga ainda saindo.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

use windows_sys::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
use windows_sys::Win32::Security::{
    GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, WaitForSingleObject, PROCESS_SYNCHRONIZE,
};
use windows_sys::Win32::UI::Shell::ShellExecuteW;

use crate::error::{Error, Result};

const SW_SHOWNORMAL: i32 = 1;
/// Quanto a instância nova espera a antiga sumir. Ela está só desligando a saída.
const HANDOFF_TIMEOUT_MS: u32 = 15_000;

pub fn is_elevated() -> bool {
    // SAFETY: token do próprio processo; handle fechado em todos os caminhos.
    unsafe {
        let mut token = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut returned = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            &mut elevation as *mut _ as *mut _,
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut returned,
        );
        CloseHandle(token);
        ok != 0 && elevation.TokenIsElevated != 0
    }
}

/// Lança uma cópia elevada deste executável com `--after-pid <pid atual>` e os
/// `args` dados. Devolve `Ok` quando o Windows aceitou — a partir daí quem
/// chamou deve encerrar o app. UAC recusado vira erro legível.
pub fn relaunch_elevated(args: &[String]) -> Result<()> {
    let exe = std::env::current_exe()?;
    let mut params = format!("{} {}", super::AFTER_PID_FLAG, std::process::id());
    for arg in args {
        params.push(' ');
        params.push_str(arg);
    }

    let verb = wide("runas");
    let file = wide(exe.as_os_str());
    let params = wide(&params);

    // SAFETY: todas as strings são NUL-terminadas e vivem durante a chamada.
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            file.as_ptr(),
            params.as_ptr(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    // Documentação: sucesso é um valor > 32; abaixo disso é um código de erro.
    if (result as isize) <= 32 {
        return Err(Error::other(
            "o Windows não autorizou abrir como administrador (UAC recusado?)",
        ));
    }
    Ok(())
}

/// Espera o processo `pid` terminar (ou o tempo esgotar). Se ele já não
/// existe, volta na hora.
pub fn wait_for_pid(pid: u32) {
    // SAFETY: handle fechado após a espera.
    unsafe {
        let handle = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
        if handle.is_null() {
            return;
        }
        let waited = WaitForSingleObject(handle, HANDOFF_TIMEOUT_MS);
        CloseHandle(handle);
        if waited != WAIT_OBJECT_0 {
            log::warn!("a instância antiga ({pid}) não saiu a tempo; seguindo mesmo assim");
        }
    }
}

fn wide(text: impl AsRef<OsStr>) -> Vec<u16> {
    text.as_ref()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}
