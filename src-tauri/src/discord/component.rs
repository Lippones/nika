//! O componente do proxy do Discord: o `version.dll` do Nika (o shim).
//!
//! Diferente da abordagem anterior (baixar o binário de terceiro do drover),
//! o shim é **nosso** (crate `discord-shim`, ver docs/discord-dll.md) e vai
//! empacotado como recurso. Aqui não há download nem rede: só localizar o
//! recurso e comparar hashes para saber se a cópia instalada numa pasta do
//! Discord é a nossa.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Manager};

use super::Component;

/// Versão exibida na UI. O shim acompanha a versão do app.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Caminho do shim empacotado, ou `None` num build que não o incluiu (dev/Linux,
/// ou build Tor-only). Espelha o tratamento opcional de `geoip` em `paths`.
pub fn shim_path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .resolve("resources/discord/version.dll", BaseDirectory::Resource)
        .ok()
        .filter(|path| path.is_file())
}

/// Ready se o shim foi empacotado; Missing caso contrário. Não há `Corrupt`: o
/// shim é gerado por nós no build, não baixado de terceiro.
pub fn state(app: &AppHandle) -> Component {
    if shim_path(app).is_some() {
        Component::Ready
    } else {
        Component::Missing
    }
}

/// SHA-256 de um arquivo, em hex; `None` se não der para ler.
pub fn sha256_file(path: &Path) -> Option<String> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher).ok()?;
    Some(hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_a_file_in_the_pinned_hex_format() {
        let dir = std::env::temp_dir().join("nika-shim-hash");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("empty");
        std::fs::write(&file, b"").unwrap();

        assert_eq!(
            sha256_file(&file).as_deref(),
            Some("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn same_bytes_hash_equal_different_bytes_differ() {
        let dir = std::env::temp_dir().join("nika-shim-hash-eq");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let a = dir.join("a");
        let b = dir.join("b");
        let c = dir.join("c");
        std::fs::write(&a, b"shim bytes").unwrap();
        std::fs::write(&b, b"shim bytes").unwrap();
        std::fs::write(&c, b"outra dll").unwrap();

        assert_eq!(sha256_file(&a), sha256_file(&b));
        assert_ne!(sha256_file(&a), sha256_file(&c));

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
