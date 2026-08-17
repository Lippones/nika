//! Supervisor do processo do Tor (RF-01, RF-02, RF-03).
//!
//! É um ator: uma única tarefa é dona do `Child`, do contador de tentativas e
//! dos prazos. Todo mundo fala com ele por mensagem, então não existe estado
//! mutável compartilhado e não há como duas partes do app subirem dois `tor.exe`.

use std::ops::ControlFlow;
use std::process::{ExitStatus, Stdio};
use std::time::Duration;

use tauri::AppHandle;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;

use crate::config::ConfigStore;
use crate::control::{self, AsyncEvent, Connection};
use crate::error::{Error, Result};
use crate::logs::LogBuffer;
use crate::paths::{self, Paths};
use crate::platform::{self, JobGuard};
use crate::ports;
use crate::state::{ControlSlot, Phase, StatusStore};
use crate::torrc;

/// RF-03: no máximo 5 tentativas, com backoff exponencial de 1s a 30s.
const MAX_RESTARTS: u32 = 5;
const BACKOFF_BASE: Duration = Duration::from_secs(1);
const BACKOFF_CAP: Duration = Duration::from_secs(30);

/// §9 do PRD: bootstrap travado por 60s vira estado de erro acionável.
const BOOTSTRAP_TIMEOUT: Duration = Duration::from_secs(60);
const CONTROL_TIMEOUT: Duration = Duration::from_secs(20);
const SHUTDOWN_GRACE: Duration = Duration::from_secs(3);

const INBOX_SIZE: usize = 16;

enum Message {
    Start(oneshot::Sender<Result<()>>),
    Stop(oneshot::Sender<()>),
    Shutdown(oneshot::Sender<()>),
    /// Resultado do handshake com o ControlPort, que roda fora do laço.
    ControlReady(u64, Box<Result<Connection>>),
    Event(u64, AsyncEvent),
    ControlLost(u64),
}

/// Ponta pública do supervisor. Clonável e barata.
#[derive(Clone)]
pub struct SupervisorHandle {
    inbox: mpsc::Sender<Message>,
}

impl SupervisorHandle {
    /// Sobe o tor. O `Result` cobre só a largada (portas, torrc, spawn); o
    /// progresso do bootstrap chega pelo estado.
    pub async fn start(&self) -> Result<()> {
        self.request(Message::Start).await?
    }

    pub async fn stop(&self) -> Result<()> {
        self.request(Message::Stop).await
    }

    /// Encerra o tor e a própria tarefa. Best-effort e idempotente: a saída do
    /// app chama isso de novo depois do menu "Sair" já ter chamado.
    pub async fn shutdown(&self) {
        if let Err(err) = self.request(Message::Shutdown).await {
            log::debug!("supervisor já estava encerrado: {err}");
        }
    }

    async fn request<T>(&self, build: impl FnOnce(oneshot::Sender<T>) -> Message) -> Result<T> {
        let (responder, response) = oneshot::channel();
        self.inbox
            .send(build(responder))
            .await
            .map_err(|_| supervisor_gone())?;
        response.await.map_err(|_| supervisor_gone())
    }
}

fn supervisor_gone() -> Error {
    Error::other("o supervisor do Tor não está mais rodando")
}

pub fn spawn(
    app: AppHandle,
    config: ConfigStore,
    status: StatusStore,
    logs: LogBuffer,
    control: ControlSlot,
) -> SupervisorHandle {
    let (sender, receiver) = mpsc::channel(INBOX_SIZE);

    let supervisor = Supervisor {
        app,
        config,
        status,
        logs,
        control,
        inbox: sender.clone(),
        job: JobGuard::new(),
        child: None,
        generation: 0,
        attempt: 0,
        wanted: false,
        retry_at: None,
        bootstrap_deadline: None,
    };

    tauri::async_runtime::spawn(supervisor.run(receiver));

    SupervisorHandle { inbox: sender }
}

struct Supervisor {
    app: AppHandle,
    config: ConfigStore,
    status: StatusStore,
    logs: LogBuffer,
    control: ControlSlot,
    inbox: mpsc::Sender<Message>,

    /// Mantido vivo pelo tempo do app: é o fechamento dele que mata o tor
    /// caso o app morra sem passar por aqui.
    job: Option<JobGuard>,
    child: Option<Child>,

    /// Identifica a execução atual do tor. Mensagens de execuções anteriores
    /// (handshake lento, eventos atrasados) são descartadas pela comparação.
    generation: u64,
    attempt: u32,
    /// O usuário quer o tor no ar? Distingue "parou porque pedi" de "caiu".
    wanted: bool,
    retry_at: Option<Instant>,
    bootstrap_deadline: Option<Instant>,
}

impl Supervisor {
    async fn run(mut self, mut inbox: mpsc::Receiver<Message>) {
        loop {
            tokio::select! {
                message = inbox.recv() => match message {
                    Some(message) => {
                        if self.handle(message).await.is_break() {
                            return;
                        }
                    }
                    None => return,
                },
                exit = child_exit(&mut self.child) => self.on_child_exit(exit).await,
                () = deadline(self.retry_at) => {
                    self.retry_at = None;
                    self.relaunch().await;
                }
                () = deadline(self.bootstrap_deadline) => {
                    self.bootstrap_deadline = None;
                    self.on_bootstrap_timeout();
                }
            }
        }
    }

    async fn handle(&mut self, message: Message) -> ControlFlow<()> {
        match message {
            Message::Start(ack) => {
                self.wanted = true;
                self.attempt = 0;
                self.retry_at = None;

                let result = self.launch().await;
                if let Err(err) = &result {
                    self.wanted = false;
                    self.fail(err.to_string());
                }
                let _ = ack.send(result);
            }

            Message::Stop(ack) => {
                self.wanted = false;
                self.attempt = 0;
                self.retry_at = None;
                self.terminate().await;
                self.status.reset(Phase::Stopped, "desconectado");
                let _ = ack.send(());
            }

            Message::Shutdown(ack) => {
                self.wanted = false;
                self.terminate().await;
                let _ = ack.send(());
                return ControlFlow::Break(());
            }

            Message::ControlReady(generation, result) => {
                self.on_control_ready(generation, *result).await;
            }

            Message::Event(generation, event) => {
                if generation == self.generation {
                    self.on_event(event);
                }
            }

            Message::ControlLost(generation) => {
                if generation == self.generation && self.wanted {
                    self.retry("a conexão com o ControlPort caiu".into()).await;
                }
            }
        }

        ControlFlow::Continue(())
    }

    /// Prepara o ambiente e faz o spawn. Erros aqui são de largada e chegam ao
    /// usuário direto (porta ocupada, binário ausente, config inválida).
    async fn launch(&mut self) -> Result<()> {
        self.terminate().await;

        let config = self.config.get();
        config.validate()?;

        let paths = Paths::resolve(&self.app)?;
        paths.ensure_dirs()?;
        ports::ensure_available(&config)?;
        torrc::write(&config, &paths)?;

        let binary = paths::tor_binary(&self.app)?;

        self.status.update(|status| {
            status.phase = Phase::Starting;
            status.bootstrap = 0;
            status.summary = String::from("iniciando o tor");
            status.error = None;
        });

        let mut command = Command::new(&binary);
        command
            .arg("-f")
            .arg(&paths.torrc)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // Rede de segurança para saídas limpas; o Job Object cobre o resto.
            .kill_on_drop(true);
        platform::prepare_command(&mut command);

        let mut child = command.spawn()?;
        if let Some(job) = &self.job {
            job.assign(&child);
        }

        if let Some(stdout) = child.stdout.take() {
            pipe_to_logs(stdout, self.logs.clone());
        }
        if let Some(stderr) = child.stderr.take() {
            pipe_to_logs(stderr, self.logs.clone());
        }

        self.generation = self.generation.wrapping_add(1);
        self.child = Some(child);
        self.bootstrap_deadline = Some(Instant::now() + BOOTSTRAP_TIMEOUT);
        self.logs
            .push(format!("[nika] tor iniciado ({})", binary.display()));

        // O handshake pode levar segundos; fazê-lo aqui travaria o laço e
        // deixaria o app surdo a um "Desconectar" no meio do caminho.
        let generation = self.generation;
        let inbox = self.inbox.clone();
        let cookie = paths.cookie.clone();
        let control_port = config.control_port;
        tokio::spawn(async move {
            let result = control::establish(control_port, &cookie, CONTROL_TIMEOUT).await;
            let _ = inbox
                .send(Message::ControlReady(generation, Box::new(result)))
                .await;
        });

        Ok(())
    }

    async fn relaunch(&mut self) {
        if !self.wanted {
            return;
        }

        if let Err(err) = self.launch().await {
            self.retry(err.to_string()).await;
        }
    }

    async fn on_control_ready(&mut self, generation: u64, result: Result<Connection>) {
        if generation != self.generation {
            // Sobra de uma execução anterior.
            return;
        }

        let (client, events) = match result {
            Ok(connection) => connection,
            Err(err) => {
                self.retry(format!("não consegui falar com o ControlPort: {err}"))
                    .await;
                return;
            }
        };

        self.control.set(Some(client.clone()));
        self.forward_events(generation, events);
        self.status.update(|status| {
            status.phase = Phase::Bootstrapping;
            status.summary = String::from("autenticado no ControlPort");
        });

        // O bootstrap pode já ter avançado antes de assinarmos os eventos.
        match client.get_info("status/bootstrap-phase").await {
            Ok(line) => {
                if let Some(event) = AsyncEvent::from_line(&line) {
                    self.on_event(event);
                }
            }
            Err(err) => log::warn!("não consegui ler o estado inicial do bootstrap: {err}"),
        }
    }

    /// Traz os eventos do ControlPort para dentro do laço, já etiquetados com a
    /// execução a que pertencem.
    fn forward_events(&self, generation: u64, mut events: mpsc::Receiver<AsyncEvent>) {
        let inbox = self.inbox.clone();

        tokio::spawn(async move {
            while let Some(event) = events.recv().await {
                if inbox.send(Message::Event(generation, event)).await.is_err() {
                    return;
                }
            }
            let _ = inbox.send(Message::ControlLost(generation)).await;
        });
    }

    fn on_event(&mut self, event: AsyncEvent) {
        let AsyncEvent::Bootstrap {
            progress, summary, ..
        } = event
        else {
            return;
        };

        let connected = progress >= 100;
        if connected {
            self.bootstrap_deadline = None;
            self.attempt = 0;
        }

        self.status.update(move |status| {
            status.phase = if connected {
                Phase::Connected
            } else {
                Phase::Bootstrapping
            };
            status.bootstrap = progress;
            if !summary.is_empty() {
                status.summary = summary;
            }
            if connected {
                status.error = None;
            }
        });
    }

    async fn on_child_exit(&mut self, exit: std::io::Result<ExitStatus>) {
        self.child = None;
        self.control.set(None);
        self.bootstrap_deadline = None;

        let reason = match exit {
            Ok(status) => format!("o processo do tor encerrou ({status})"),
            Err(err) => format!("perdi o processo do tor ({err})"),
        };

        if !self.wanted {
            self.status.reset(Phase::Stopped, "desconectado");
            return;
        }

        self.retry(reason).await;
    }

    fn on_bootstrap_timeout(&mut self) {
        // O processo continua vivo de propósito: se o bootstrap destravar
        // sozinho, o próximo evento devolve o estado para "conectado".
        let progress = self.status.get().bootstrap;
        self.fail(format!(
            "o bootstrap travou em {progress}% por {}s — a rede pode estar bloqueando o Tor. \
             Tente outra rede ou aguarde; o Tor continua tentando.",
            BOOTSTRAP_TIMEOUT.as_secs()
        ));
    }

    /// Agenda nova tentativa com backoff, ou desiste depois do limite.
    async fn retry(&mut self, reason: String) {
        self.terminate().await;
        self.logs.push(format!("[nika] {reason}"));

        if self.attempt >= MAX_RESTARTS {
            self.wanted = false;
            self.fail(format!(
                "{reason}. Desisti depois de {MAX_RESTARTS} tentativas."
            ));
            return;
        }

        self.attempt += 1;
        let attempt = self.attempt;
        let delay = backoff(attempt);
        self.retry_at = Some(Instant::now() + delay);

        self.status.update(move |status| {
            status.phase = Phase::Retrying;
            status.attempt = attempt;
            status.summary = format!(
                "reiniciando em {}s (tentativa {attempt}/{MAX_RESTARTS})",
                delay.as_secs()
            );
            status.error = Some(reason);
        });
    }

    fn fail(&self, message: String) {
        self.logs.push(format!("[nika] {message}"));
        self.status.update(move |status| {
            status.phase = Phase::Failed;
            status.summary = String::from("falhou");
            status.error = Some(message);
        });
    }

    /// Derruba o tor atual. Idempotente.
    async fn terminate(&mut self) {
        // Invalida handshakes e eventos em voo da execução que está morrendo.
        self.generation = self.generation.wrapping_add(1);
        self.bootstrap_deadline = None;

        if let Some(client) = self.control.take() {
            let _ = tokio::time::timeout(SHUTDOWN_GRACE, client.signal("HALT")).await;
        }

        let Some(mut child) = self.child.take() else {
            return;
        };

        if tokio::time::timeout(SHUTDOWN_GRACE, child.wait())
            .await
            .is_err()
        {
            log::warn!("o tor ignorou SIGNAL HALT; encerrando à força");
            let _ = child.kill().await;
        }
    }
}

/// 1s, 2s, 4s, 8s, 16s — limitado a [`BACKOFF_CAP`].
fn backoff(attempt: u32) -> Duration {
    let factor = 1u32 << attempt.saturating_sub(1).min(5);
    BACKOFF_BASE.saturating_mul(factor).min(BACKOFF_CAP)
}

/// Espera o processo morrer; se não há processo, nunca resolve — o que desabilita
/// o braço correspondente do `select!`.
async fn child_exit(child: &mut Option<Child>) -> std::io::Result<ExitStatus> {
    match child.as_mut() {
        Some(child) => child.wait().await,
        None => std::future::pending().await,
    }
}

/// Idem para prazos opcionais.
async fn deadline(at: Option<Instant>) {
    match at {
        Some(at) => tokio::time::sleep_until(at).await,
        None => std::future::pending().await,
    }
}

fn pipe_to_logs<R>(pipe: R, logs: LogBuffer)
where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut lines = BufReader::new(pipe).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            logs.push(line);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_grows_and_is_capped() {
        assert_eq!(backoff(1), Duration::from_secs(1));
        assert_eq!(backoff(2), Duration::from_secs(2));
        assert_eq!(backoff(5), Duration::from_secs(16));
        assert_eq!(backoff(50), BACKOFF_CAP);
    }
}
