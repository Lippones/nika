//! Quem é dono de uma porta local, e se esse processo é o Discord (RF-52).
//!
//! A fonte principal é a camada SOCKET do WinDivert, que avisa a cada `bind`.
//! Isto aqui cobre o que ela não vê: sockets abertos **antes** de o túnel
//! ligar (o Discord já estava rodando) e a corrida entre o primeiro pacote e o
//! evento. Para isso lemos as tabelas do IP Helper, as mesmas do `netstat -o`.

use std::ffi::c_void;

use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, GetExtendedUdpTable, TCP_TABLE_OWNER_PID_ALL, UDP_TABLE_OWNER_PID,
};
use windows_sys::Win32::Networking::WinSock::{AF_INET, AF_INET6};
use windows_sys::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
};

use super::packet::Proto;
use crate::discord::Flavor;

const ERROR_INSUFFICIENT_BUFFER: u32 = 122;
const NO_ERROR: u32 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortOwner {
    pub proto: Proto,
    pub port: u16,
    pub pid: u32,
}

/// Nome do executável do processo, em minúsculas, sem o caminho.
/// `None` se o processo já saiu ou não deixa perguntar.
pub fn process_name(pid: u32) -> Option<String> {
    // SAFETY: o handle é fechado em todos os caminhos; o buffer é nosso.
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return None;
        }
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut len);
        CloseHandle(handle);
        if ok == 0 {
            return None;
        }
        let full = String::from_utf16_lossy(&buf[..len as usize]);
        let name = full.rsplit(['\\', '/']).next().unwrap_or(&full);
        Some(name.to_ascii_lowercase())
    }
}

pub fn is_discord(pid: u32) -> bool {
    process_name(pid)
        .map(|name| Flavor::from_exe_name(&name).is_some())
        .unwrap_or(false)
}

/// Todas as portas TCP e UDP com dono, IPv4 e IPv6 (um socket dual-stack só
/// aparece na tabela v6). Custa alguns centenas de microssegundos.
pub fn snapshot() -> Vec<PortOwner> {
    let mut owners = Vec::with_capacity(512);
    for af in [AF_INET, AF_INET6] {
        udp_table(af as u32, &mut owners);
        tcp_table(af as u32, &mut owners);
    }
    owners
}

/// Dono de uma porta específica, por varredura completa.
pub fn lookup(proto: Proto, port: u16) -> Option<u32> {
    snapshot()
        .into_iter()
        .find(|owner| owner.proto == proto && owner.port == port)
        .map(|owner| owner.pid)
}

// Layouts (bytes) das linhas, do `iprtrmib.h`/`tcpmib.h`/`udpmib.h`:
//   MIB_UDPROW_OWNER_PID   12: LocalAddr@0  LocalPort@4  Pid@8
//   MIB_UDP6ROW_OWNER_PID  28: LocalAddr@0  Scope@16  LocalPort@20  Pid@24
//   MIB_TCPROW_OWNER_PID   24: State@0  LocalAddr@4  LocalPort@8  Remote…  Pid@20
//   MIB_TCP6ROW_OWNER_PID  56: LocalAddr@0  Scope@16  LocalPort@20  Remote…  State@48  Pid@52
// O cabeçalho da tabela é um `DWORD dwNumEntries` e as linhas começam em 4.
struct RowLayout {
    size: usize,
    port: usize,
    pid: usize,
}

const UDP4: RowLayout = RowLayout { size: 12, port: 4, pid: 8 };
const UDP6: RowLayout = RowLayout { size: 28, port: 20, pid: 24 };
const TCP4: RowLayout = RowLayout { size: 24, port: 8, pid: 20 };
const TCP6: RowLayout = RowLayout { size: 56, port: 20, pid: 52 };

fn udp_table(af: u32, out: &mut Vec<PortOwner>) {
    let layout = if af == AF_INET as u32 { UDP4 } else { UDP6 };
    // SAFETY: o padrão documentado — primeira chamada mede, segunda preenche.
    let buffer = unsafe {
        grow_and_fill(|ptr, size| {
            GetExtendedUdpTable(ptr, size, 0, af, UDP_TABLE_OWNER_PID, 0)
        })
    };
    if let Some(buffer) = buffer {
        parse_rows(&buffer, &layout, Proto::Udp, out);
    }
}

fn tcp_table(af: u32, out: &mut Vec<PortOwner>) {
    let layout = if af == AF_INET as u32 { TCP4 } else { TCP6 };
    // SAFETY: idem.
    let buffer = unsafe {
        grow_and_fill(|ptr, size| {
            GetExtendedTcpTable(ptr, size, 0, af, TCP_TABLE_OWNER_PID_ALL, 0)
        })
    };
    if let Some(buffer) = buffer {
        parse_rows(&buffer, &layout, Proto::Tcp, out);
    }
}

/// Chama `f` até o buffer caber. Até três tentativas: a tabela pode crescer
/// entre a medição e o preenchimento.
unsafe fn grow_and_fill(f: impl Fn(*mut c_void, *mut u32) -> u32) -> Option<Vec<u8>> {
    let mut size = 0u32;
    let mut buffer: Vec<u8> = Vec::new();
    for _ in 0..3 {
        let ptr = if buffer.is_empty() {
            std::ptr::null_mut()
        } else {
            buffer.as_mut_ptr() as *mut c_void
        };
        match f(ptr, &mut size) {
            NO_ERROR if !buffer.is_empty() => {
                buffer.truncate(size as usize);
                return Some(buffer);
            }
            NO_ERROR | ERROR_INSUFFICIENT_BUFFER => {
                buffer = vec![0u8; size as usize];
            }
            code => {
                log::debug!("tabela de portas indisponível (erro {code})");
                return None;
            }
        }
    }
    None
}

fn parse_rows(buffer: &[u8], layout: &RowLayout, proto: Proto, out: &mut Vec<PortOwner>) {
    if buffer.len() < 4 {
        return;
    }
    let count = u32::from_ne_bytes(buffer[0..4].try_into().expect("4 bytes")) as usize;
    let rows = &buffer[4..];
    for i in 0..count {
        let start = i * layout.size;
        let Some(row) = rows.get(start..start + layout.size) else {
            break;
        };
        // "dwLocalPort: in network byte order" — nos 16 bits baixos do DWORD.
        let raw_port = u32::from_ne_bytes(row[layout.port..layout.port + 4].try_into().expect("4"));
        let port = u16::from_be((raw_port & 0xffff) as u16);
        let pid = u32::from_ne_bytes(row[layout.pid..layout.pid + 4].try_into().expect("4"));
        if port != 0 && pid != 0 {
            out.push(PortOwner { proto, port, pid });
        }
    }
}
