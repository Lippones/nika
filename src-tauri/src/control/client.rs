//! Cliente do ControlPort.
//!
//! A conexão é dividida em duas tarefas que não compartilham nada além da fila
//! de respostas pendentes:
//!
//! - a **escritora** consome os comandos enfileirados e os manda para o tor;
//! - a **leitora** monta respostas linha a linha e roteia cada uma: eventos
//!   assíncronos (650) vão para o canal de eventos, o resto responde ao comando
//!   mais antigo pendente (o tor responde em ordem).
//!
//! Assim nenhum `select!` precisa ser cancel-safe e o cliente pode ser clonado
//! livremente entre comandos do Tauri e o supervisor.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot};

use super::events::AsyncEvent;
use super::protocol::{Reply, ReplyParser};
use crate::error::{Error, Result};

const COMMAND_QUEUE: usize = 32;
const EVENT_QUEUE: usize = 256;

type Pending = Arc<Mutex<VecDeque<oneshot::Sender<Result<Reply>>>>>;

struct Request {
    line: String,
    responder: oneshot::Sender<Result<Reply>>,
}

/// Handle clonável para falar com o ControlPort.
#[derive(Clone)]
pub struct ControlClient {
    commands: mpsc::Sender<Request>,
}

impl ControlClient {
    /// Abre a conexão e sobe as tarefas de I/O. Não autentica — ver
    /// [`super::establish`].
    pub async fn open(stream: TcpStream) -> (Self, mpsc::Receiver<AsyncEvent>) {
        let (read_half, write_half) = stream.into_split();
        let (command_tx, command_rx) = mpsc::channel(COMMAND_QUEUE);
        let (event_tx, event_rx) = mpsc::channel(EVENT_QUEUE);
        let pending: Pending = Arc::default();

        tokio::spawn(write_loop(write_half, command_rx, pending.clone()));
        tokio::spawn(read_loop(read_half, event_tx, pending));

        (
            Self {
                commands: command_tx,
            },
            event_rx,
        )
    }

    /// Envia um comando e devolve a resposta, já validada como 2xx.
    pub async fn send(&self, command: &str) -> Result<Reply> {
        let (responder, response) = oneshot::channel();

        self.commands
            .send(Request {
                line: format!("{command}\r\n"),
                responder,
            })
            .await
            .map_err(|_| Error::ControlClosed)?;

        response
            .await
            .map_err(|_| Error::ControlClosed)?
            .and_then(Reply::into_result)
    }

    /// `GETINFO <key>` devolvendo apenas o valor.
    pub async fn get_info(&self, key: &str) -> Result<String> {
        let reply = self.send(&format!("GETINFO {key}")).await?;
        reply
            .value(key)
            .map(str::to_string)
            .ok_or_else(|| Error::other(format!("resposta sem o campo `{key}`")))
    }

    pub async fn signal(&self, name: &str) -> Result<()> {
        self.send(&format!("SIGNAL {name}")).await?;
        Ok(())
    }
}

async fn write_loop(
    mut writer: OwnedWriteHalf,
    mut commands: mpsc::Receiver<Request>,
    pending: Pending,
) {
    while let Some(request) = commands.recv().await {
        // Enfileirar antes de escrever: a resposta só pode chegar depois disso,
        // então a leitora nunca encontra a fila vazia.
        pending
            .lock()
            .expect("pending mutex")
            .push_back(request.responder);

        if let Err(err) = writer.write_all(request.line.as_bytes()).await {
            log::warn!("erro ao escrever no ControlPort: {err}");
            break;
        }
    }

    fail_pending(&pending);
}

async fn read_loop(reader: OwnedReadHalf, events: mpsc::Sender<AsyncEvent>, pending: Pending) {
    let mut lines = BufReader::new(reader).lines();
    let mut parser = ReplyParser::default();

    loop {
        let line = match lines.next_line().await {
            Ok(Some(line)) => line,
            Ok(None) => break,
            Err(err) => {
                log::warn!("erro ao ler do ControlPort: {err}");
                break;
            }
        };

        let Some(reply) = parser.push(&line) else {
            continue;
        };

        if reply.code == 650 {
            if let Some(event) = AsyncEvent::parse(&reply) {
                // Nunca bloquear a leitura por causa de um consumidor lento.
                let _ = events.try_send(event);
            }
            continue;
        }

        let responder = pending.lock().expect("pending mutex").pop_front();
        match responder {
            Some(responder) => {
                let _ = responder.send(Ok(reply));
            }
            None => log::warn!("resposta {} sem comando correspondente", reply.code),
        }
    }

    fail_pending(&pending);
}

/// Libera quem estava esperando resposta quando a conexão morre.
fn fail_pending(pending: &Pending) {
    let waiting = std::mem::take(&mut *pending.lock().expect("pending mutex"));
    for responder in waiting {
        let _ = responder.send(Err(Error::ControlClosed));
    }
}
