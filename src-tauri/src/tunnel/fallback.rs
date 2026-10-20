//! Stub do engine para plataformas sem WinDivert (macOS/Linux, CI).
//!
//! Mantém o crate compilando e os módulos puros testáveis; qualquer tentativa
//! de ligar o túnel devolve [`Error::TunnelUnsupported`].

use std::path::Path;

use super::profile::Profile;
use super::EngineStats;
use crate::error::{Error, Result};

pub struct Engine;

impl Engine {
    pub async fn start(_profile: Profile, _driver_dir: &Path) -> Result<Self> {
        Err(Error::TunnelUnsupported)
    }

    pub fn stats(&self) -> Result<EngineStats> {
        Ok(EngineStats::default())
    }

    pub async fn stop(self) {}
}

pub fn is_elevated() -> bool {
    false
}

pub fn relaunch_elevated(_args: &[String]) -> Result<()> {
    Err(Error::TunnelUnsupported)
}

pub fn wait_for_pid(_pid: u32) {}
