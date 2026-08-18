//! Descoberta das instalações do Discord (RF-27).
//!
//! Só `HKCU` e `%LOCALAPPDATA%`: o Discord instala por usuário, e é isso que
//! mantém a promessa de "sem admin" do PRD §7.

use std::path::{Path, PathBuf};

use super::{install, AppDir, Flavor, Install};

pub fn installs(shim_hash: Option<&str>) -> Vec<Install> {
    Flavor::ALL
        .into_iter()
        .filter_map(|flavor| of_flavor(flavor, shim_hash))
        .collect()
}

fn of_flavor(flavor: Flavor, shim_hash: Option<&str>) -> Option<Install> {
    let mut base_dir: Option<PathBuf> = None;
    let mut app_dirs: Vec<AppDir> = Vec::new();

    for candidate in base_dirs(flavor) {
        let found = app_dirs_in(&candidate, flavor);
        if found.is_empty() {
            continue;
        }

        base_dir.get_or_insert(candidate);
        for path in found {
            if app_dirs.iter().any(|dir| same_path(&dir.path, &path)) {
                continue;
            }
            app_dirs.push(install::inspect(path, shim_hash));
        }
    }

    let base_dir = base_dir?;
    // A mais nova primeiro: é dela que a UI fala e é ela que o Discord abre.
    app_dirs.sort_by_key(|dir| std::cmp::Reverse(version_key(&dir.version)));

    let update_exe = Some(base_dir.join("Update.exe")).filter(|path| path.is_file());

    Some(Install {
        flavor,
        label: flavor.label(),
        exe_name: flavor.exe_name(),
        base_dir,
        update_exe,
        app_dirs,
    })
}

/// Candidatos a diretório base, em ordem de confiança, sem repetição.
fn base_dirs(flavor: Flavor) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();

    let mut push = |dir: PathBuf| {
        if dir.is_dir() && !out.iter().any(|existing| same_path(existing, &dir)) {
            out.push(dir);
        }
    };

    for dir in registry_dirs(flavor) {
        push(dir);
    }

    // Fallback para registro sujo (desinstalação incompleta, perfil migrado).
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        push(PathBuf::from(local).join(flavor.key()));
    }

    out
}

#[cfg(windows)]
fn registry_dirs(flavor: Flavor) -> Vec<PathBuf> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let mut out = Vec::new();

    let uninstall = format!(
        r"Software\Microsoft\Windows\CurrentVersion\Uninstall\{}",
        flavor.key()
    );
    if let Ok(key) = hkcu.open_subkey(uninstall) {
        if let Ok(location) = key.get_value::<String, _>("InstallLocation") {
            if !location.trim().is_empty() {
                out.push(PathBuf::from(location.trim()));
            }
        }
    }

    let command = format!(r"Software\Classes\{}\shell\open\command", flavor.key());
    if let Ok(key) = hkcu.open_subkey(command) {
        if let Ok(value) = key.get_value::<String, _>("") {
            if let Some(base) = base_from_command(&value) {
                out.push(PathBuf::from(base));
            }
        }
    }

    out
}

#[cfg(not(windows))]
fn registry_dirs(_flavor: Flavor) -> Vec<PathBuf> {
    // Fora do Windows não há de onde descobrir; sobra o fallback de ambiente.
    Vec::new()
}

/// Extrai o diretório base do comando registrado para o protocolo `discord://`.
///
/// `"C:\...\Local\Discord\app-1.0.9186\Discord.exe" --url -- "%1"`
/// vira `C:\...\Local\Discord\`.
// Fora do Windows só os testes chamam — o registro não existe lá.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn base_from_command(value: &str) -> Option<String> {
    let path = match value.split_once('"') {
        Some((_, rest)) => rest.split('"').next()?,
        // Sem aspas o valor é o caminho inteiro; é raro, mas custa uma linha.
        None => value.trim(),
    };

    let cut = path.to_ascii_lowercase().rfind("app-")?;
    Some(path[..cut].to_string())
}

fn app_dirs_in(base: &Path, flavor: Flavor) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(base) else {
        return Vec::new();
    };

    entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| is_app_dir(path))
        .filter(|path| path.join(flavor.exe_name()).is_file())
        .collect()
}

fn is_app_dir(path: &Path) -> bool {
    path.is_dir()
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.to_ascii_lowercase().starts_with("app-"))
}

/// `app-1.0.9186` → `1.0.9186`. Nome fora do padrão devolve string vazia, que
/// ordena por último sem quebrar nada.
pub fn version_of(dir_name: &str) -> String {
    dir_name
        .get(4..)
        .filter(|_| dir_name.to_ascii_lowercase().starts_with("app-"))
        .map(|version| {
            version
                .trim_matches(|c: char| !c.is_ascii_digit() && c != '.')
                .to_string()
        })
        .unwrap_or_default()
}

/// Comparação numérica, não lexicográfica: `1.0.9186` é maior que `1.0.972`.
pub fn version_key(version: &str) -> Vec<u32> {
    version
        .split('.')
        .map(|part| part.parse().unwrap_or(0))
        .collect()
}

/// O Windows não diferencia caixa em caminho, e o registro devolve o que o
/// instalador gravou — comparar cru duplicaria a mesma pasta.
fn same_path(a: &Path, b: &Path) -> bool {
    let normalize = |path: &Path| {
        path.to_string_lossy()
            .trim_end_matches(['\\', '/'])
            .to_ascii_lowercase()
    };
    normalize(a) == normalize(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_the_base_dir_from_the_protocol_command() {
        let command =
            r#""C:\Users\f\AppData\Local\Discord\app-1.0.9186\Discord.exe" --url -- "%1""#;
        assert_eq!(
            base_from_command(command).as_deref(),
            Some(r"C:\Users\f\AppData\Local\Discord\")
        );
    }

    #[test]
    fn ignores_a_command_without_an_app_folder() {
        assert_eq!(base_from_command(r#""C:\Windows\explorer.exe""#), None);
    }

    #[test]
    fn orders_versions_as_numbers() {
        let mut versions = ["1.0.9", "1.0.9186", "1.0.972"];
        versions.sort_by_key(|version| std::cmp::Reverse(version_key(version)));
        assert_eq!(versions, ["1.0.9186", "1.0.972", "1.0.9"]);
    }

    #[test]
    fn reads_the_version_from_the_folder_name() {
        assert_eq!(version_of("app-1.0.9186"), "1.0.9186");
        assert_eq!(version_of("modules"), "");
    }
}
