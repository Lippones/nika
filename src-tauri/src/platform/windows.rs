//! Job Object: garante que o `tor.exe` morra junto com o app (RF-02).
//!
//! `kill_on_drop` do tokio só cobre encerramento limpo. Se o app for morto pelo
//! Gerenciador de Tarefas, nenhum código nosso roda — quem mata o filho é o
//! próprio Windows, ao fechar o último handle do Job com
//! `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`.

use std::ffi::c_void;
use std::ptr;

use tokio::process::{Child, Command};
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};

/// Não abrir janela de console ao subir o `tor.exe`.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub struct JobGuard {
    handle: HANDLE,
}

// O handle é usado apenas por chamadas de sistema thread-safe.
unsafe impl Send for JobGuard {}
unsafe impl Sync for JobGuard {}

impl JobGuard {
    /// `None` se o SO recusar o Job — o app continua funcionando, apenas sem a
    /// rede de segurança contra processo órfão.
    pub fn new() -> Option<Self> {
        unsafe {
            let handle = CreateJobObjectW(ptr::null(), ptr::null());
            if handle.is_null() {
                log::warn!("CreateJobObject falhou; tor pode sobreviver a um crash do app");
                return None;
            }

            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

            let ok = SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as *const c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );

            if ok == 0 {
                log::warn!("SetInformationJobObject falhou; Job Object desativado");
                CloseHandle(handle);
                return None;
            }

            Some(Self { handle })
        }
    }

    pub fn assign(&self, child: &Child) {
        let Some(raw) = child.raw_handle() else {
            return;
        };

        // SAFETY: `raw` é um handle válido de processo enquanto `child` existe.
        let ok = unsafe { AssignProcessToJobObject(self.handle, raw as HANDLE) };
        if ok == 0 {
            log::warn!("AssignProcessToJobObject falhou para o processo do tor");
        }
    }
}

impl Drop for JobGuard {
    fn drop(&mut self) {
        // Fechar o handle é o que dispara o kill dos processos do Job.
        unsafe { CloseHandle(self.handle) };
    }
}

pub fn prepare_command(command: &mut Command) {
    command.creation_flags(CREATE_NO_WINDOW);
}
