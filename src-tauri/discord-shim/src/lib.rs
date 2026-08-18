//! `version.dll` do Nika — injeta o proxy Tor no processo do Discord.
//!
//! Colocado ao lado do `Discord.exe`, o loader do Windows carrega esta DLL no
//! lugar do `version.dll` do System32 (a busca começa pelo diretório do
//! executável). Ao carregar, ela:
//!
//! 1. reexporta as 17 funções do `version.dll` real, encaminhando cada uma para
//!    o `version.dll` verdadeiro do System32 (§4 de docs/discord-dll.md);
//! 2. instala dois hooks inline — `GetCommandLineW` e `GetEnvironmentVariableW`
//!    — que injetam `--proxy-server` e `http_proxy` (§5).
//!
//! Regras que não podem ser quebradas:
//! - **Falhar aberto** (E-08): qualquer erro deixa o Discord subir sem proxy.
//!   Nunca derrubar o processo. `panic = "abort"` + nenhum `unwrap` no init.
//! - **`DllMain` sempre retorna TRUE**: FALSE impediria o Discord de iniciar.
//! - **Nada de forwarder de `.def`** (E-05): os stubs saltam para um ponteiro
//!   que nós resolvemos apontando para o System32, nunca para "version.Foo".
//!
//! Este arquivo compila (`cargo check --target x86_64-pc-windows-msvc`), mas só
//! o teste dentro de um Discord real prova que ele hooka sem quebrar nada — ver
//! os gates em docs/discord-dll.md §12.

#![allow(non_snake_case)]

use core::ffi::c_void;
use core::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;

use retour::GenericDetour;
use windows_sys::Win32::Foundation::{BOOL, HINSTANCE, TRUE};
use windows_sys::Win32::System::LibraryLoader::{
    GetModuleFileNameW, GetModuleHandleW, GetProcAddress, LoadLibraryW,
};
use windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW;
use windows_sys::Win32::System::SystemServices::DLL_PROCESS_ATTACH;

// ============================ Reexports (§4) ================================

/// Destino de qualquer reexport que não tenha sido resolvido: zera o retorno e
/// volta. `xor eax, eax; ret` é seguro para qualquer uma das 17 funções (todas
/// retornam via RAX; 0/NULL é um "falhou" que o chamador trata), e converte o
/// que seria um `jmp` para NULL numa falha benigna.
#[unsafe(naked)]
extern "C" fn reexport_fallback() {
    core::arch::naked_asm!("xor eax, eax", "ret")
}

/// Gera, para cada função do `version.dll`, um ponteiro global (preenchido em
/// runtime) e um stub `#[naked]` que salta para ele. O stub é agnóstico à
/// assinatura: não toca em registrador nem em pilha, só desvia.
macro_rules! reexport {
    ($($stub:ident => $slot:ident),* $(,)?) => {
        $(
            static $slot: AtomicUsize = AtomicUsize::new(0);

            #[unsafe(naked)]
            #[no_mangle]
            pub extern "C" fn $stub() {
                core::arch::naked_asm!("jmp qword ptr [rip + {ptr}]", ptr = sym $slot)
            }
        )*

        /// Preenche todos os slots. **Nenhum fica em 0**: o que não resolver
        /// aponta para o fallback, que retorna 0. Assim um export ausente (ou o
        /// version.dll real não carregar) vira "falha benigna" em vez de um
        /// `jmp` para NULL — é o que mantém a promessa de falhar aberto (E-08).
        unsafe fn prime_reexports(real: Option<*mut c_void>) {
            let fallback = reexport_fallback as *const () as usize;
            $(
                let addr = real
                    .map(|module| {
                        proc_address(module, concat!(stringify!($stub), "\0").as_bytes())
                    })
                    .unwrap_or(0);
                $slot.store(if addr != 0 { addr } else { fallback }, Ordering::Relaxed);
            )*
        }
    };
}

reexport! {
    GetFileVersionInfoA        => REAL_GET_FILE_VERSION_INFO_A,
    GetFileVersionInfoByHandle => REAL_GET_FILE_VERSION_INFO_BY_HANDLE,
    GetFileVersionInfoExA      => REAL_GET_FILE_VERSION_INFO_EX_A,
    GetFileVersionInfoExW      => REAL_GET_FILE_VERSION_INFO_EX_W,
    GetFileVersionInfoSizeA    => REAL_GET_FILE_VERSION_INFO_SIZE_A,
    GetFileVersionInfoSizeExA  => REAL_GET_FILE_VERSION_INFO_SIZE_EX_A,
    GetFileVersionInfoSizeExW  => REAL_GET_FILE_VERSION_INFO_SIZE_EX_W,
    GetFileVersionInfoSizeW    => REAL_GET_FILE_VERSION_INFO_SIZE_W,
    GetFileVersionInfoW        => REAL_GET_FILE_VERSION_INFO_W,
    VerFindFileA               => REAL_VER_FIND_FILE_A,
    VerFindFileW               => REAL_VER_FIND_FILE_W,
    VerInstallFileA            => REAL_VER_INSTALL_FILE_A,
    VerInstallFileW            => REAL_VER_INSTALL_FILE_W,
    VerLanguageNameA           => REAL_VER_LANGUAGE_NAME_A,
    VerLanguageNameW           => REAL_VER_LANGUAGE_NAME_W,
    VerQueryValueA             => REAL_VER_QUERY_VALUE_A,
    VerQueryValueW             => REAL_VER_QUERY_VALUE_W,
}

// ============================ Ponto de entrada =============================

/// O CRT chama isto no load. Todo o trabalho é best-effort: erro nunca vira
/// crash, e o retorno é sempre TRUE (E-08).
///
/// `HINSTANCE` é um handle opaco (ponteiro), não algo que desreferenciamos como
/// dado — daí o `allow` do lint que presume deref de ponteiro de argumento.
#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "system" fn DllMain(hinst: HINSTANCE, reason: u32, _reserved: *mut c_void) -> BOOL {
    if reason == DLL_PROCESS_ATTACH {
        // SAFETY: rodamos uma vez, no attach, sob o loader lock. Só tocamos
        // kernel32 (sempre carregada) e o version.dll do System32.
        unsafe { attach(hinst) };
    }
    TRUE
}

unsafe fn attach(hinst: HINSTANCE) {
    // 1. Preenche os 17 slots. Isto vem PRIMEIRO e é incondicional: mesmo que o
    //    version.dll real não carregue, nenhum slot fica em 0 — um reexport não
    //    resolvido cai no fallback (retorna 0), nunca num jmp para NULL (E-08).
    prime_reexports(load_real_version());

    // 2. Daqui para baixo é só o proxy: só em processos do Discord, e só com um
    //    proxy válido no ini. Qualquer saída aqui deixa a DLL como um proxy
    //    transparente do version.dll, sem hook — que é o comportamento correto.
    if !current_process_is_discord() {
        return;
    }
    let Some(proxy) = read_proxy(hinst) else {
        return;
    };

    // 3. Linha de comando com --proxy-server, montada uma vez.
    install_command_line_hook(&proxy);

    // 4. http_proxy/https_proxy para o lado Node.
    install_environment_hook(&proxy);
}

// ============================ Resolução ====================================

/// Carrega o `version.dll` verdadeiro pelo caminho absoluto do System32. Nunca
/// `LoadLibraryW("version.dll")` relativo — carregaria a si mesmo (E-04).
unsafe fn load_real_version() -> Option<*mut c_void> {
    let mut dir = [0u16; 260];
    let len = GetSystemDirectoryW(dir.as_mut_ptr(), dir.len() as u32);
    if len == 0 || len as usize >= dir.len() {
        return None;
    }

    let mut path: Vec<u16> = dir[..len as usize].to_vec();
    path.extend("\\version.dll\0".encode_utf16());

    let handle = LoadLibraryW(path.as_ptr());
    if handle.is_null() {
        None
    } else {
        Some(handle)
    }
}

unsafe fn proc_address(module: *mut c_void, name_z: &[u8]) -> usize {
    match GetProcAddress(module, name_z.as_ptr()) {
        Some(f) => f as usize,
        None => 0,
    }
}

/// Endereço de uma função da kernel32 (sempre carregada; seguro sob loader lock).
unsafe fn kernel32(name_z: &[u8]) -> usize {
    let module = GetModuleHandleW("kernel32.dll\0".encode_utf16().collect::<Vec<_>>().as_ptr());
    if module.is_null() {
        return 0;
    }
    proc_address(module, name_z)
}

// ============================ Identidade ===================================

/// Nome do executável do processo atual, em minúsculas.
unsafe fn current_exe_name() -> Option<String> {
    let mut buf = [0u16; 260];
    let len = GetModuleFileNameW(core::ptr::null_mut(), buf.as_mut_ptr(), buf.len() as u32);
    // `len == buf.len()` = truncado (ERROR_INSUFFICIENT_BUFFER). Tratar como
    // falha: um nome parcial poderia casar com "discord.exe" por engano.
    if len == 0 || len as usize >= buf.len() {
        return None;
    }

    let full = String::from_utf16_lossy(&buf[..len as usize]);
    let name = full.rsplit(['\\', '/']).next().unwrap_or(&full);
    Some(name.to_ascii_lowercase())
}

fn current_process_is_discord() -> bool {
    const NAMES: [&str; 3] = ["discord.exe", "discordcanary.exe", "discordptb.exe"];
    // SAFETY: só lê o caminho do próprio módulo.
    match unsafe { current_exe_name() } {
        Some(name) => NAMES.contains(&name.as_str()),
        None => false,
    }
}

// ============================ Config (ini) =================================

/// Lê `nika-proxy.ini` ao lado desta DLL e devolve o valor de `proxy =`, se
/// houver um não-vazio. Formato de uma seção, uma chave — o mesmo do drover.
unsafe fn read_proxy(hinst: HINSTANCE) -> Option<String> {
    let mut buf = [0u16; 260];
    let len = GetModuleFileNameW(hinst as _, buf.as_mut_ptr(), buf.len() as u32);
    // Truncado (caminho > buffer): melhor não injetar do que ler um ini do
    // diretório errado.
    if len == 0 || len as usize >= buf.len() {
        return None;
    }

    let dll_path = String::from_utf16_lossy(&buf[..len as usize]);
    let cut = dll_path.rfind(['\\', '/'])?;
    let ini_path = format!("{}\\nika-proxy.ini", &dll_path[..cut]);

    parse_proxy(&std::fs::read_to_string(ini_path).ok()?)
}

/// Extrai o valor de `proxy =`, ignorando comentários (`;`/`#`) e espaços.
fn parse_proxy(content: &str) -> Option<String> {
    content.lines().find_map(|line| {
        let line = line.trim();
        if line.starts_with(';') || line.starts_with('#') {
            return None;
        }
        let (key, value) = line.split_once('=')?;
        if !key.trim().eq_ignore_ascii_case("proxy") {
            return None;
        }
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_string())
    })
}

// ============================ Hook: linha de comando ========================

type FnGetCommandLineW = unsafe extern "system" fn() -> *mut u16;

static CMDLINE_BUFFER: OnceLock<Vec<u16>> = OnceLock::new();
static CMDLINE_DETOUR: OnceLock<GenericDetour<FnGetCommandLineW>> = OnceLock::new();
static REAL_GET_COMMAND_LINE_W: AtomicUsize = AtomicUsize::new(0);

unsafe fn install_command_line_hook(proxy: &str) {
    let addr = kernel32(b"GetCommandLineW\0");
    if addr == 0 {
        return;
    }
    REAL_GET_COMMAND_LINE_W.store(addr, Ordering::Relaxed);
    let real: FnGetCommandLineW = core::mem::transmute(addr);

    // Lê a linha original ANTES de hookar, monta o buffer uma vez.
    let original = read_wide(real());
    let Some(patched) = patch_command_line(&original, proxy) else {
        // Já tinha --proxy-server: nada a fazer, deixamos a original passar.
        return;
    };
    if CMDLINE_BUFFER.set(patched).is_err() {
        return;
    }

    // O hook é trivial: devolve sempre o mesmo ponteiro persistente.
    let Ok(detour) = GenericDetour::<FnGetCommandLineW>::new(real, hooked_get_command_line_w)
    else {
        return;
    };
    if detour.enable().is_ok() {
        let _ = CMDLINE_DETOUR.set(detour);
    }
}

unsafe extern "system" fn hooked_get_command_line_w() -> *mut u16 {
    match CMDLINE_BUFFER.get() {
        Some(buf) => buf.as_ptr() as *mut u16,
        // Nunca deveria acontecer (setamos o buffer antes de habilitar), mas se
        // acontecer, devolve a linha real em vez de NULL.
        None => {
            let addr = REAL_GET_COMMAND_LINE_W.load(Ordering::Relaxed);
            if addr == 0 {
                core::ptr::null_mut()
            } else {
                core::mem::transmute::<usize, FnGetCommandLineW>(addr)()
            }
        }
    }
}

/// Acrescenta ` --proxy-server=<proxy>` e termina em NUL. `None` se a linha já
/// tiver `--proxy-server` (não duplicamos; deixamos a original).
fn patch_command_line(original: &str, proxy: &str) -> Option<Vec<u16>> {
    if original.to_ascii_lowercase().contains("--proxy-server") {
        return None;
    }
    let line = format!("{original} --proxy-server={proxy}");
    let mut wide: Vec<u16> = line.encode_utf16().collect();
    wide.push(0);
    Some(wide)
}

// ============================ Hook: env vars ===============================

type FnGetEnvironmentVariableW = unsafe extern "system" fn(*const u16, *mut u16, u32) -> u32;

static ENV_DETOUR: OnceLock<GenericDetour<FnGetEnvironmentVariableW>> = OnceLock::new();
/// Valor a devolver para http_proxy/https_proxy, em UTF-16 com NUL.
static ENV_VALUE: OnceLock<Vec<u16>> = OnceLock::new();

unsafe fn install_environment_hook(proxy: &str) {
    let addr = kernel32(b"GetEnvironmentVariableW\0");
    if addr == 0 {
        return;
    }
    let real: FnGetEnvironmentVariableW = core::mem::transmute(addr);

    let mut value: Vec<u16> = proxy.encode_utf16().collect();
    value.push(0);
    if ENV_VALUE.set(value).is_err() {
        return;
    }

    let Ok(detour) =
        GenericDetour::<FnGetEnvironmentVariableW>::new(real, hooked_get_environment_variable_w)
    else {
        return;
    };
    if detour.enable().is_ok() {
        let _ = ENV_DETOUR.set(detour);
    }
}

unsafe extern "system" fn hooked_get_environment_variable_w(
    name: *const u16,
    buffer: *mut u16,
    size: u32,
) -> u32 {
    if is_proxy_variable(name) {
        if let Some(value) = ENV_VALUE.get() {
            return write_env_value(value, buffer, size);
        }
    }

    // Qualquer outra variável: comportamento original.
    match ENV_DETOUR.get() {
        Some(detour) => detour.call(name, buffer, size),
        None => 0,
    }
}

unsafe fn is_proxy_variable(name: *const u16) -> bool {
    if name.is_null() {
        return false;
    }
    let name = read_wide(name as *mut u16).to_ascii_lowercase();
    name == "http_proxy" || name == "https_proxy"
}

/// Respeita o contrato de `GetEnvironmentVariableW`: se o buffer não cabe,
/// devolve o tamanho necessário **incluindo** o NUL; se cabe, copia e devolve o
/// comprimento **sem** o NUL. `value` já vem com o NUL final.
unsafe fn write_env_value(value: &[u16], buffer: *mut u16, size: u32) -> u32 {
    let needed = value.len() as u32; // inclui NUL
    if buffer.is_null() || size < needed {
        return needed;
    }
    core::ptr::copy_nonoverlapping(value.as_ptr(), buffer, value.len());
    needed - 1 // sem o NUL
}

// ============================ Utilidades ===================================

/// Lê uma string wide terminada em NUL para uma `String`.
unsafe fn read_wide(ptr: *mut u16) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let mut len = 0isize;
    while *ptr.offset(len) != 0 {
        len += 1;
    }
    let slice = core::slice::from_raw_parts(ptr, len as usize);
    String::from_utf16_lossy(slice)
}

#[cfg(test)]
mod tests {
    use super::{parse_proxy, patch_command_line};

    #[test]
    fn reads_proxy_ignoring_comments_and_spacing() {
        let ini = "[drover]\r\n; proxy = http://x\r\n  proxy = http://127.0.0.1:9080 \r\n";
        assert_eq!(parse_proxy(ini).as_deref(), Some("http://127.0.0.1:9080"));
    }

    #[test]
    fn empty_proxy_is_none() {
        assert_eq!(parse_proxy("[drover]\nproxy =\n"), None);
    }

    #[test]
    fn appends_proxy_server_and_terminates_with_nul() {
        let patched = patch_command_line("\"C:\\Discord.exe\"", "http://127.0.0.1:9080")
            .expect("deveria injetar");
        let text = String::from_utf16_lossy(&patched[..patched.len() - 1]);
        assert_eq!(
            text,
            "\"C:\\Discord.exe\" --proxy-server=http://127.0.0.1:9080"
        );
        assert_eq!(*patched.last().unwrap(), 0u16, "precisa terminar em NUL");
    }

    #[test]
    fn does_not_duplicate_an_existing_proxy_server() {
        assert!(patch_command_line(
            "\"C:\\Discord.exe\" --proxy-server=http://other",
            "http://127.0.0.1:9080"
        )
        .is_none());
    }
}
