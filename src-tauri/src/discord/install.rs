//! Escrita e remoção dos arquivos dentro das pastas do Discord (RF-30/31).

use std::path::{Path, PathBuf};

use super::{component, discover, AppDir, Install, Mode};
use crate::config::Config;
use crate::error::{Error, Result};

pub const DLL_NAME: &str = "version.dll";
pub const INI_NAME: &str = "nika-proxy.ini";

/// O que o Nika coloca. Na remoção também limpamos os arquivos legados do
/// drover, para quem veio da versão que baixava o binário de terceiro.
pub const FILES: [&str; 2] = [DLL_NAME, INI_NAME];
const LEGACY_FILES: [&str; 2] = ["drover.ini", "drover-packet.bin"];

/// Estado de uma pasta `app-*`, lido do disco. `shim_hash` é o SHA-256 do shim
/// empacotado; uma `version.dll` só conta como nossa se bater com ele.
pub fn inspect(path: PathBuf, shim_hash: Option<&str>) -> AppDir {
    let version = path
        .file_name()
        .and_then(|name| name.to_str())
        .map(discover::version_of)
        .unwrap_or_default();

    let installed = shim_hash
        .is_some_and(|hash| component::sha256_file(&path.join(DLL_NAME)).as_deref() == Some(hash));

    AppDir {
        installed,
        proxy: read_proxy(&path.join(INI_NAME)),
        version,
        path,
    }
}

pub fn render_ini(proxy: &str) -> String {
    format!("[nika]\r\n; gerado pelo Nika — não editar\r\nproxy = {proxy}\r\n")
}

/// Leitor de INI de uma seção e uma chave. Trazer dependência para isto seria
/// desproporcional ao formato de três linhas que o drover usa.
pub fn read_proxy(ini: &Path) -> Option<String> {
    let content = std::fs::read_to_string(ini).ok()?;

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

/// RF-30. Escreve em **todas** as pastas `app-*` (D-04): o Discord pode abrir
/// de uma pasta antiga, e cobrir só a mais nova deixa um buraco silencioso.
pub fn apply(installs: &[Install], mode: Mode, config: &Config, shim: &Path) -> Result<()> {
    let proxy = mode
        .proxy_url(config)
        .ok_or_else(|| Error::InvalidConfig("escolha o modo do proxy do Discord".into()))?;

    if !shim.is_file() {
        return Err(Error::ShimMissing);
    }

    let mut failures = Vec::new();

    for dir in installs.iter().flat_map(|install| &install.app_dirs) {
        let ini = dir.path.join(INI_NAME);
        record(
            &mut failures,
            &ini,
            std::fs::write(&ini, render_ini(&proxy)),
        );

        let target = dir.path.join(DLL_NAME);
        record(&mut failures, &target, std::fs::copy(shim, &target));
    }

    // Sem rollback de propósito: uma pasta com os arquivos certos nunca é pior
    // que uma pasta pela metade, e o próximo scan mostra o estado real.
    finish(failures)
}

/// RF-38: só o endereço muda; a DLL fica onde está.
pub fn rewrite_ini(installs: &[Install], mode: Mode, config: &Config) -> Result<()> {
    let proxy = mode
        .proxy_url(config)
        .ok_or_else(|| Error::InvalidConfig("modo do proxy do Discord desligado".into()))?;

    let mut failures = Vec::new();

    for dir in installs
        .iter()
        .flat_map(|install| &install.app_dirs)
        .filter(|dir| dir.installed)
    {
        let ini = dir.path.join(INI_NAME);
        record(
            &mut failures,
            &ini,
            std::fs::write(&ini, render_ini(&proxy)),
        );
    }

    finish(failures)
}

/// RF-31. Limpa todas as pastas, inclusive as que o usuário tenha instalado
/// por fora — o objetivo é não deixar resíduo nosso na pasta do Discord.
pub fn remove(installs: &[Install]) -> Result<()> {
    let mut failures = Vec::new();

    for dir in installs.iter().flat_map(|install| &install.app_dirs) {
        for name in FILES.iter().chain(LEGACY_FILES.iter()) {
            let path = dir.path.join(name);
            match std::fs::remove_file(&path) {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => record::<()>(&mut failures, &path, Err(err)),
            }
        }
    }

    finish(failures)
}

fn record<T>(failures: &mut Vec<String>, path: &Path, result: std::io::Result<T>) {
    if let Err(err) = result {
        log::warn!("falha em {}: {err}", path.display());
        failures.push(path.display().to_string());
    }
}

fn finish(failures: Vec<String>) -> Result<()> {
    if failures.is_empty() {
        return Ok(());
    }

    Err(Error::DiscordWriteFailed {
        paths: failures.join(", "),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ini_round_trip() {
        let dir = std::env::temp_dir().join("nika-ini-round-trip");
        std::fs::create_dir_all(&dir).unwrap();
        let ini = dir.join(INI_NAME);

        std::fs::write(&ini, render_ini("http://127.0.0.1:9080")).unwrap();
        assert_eq!(read_proxy(&ini).as_deref(), Some("http://127.0.0.1:9080"));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn ignores_comments_and_spacing() {
        let dir = std::env::temp_dir().join("nika-ini-comments");
        std::fs::create_dir_all(&dir).unwrap();
        let ini = dir.join(INI_NAME);

        std::fs::write(
            &ini,
            "[drover]\r\n; proxy = http://127.0.0.1:1080\r\n   proxy   =   socks5://127.0.0.1:9050   \r\n",
        )
        .unwrap();
        assert_eq!(read_proxy(&ini).as_deref(), Some("socks5://127.0.0.1:9050"));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn empty_proxy_reads_as_none() {
        let dir = std::env::temp_dir().join("nika-ini-empty");
        std::fs::create_dir_all(&dir).unwrap();
        let ini = dir.join(INI_NAME);

        std::fs::write(&ini, "[drover]\r\nproxy =\r\n").unwrap();
        assert_eq!(read_proxy(&ini), None);

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
