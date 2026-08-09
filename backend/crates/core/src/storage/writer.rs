// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The one task that holds the write connection.

use std::{fmt, future::Future, pin::Pin};

use sqlx::SqliteConnection;
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
};

use crate::storage::StorageError;

/// How many mutations may queue before a caller waits for the actor.
const QUEUE_DEPTH: usize = 64;

/// One mutation, executed by the write actor on the single write connection.
///
/// The operation receives a connection and nothing else — no HTTP client, no
/// filesystem root. That is what stops a transaction spanning external I/O
/// (`I-DATA-2`): a write operation has nothing to make an external call with, so
/// a long pass is forced into the shape PRD §19.4 requires, reading a snapshot,
/// doing its I/O outside, and committing in short operations at checkpoints.
pub trait WriteOperation: Send + 'static {
    /// What the caller gets back once the mutation has been applied.
    type Output: Send + 'static;

    /// Applies the mutation.
    ///
    /// # Errors
    /// Returns the underlying `sqlx` failure; the actor stays alive and serves
    /// the next mutation.
    fn execute(
        self,
        conn: &mut SqliteConnection,
    ) -> impl Future<Output = Result<Self::Output, sqlx::Error>> + Send;
}

/// A cloneable handle to the write actor. The only way to mutate the database.
#[derive(Clone)]
pub struct WriteHandle {
    sender: mpsc::Sender<Message>,
}

impl WriteHandle {
    /// Applies `operation` on the write connection and returns its result.
    ///
    /// # Errors
    /// Returns [`StorageError::WriterStopped`] if the actor has shut down, or
    /// [`StorageError::Statement`] if the mutation itself failed.
    pub async fn submit<O: WriteOperation>(&self, operation: O) -> Result<O::Output, StorageError> {
        let (reply, response) = oneshot::channel();
        self.sender
            .send(Message::Run(Box::new(Job { operation, reply })))
            .await
            .map_err(|_| StorageError::WriterStopped)?;
        response
            .await
            .map_err(|_| StorageError::WriterStopped)?
            .map_err(StorageError::from)
    }

    /// Asks the actor to finish its queue and stop.
    ///
    /// Explicit rather than "stop when the last handle drops": handles are
    /// cloned into leases, jobs, and passes, and a shutdown that waits for every
    /// clone to be dropped is a shutdown that hangs on the one clone somebody
    /// kept. Work already queued still runs — the message is FIFO behind it.
    pub(crate) async fn shutdown(&self) {
        drop(self.sender.send(Message::Shutdown).await);
    }
}

impl fmt::Debug for WriteHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WriteHandle")
            .field("closed", &self.sender.is_closed())
            .finish()
    }
}

/// Starts the actor that owns `connection`, returning its handle and its task.
pub(crate) fn spawn(connection: SqliteConnection) -> (WriteHandle, JoinHandle<()>) {
    let (sender, mut inbox) = mpsc::channel::<Message>(QUEUE_DEPTH);
    let task = tokio::spawn(async move {
        let mut connection = connection;
        while let Some(message) = inbox.recv().await {
            match message {
                Message::Run(job) => job.run(&mut connection).await,
                Message::Shutdown => break,
            }
        }
        drop(sqlx::Connection::close(connection).await);
    });
    (WriteHandle { sender }, task)
}

/// What travels down the channel to the actor.
enum Message {
    /// A mutation to apply.
    Run(ErasedJob),
    /// Finish the queue and close the connection.
    Shutdown,
}

/// A queued mutation with its type erased so the channel can carry any of them.
type ErasedJob = Box<dyn ErasedWrite + Send>;

/// The dyn-compatible shape of a queued mutation.
trait ErasedWrite {
    fn run<'c>(
        self: Box<Self>,
        conn: &'c mut SqliteConnection,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'c>>;
}

struct Job<O: WriteOperation> {
    operation: O,
    reply: oneshot::Sender<Result<O::Output, sqlx::Error>>,
}

impl<O: WriteOperation> ErasedWrite for Job<O> {
    fn run<'c>(
        self: Box<Self>,
        conn: &'c mut SqliteConnection,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'c>> {
        Box::pin(async move {
            let outcome = self.operation.execute(conn).await;
            // The caller may have been cancelled between submitting and awaiting.
            // The mutation still happened, and dropping the reply says nothing
            // about whether it did.
            drop(self.reply.send(outcome));
        })
    }
}
