//! FFI mínimo e dinâmico ao `WinDivert.dll` (T-07).
//!
//! Carregamos a DLL com `LoadLibraryW` a partir da pasta de recursos e
//! resolvemos cinco funções. Sem import estático e sem `.lib`: o Nika sobe
//! mesmo sem o driver, e o crate compila fora do Windows. A DLL instala o
//! `WinDivert64.sys` sozinha, a partir do próprio diretório, na primeira
//! `WinDivertOpen` — por isso os dois arquivos ficam juntos.
//!
//! Layout do `WINDIVERT_ADDRESS` (WinDivert 2.2, `include/windivert.h`):
//!
//! ```text
//! 0   INT64  Timestamp
//! 8   UINT32 Layer:8 | Event:8 | Sniffed:1 | Outbound:1 | Loopback:1 | Impostor:1
//!            | IPv6:1 | IPChecksum:1 | TCPChecksum:1 | UDPChecksum:1 | Reserved1:8
//! 12  UINT32 Reserved2
//! 16  union { Network; Flow; Socket; Reflect; UINT8 Reserved3[64] }
//! ```

use std::ffi::{c_char, c_void, CString};
use std::path::Path;
use std::sync::Arc;

use windows_sys::Win32::Foundation::{
    FreeLibrary, GetLastError, BOOL, HANDLE, HMODULE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

use crate::error::{Error, Result};

pub const LAYER_NETWORK: u32 = 0;
pub const LAYER_SOCKET: u32 = 3;

pub const FLAG_SNIFF: u64 = 0x0001;
pub const FLAG_RECV_ONLY: u64 = 0x0004;

const SHUTDOWN_BOTH: u32 = 0x3;

pub mod event {
    pub const SOCKET_BIND: u8 = 3;
    pub const SOCKET_CONNECT: u8 = 4;
    pub const SOCKET_LISTEN: u8 = 5;
    pub const SOCKET_ACCEPT: u8 = 6;
    pub const SOCKET_CLOSE: u8 = 7;
}

// Códigos do Windows que o WinDivert devolve e que valem uma frase própria.
const ERROR_FILE_NOT_FOUND: u32 = 2;
const ERROR_ACCESS_DENIED: u32 = 5;
const ERROR_INVALID_PARAMETER: u32 = 87;
const ERROR_INSUFFICIENT_BUFFER: u32 = 122;
const ERROR_NO_DATA: u32 = 232;
const ERROR_INVALID_IMAGE_HASH: u32 = 577;
const ERROR_SERVICE_DOES_NOT_EXIST: u32 = 1060;
const EPT_S_NOT_REGISTERED: u32 = 1753;

type FnOpen = unsafe extern "system" fn(*const c_char, u32, i16, u64) -> HANDLE;
type FnRecv = unsafe extern "system" fn(HANDLE, *mut c_void, u32, *mut u32, *mut Address) -> BOOL;
type FnSend =
    unsafe extern "system" fn(HANDLE, *const c_void, u32, *mut u32, *const Address) -> BOOL;
type FnShutdown = unsafe extern "system" fn(HANDLE, u32) -> BOOL;
type FnClose = unsafe extern "system" fn(HANDLE) -> BOOL;

/// Endereço de um pacote/evento. Espelho byte a byte do `WINDIVERT_ADDRESS`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Address {
    pub timestamp: i64,
    bits: u32,
    reserved2: u32,
    data: [u8; 64],
}

const _: () = assert!(std::mem::size_of::<Address>() == 80);

#[derive(Debug, Clone, Copy)]
pub struct SocketEvent {
    pub process_id: u32,
    pub local_port: u16,
    pub protocol: u8,
}

impl Address {
    pub fn zeroed() -> Self {
        Self {
            timestamp: 0,
            bits: 0,
            reserved2: 0,
            data: [0; 64],
        }
    }

    fn bit(&self, index: u32) -> bool {
        self.bits & (1 << index) != 0
    }

    fn set_bit(&mut self, index: u32, value: bool) {
        if value {
            self.bits |= 1 << index;
        } else {
            self.bits &= !(1 << index);
        }
    }

    pub fn event(&self) -> u8 {
        ((self.bits >> 8) & 0xff) as u8
    }
    pub fn outbound(&self) -> bool {
        self.bit(17)
    }
    pub fn loopback(&self) -> bool {
        self.bit(18)
    }
    pub fn ipv6(&self) -> bool {
        self.bit(20)
    }
    /// Os três bits de checksum dizem se o pacote **já** tem checksum válido.
    /// Na saída costumam vir em 0 quando a placa faria o cálculo (offload).
    pub fn ip_checksum_valid(&self) -> bool {
        self.bit(21)
    }
    pub fn tcp_checksum_valid(&self) -> bool {
        self.bit(22)
    }
    pub fn udp_checksum_valid(&self) -> bool {
        self.bit(23)
    }

    /// Endereço para injetar um pacote de **entrada** pela interface dada,
    /// como se tivesse chegado da rede. Checksums marcados como válidos: nós
    /// os corrigimos antes.
    pub fn inbound(if_idx: u32, sub_if_idx: u32) -> Self {
        let mut addr = Self::zeroed();
        addr.bits = LAYER_NETWORK & 0xff;
        addr.set_bit(17, false);
        addr.set_bit(21, true);
        addr.set_bit(22, true);
        addr.set_bit(23, true);
        addr.data[0..4].copy_from_slice(&if_idx.to_ne_bytes());
        addr.data[4..8].copy_from_slice(&sub_if_idx.to_ne_bytes());
        addr
    }

    /// `(IfIdx, SubIfIdx)` de um pacote da camada NETWORK.
    pub fn network(&self) -> (u32, u32) {
        (
            u32::from_ne_bytes(self.data[0..4].try_into().expect("4 bytes")),
            u32::from_ne_bytes(self.data[4..8].try_into().expect("4 bytes")),
        )
    }

    /// Dados de um evento da camada SOCKET.
    ///
    /// Portas em ordem de bytes do host, como os exemplos do próprio WinDivert
    /// (`socketdump.c`) as imprimem sem `ntohs`. Se o gate 1 de
    /// docs/discord-tunnel.md §13 mostrar portas invertidas, é aqui que se troca.
    pub fn socket(&self) -> SocketEvent {
        SocketEvent {
            process_id: u32::from_ne_bytes(self.data[16..20].try_into().expect("4 bytes")),
            local_port: u16::from_ne_bytes(self.data[52..54].try_into().expect("2 bytes")),
            protocol: self.data[56],
        }
    }
}

/// As funções resolvidas. Vive enquanto houver um [`Handle`].
pub struct Api {
    module: HMODULE,
    open: FnOpen,
    recv: FnRecv,
    send: FnSend,
    shutdown: FnShutdown,
    close: FnClose,
}

// Ponteiros de função e um HMODULE: seguros de compartilhar entre threads.
unsafe impl Send for Api {}
unsafe impl Sync for Api {}

impl Api {
    /// Carrega `WinDivert.dll` de `dir`. Falhar aqui é "componente ausente ou
    /// corrompido", não falta de permissão — a DLL carrega sem admin; é o
    /// `WinDivertOpen` que exige.
    pub fn load(dir: &Path) -> Result<Arc<Self>> {
        let dll = dir.join("WinDivert.dll");
        let wide: Vec<u16> = dll
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        // SAFETY: caminho NUL-terminado; o módulo é liberado no Drop.
        let module = unsafe { LoadLibraryW(wide.as_ptr()) };
        if module.is_null() {
            return Err(Error::Divert(format!(
                "não consegui carregar {} (erro {})",
                dll.display(),
                unsafe { GetLastError() }
            )));
        }

        // SAFETY: `module` é válido; cada símbolo é transmutado para a
        // assinatura documentada no windivert.h.
        unsafe {
            let sym = |name: &[u8]| -> Result<*const c_void> {
                let ptr = GetProcAddress(module, name.as_ptr());
                match ptr {
                    Some(f) => Ok(f as *const c_void),
                    None => Err(Error::Divert(format!(
                        "WinDivert.dll sem {}",
                        String::from_utf8_lossy(&name[..name.len() - 1])
                    ))),
                }
            };
            let api = Self {
                module,
                open: std::mem::transmute::<*const c_void, FnOpen>(sym(b"WinDivertOpen\0")?),
                recv: std::mem::transmute::<*const c_void, FnRecv>(sym(b"WinDivertRecv\0")?),
                send: std::mem::transmute::<*const c_void, FnSend>(sym(b"WinDivertSend\0")?),
                shutdown: std::mem::transmute::<*const c_void, FnShutdown>(sym(
                    b"WinDivertShutdown\0",
                )?),
                close: std::mem::transmute::<*const c_void, FnClose>(sym(b"WinDivertClose\0")?),
            };
            Ok(Arc::new(api))
        }
    }

    pub fn open(self: &Arc<Self>, filter: &str, layer: u32, flags: u64) -> Result<Handle> {
        let filter_c = CString::new(filter).map_err(|_| Error::Divert("filtro com NUL".into()))?;
        // SAFETY: string NUL-terminada; o handle é fechado no Drop.
        let raw = unsafe { (self.open)(filter_c.as_ptr(), layer, 0, flags) };
        if raw == INVALID_HANDLE_VALUE || raw.is_null() {
            return Err(open_error(unsafe { GetLastError() }, filter));
        }
        Ok(Handle {
            raw,
            api: self.clone(),
        })
    }
}

impl Drop for Api {
    fn drop(&mut self) {
        // SAFETY: o módulo foi carregado por nós e nenhum Handle o referencia mais.
        unsafe { FreeLibrary(self.module) };
    }
}

fn open_error(code: u32, filter: &str) -> Error {
    match code {
        ERROR_ACCESS_DENIED => Error::NeedsElevation,
        ERROR_FILE_NOT_FOUND => {
            Error::Divert("WinDivert64.sys não está ao lado da WinDivert.dll".into())
        }
        ERROR_INVALID_PARAMETER => Error::Divert(format!("filtro inválido: {filter}")),
        ERROR_INVALID_IMAGE_HASH => {
            Error::Divert("o Windows recusou a assinatura do driver (WinDivert64.sys)".into())
        }
        ERROR_SERVICE_DOES_NOT_EXIST => {
            Error::Divert("o serviço do driver sumiu durante a instalação; tente de novo".into())
        }
        EPT_S_NOT_REGISTERED => Error::Divert(
            "o serviço Base Filtering Engine (BFE) está parado; o driver depende dele".into(),
        ),
        other => Error::Divert(format!("WinDivertOpen falhou (erro {other})")),
    }
}

/// Um handle aberto. Thread-safe: o WinDivert permite `Recv` e `Send`
/// concorrentes no mesmo handle, e `Shutdown` de outra thread é a forma
/// documentada de acordar um `Recv` bloqueado.
pub struct Handle {
    raw: HANDLE,
    api: Arc<Api>,
}

unsafe impl Send for Handle {}
unsafe impl Sync for Handle {}

impl Handle {
    /// Próximo pacote (camada NETWORK). `Ok(None)` = handle encerrado.
    pub fn recv(&self, buf: &mut [u8]) -> Result<Option<(usize, Address)>> {
        let mut len = 0u32;
        let mut addr = Address::zeroed();
        // SAFETY: buffer e addr são nossos e vivem durante a chamada.
        let ok = unsafe {
            (self.api.recv)(
                self.raw,
                buf.as_mut_ptr() as *mut c_void,
                buf.len() as u32,
                &mut len,
                &mut addr,
            )
        };
        if ok == 0 {
            return match unsafe { GetLastError() } {
                ERROR_NO_DATA => Ok(None),
                // Pacote maior que o buffer: foi descartado pelo driver; seguir.
                ERROR_INSUFFICIENT_BUFFER => Ok(Some((0, addr))),
                code => Err(Error::Divert(format!("WinDivertRecv falhou (erro {code})"))),
            };
        }
        Ok(Some((len as usize, addr)))
    }

    /// Próximo evento (camada SOCKET): não há pacote, só o endereço.
    pub fn recv_event(&self) -> Result<Option<Address>> {
        let mut addr = Address::zeroed();
        // SAFETY: a camada SOCKET aceita buffer nulo com tamanho zero.
        let ok = unsafe {
            (self.api.recv)(
                self.raw,
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                &mut addr,
            )
        };
        if ok == 0 {
            return match unsafe { GetLastError() } {
                ERROR_NO_DATA => Ok(None),
                ERROR_INSUFFICIENT_BUFFER => Ok(Some(addr)),
                code => Err(Error::Divert(format!(
                    "WinDivertRecv (socket) falhou (erro {code})"
                ))),
            };
        }
        Ok(Some(addr))
    }

    /// Injeta (ou devolve) um pacote. Falha aqui é por pacote, nunca fatal:
    /// é o que uma placa faria ao descartar.
    pub fn send(&self, packet: &[u8], addr: &Address) {
        let mut sent = 0u32;
        // SAFETY: buffer e addr válidos durante a chamada.
        let ok = unsafe {
            (self.api.send)(
                self.raw,
                packet.as_ptr() as *const c_void,
                packet.len() as u32,
                &mut sent,
                addr,
            )
        };
        if ok == 0 {
            log::debug!("WinDivertSend falhou (erro {})", unsafe { GetLastError() });
        }
    }

    /// Acorda quem estiver em `recv` e faz as próximas chamadas devolverem
    /// `Ok(None)`. Idempotente.
    pub fn shutdown(&self) {
        // SAFETY: handle válido até o Drop.
        unsafe { (self.api.shutdown)(self.raw, SHUTDOWN_BOTH) };
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: fechamos uma única vez; o último handle descarrega o driver.
        unsafe { (self.api.close)(self.raw) };
    }
}

use std::os::windows::ffi::OsStrExt;
