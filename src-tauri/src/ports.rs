//! Checagem de portas antes de subir o tor (RF-05).

use std::net::{Ipv4Addr, TcpListener};

use crate::config::Config;
use crate::error::{Error, Result};

/// Falha cedo, com mensagem acionável, em vez de deixar o tor morrer com
/// "Address already in use" enterrado no log.
pub fn ensure_available(config: &Config) -> Result<()> {
    let ports = [
        ("SOCKS", config.socks_port),
        ("HTTP", config.http_port),
        ("Control", config.control_port),
    ];

    for (role, port) in ports {
        if !is_free(port) {
            return Err(Error::PortInUse {
                role,
                port,
                suggestion: suggestion_for(port),
            });
        }
    }

    Ok(())
}

/// Há uma janela entre esta checagem e o bind do tor; ela existe para dar uma
/// mensagem melhor, não para garantir exclusividade.
fn is_free(port: u16) -> bool {
    TcpListener::bind((Ipv4Addr::LOCALHOST, port)).is_ok()
}

/// O Tor Browser ocupa 9050/9150; pular de dois em dois evita esbarrar nele de novo.
fn suggestion_for(port: u16) -> u16 {
    port.checked_add(2).unwrap_or(port - 2)
}
