//! Detalhes dependentes de sistema operacional.
//!
//! A superfície é a mesma nos dois lados: [`JobGuard`] amarra o ciclo de vida do
//! `tor` ao do app e [`prepare_command`] ajusta o spawn.

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::{prepare_command, JobGuard};

#[cfg(not(windows))]
mod fallback;
#[cfg(not(windows))]
pub use fallback::{prepare_command, JobGuard};
