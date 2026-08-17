//! Stub para plataformas não-Windows.
//!
//! O alvo do projeto é Windows (macOS/Linux são fase 2), mas manter o crate
//! compilando fora dele permite rodar `cargo check`/`cargo test` em CI Linux.

use tokio::process::{Child, Command};

pub struct JobGuard;

impl JobGuard {
    pub fn new() -> Option<Self> {
        None
    }

    pub fn assign(&self, _child: &Child) {}
}

pub fn prepare_command(_command: &mut Command) {}
