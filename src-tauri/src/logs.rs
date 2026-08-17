//! Buffer circular com as últimas linhas de log do tor (RF-17).

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Emitter};

use crate::state::EVENT_LOG;

/// Suficiente para diagnosticar um bootstrap inteiro sem virar vazamento.
const CAPACITY: usize = 500;

#[derive(Clone)]
pub struct LogBuffer {
    lines: Arc<Mutex<VecDeque<String>>>,
    app: AppHandle,
}

impl LogBuffer {
    pub fn new(app: AppHandle) -> Self {
        Self {
            lines: Arc::new(Mutex::new(VecDeque::with_capacity(CAPACITY))),
            app,
        }
    }

    pub fn push(&self, line: impl Into<String>) {
        let line = line.into();
        let line = line.trim_end().to_string();
        if line.is_empty() {
            return;
        }

        {
            let mut lines = self.lines.lock().expect("logs mutex");
            if lines.len() == CAPACITY {
                lines.pop_front();
            }
            lines.push_back(line.clone());
        }

        if let Err(err) = self.app.emit(EVENT_LOG, &line) {
            log::warn!("falha ao emitir {EVENT_LOG}: {err}");
        }
    }

    pub fn snapshot(&self) -> Vec<String> {
        self.lines
            .lock()
            .expect("logs mutex")
            .iter()
            .cloned()
            .collect()
    }
}
