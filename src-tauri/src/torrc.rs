//! Geração do `torrc` (§8.2 do PRD).
//!
//! O arquivo é reescrito a cada start a partir da config — nunca editado à mão,
//! nunca preservado entre execuções. Isso mantém uma única fonte da verdade.

use crate::config::Config;
use crate::error::Result;
use crate::paths::Paths;

/// Todo bind é explicitamente `127.0.0.1` (RF-10). Nada de `0.0.0.0`, nunca.
const LOOPBACK: &str = "127.0.0.1";

pub fn render(config: &Config, paths: &Paths) -> String {
    let mut out = String::new();

    let mut line = |text: String| {
        out.push_str(&text);
        out.push('\n');
    };

    line("# Gerado pelo Nika a cada inicialização. Alterações manuais são perdidas.".into());
    line(format!("SocksPort {LOOPBACK}:{}", config.socks_port));
    line(format!("HTTPTunnelPort {LOOPBACK}:{}", config.http_port));
    line(format!("ControlPort {LOOPBACK}:{}", config.control_port));
    // Autenticação por cookie em arquivo: nenhuma senha no binário (RF-12).
    line("CookieAuthentication 1".into());
    line(format!("DataDirectory {}", paths.tor_data.display()));

    if let Some(geoip) = &paths.geoip {
        line(format!("GeoIPFile {}", geoip.display()));
    }
    if let Some(geoip6) = &paths.geoip6 {
        line(format!("GeoIPv6File {}", geoip6.display()));
    }

    // Cliente puro: nunca relay, nunca exit (ver não-objetivos do PRD).
    line("ClientOnly 1".into());
    line("AvoidDiskWrites 1".into());
    line("Log notice stdout".into());

    out
}

pub fn write(config: &Config, paths: &Paths) -> Result<()> {
    std::fs::write(&paths.torrc, render(config, paths))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn paths() -> Paths {
        Paths {
            tor_data: PathBuf::from("/app/tor"),
            torrc: PathBuf::from("/app/torrc"),
            cookie: PathBuf::from("/app/tor/control_auth_cookie"),
            config_file: PathBuf::from("/app/config.json"),
            geoip: None,
            geoip6: None,
        }
    }

    #[test]
    fn binds_only_to_loopback() {
        let rendered = render(&Config::default(), &paths());
        for port_line in rendered.lines().filter(|l| l.contains("Port ")) {
            assert!(
                port_line.contains("127.0.0.1:"),
                "linha sem bind explícito em loopback: {port_line}"
            );
        }
    }

    #[test]
    fn omits_geoip_when_unavailable() {
        let rendered = render(&Config::default(), &paths());
        assert!(!rendered.contains("GeoIPFile"));
    }

    #[test]
    fn uses_cookie_auth() {
        let rendered = render(&Config::default(), &paths());
        assert!(rendered.contains("CookieAuthentication 1"));
    }
}
