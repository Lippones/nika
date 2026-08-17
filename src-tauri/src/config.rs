//! Configuração do usuário (RF-24), persistida como JSON.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

pub const DEFAULT_SOCKS_PORT: u16 = 9050;
pub const DEFAULT_HTTP_PORT: u16 = 9080;
pub const DEFAULT_CONTROL_PORT: u16 = 9051;

/// Portas abaixo disso exigem privilégio em vários sistemas e não fazem sentido aqui.
const MIN_PORT: u16 = 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    pub socks_port: u16,
    pub http_port: u16,
    pub control_port: u16,
    /// Subir com o Windows (RF-20). Espelha a chave `HKCU\...\Run`.
    pub autostart: bool,
    /// Conectar sozinho ao abrir (RF-23).
    pub auto_connect: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            socks_port: DEFAULT_SOCKS_PORT,
            http_port: DEFAULT_HTTP_PORT,
            control_port: DEFAULT_CONTROL_PORT,
            autostart: false,
            auto_connect: true,
        }
    }
}

impl Config {
    pub fn validate(&self) -> Result<()> {
        let ports = [
            ("SOCKS", self.socks_port),
            ("HTTP", self.http_port),
            ("Control", self.control_port),
        ];

        for (role, port) in ports {
            if port < MIN_PORT {
                return Err(Error::InvalidConfig(format!(
                    "porta {role} deve ser >= {MIN_PORT}"
                )));
            }
        }

        for (i, (role, port)) in ports.iter().enumerate() {
            if ports[i + 1..].iter().any(|(_, other)| other == port) {
                return Err(Error::InvalidConfig(format!(
                    "porta {port} ({role}) está repetida — as três portas precisam ser distintas"
                )));
            }
        }

        Ok(())
    }

    pub fn socks_url(&self) -> String {
        format!("socks5://127.0.0.1:{}", self.socks_port)
    }

    pub fn http_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.http_port)
    }
}

/// Config compartilhada entre supervisor, comandos e tray.
#[derive(Clone)]
pub struct ConfigStore {
    current: Arc<Mutex<Config>>,
    path: Arc<PathBuf>,
}

impl ConfigStore {
    /// Config corrompida ou ilegível não impede o app de subir: cai no default.
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
        config.validate()?;
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
