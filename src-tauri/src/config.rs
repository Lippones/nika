//! Configuração do usuário, persistida como JSON.
//!
//! O que é segredo (o convite, com a chave privada) **não** mora aqui: fica em
//! `tunnel.conf` (docs/discord-tunnel.md T-10). Isto é só preferência.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::error::Result;

/// Preferências do túnel (docs/discord-tunnel.md §7.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct TunnelConfig {
    /// Ligar a saída ao abrir o Nika, se houver convite e o processo for
    /// administrador. Sem elevação a faixa só avisa — nada de UAC no boot.
    pub auto_start: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    /// Subir com o Windows. Espelha a chave `HKCU\...\Run`.
    pub autostart: bool,
    pub tunnel: TunnelConfig,
    /// A tela de boas-vindas já foi concluída. `false` no primeiro start (ou
    /// numa config antiga, onde o campo não existe e o `serde(default)` o
    /// preenche): a UI abre no onboarding, não na janela.
    pub onboarded: bool,
}

/// Config compartilhada entre comandos, bandeja e tarefas.
#[derive(Clone)]
pub struct ConfigStore {
    current: Arc<Mutex<Config>>,
    path: Arc<PathBuf>,
}

impl ConfigStore {
    /// Config corrompida ou ilegível não impede o app de subir: cai no default.
    /// Campos de versões anteriores (portas do Tor, proxy do Discord) são
    /// ignorados pelo serde e somem na próxima gravação.
    pub fn load(path: PathBuf) -> Self {
        let current = match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|err| {
                log::warn!("config.json inválido ({err}); usando os valores padrão");
                Config::default()
            }),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Config::default(),
            Err(err) => {
                log::warn!("não consegui ler config.json ({err}); usando os valores padrão");
                Config::default()
            }
        };

        Self {
            current: Arc::new(Mutex::new(current)),
            path: Arc::new(path),
        }
    }

    pub fn get(&self) -> Config {
        self.current.lock().expect("config mutex").clone()
    }

    pub fn set(&self, config: Config) -> Result<()> {
        self.persist(&config)?;
        *self.current.lock().expect("config mutex") = config;
        Ok(())
    }

    /// Grava via arquivo temporário para não deixar um JSON truncado se o
    /// processo morrer no meio da escrita.
    fn persist(&self, config: &Config) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(config)?)?;

        // No Windows `rename` falha se o destino existe.
        match std::fs::remove_file(self.path.as_path()) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(err.into()),
        }
        std::fs::rename(&tmp, self.path.as_path())?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_configs_with_tor_fields_still_load() {
        let old = r#"{"socksPort":9050,"httpPort":9080,"autostart":true,"autoConnect":true,
                     "discord":{"mode":"torHttp"},"onboarded":true}"#;
        let config: Config = serde_json::from_str(old).unwrap();
        assert!(config.autostart);
        assert!(config.onboarded);
        assert!(!config.tunnel.auto_start);
    }
}
