use std::{collections::HashMap, marker::PhantomData};

use imap_proto::{command::CommandName, response::StatusResponse, Tag};
use tokio::sync::mpsc::{self, error::TryRecvError};
use tracing::warn;

use super::ops;

pub type Payload = (Tag, Result<ops::Response, StatusResponse>);

#[derive(Debug)]
pub struct Channel<T: Into<ops::Response>> {
    tag: Tag,
    tx: mpsc::Sender<Payload>,
    phantom: std::marker::PhantomData<T>,
}

#[derive(Debug)]
pub struct SendError<T>(pub T);

impl<T: Into<ops::Response>> Channel<T> {
    pub fn tag(&self) -> &Tag {
        &self.tag
    }

    pub async fn send(self, res: Result<T, StatusResponse>) -> Result<(), SendError<Self>> {
        let Self { tag, tx, phantom } = self;

        match tx.send((tag, res.map(Into::into))).await {
            Ok(()) => Ok(()),
            Err(mpsc::error::SendError((tag, _))) => Err(SendError(Self { tag, tx, phantom })),
        }
    }
}

pub struct Queue {
    commands: HashMap<Tag, CommandName>,
    tx: mpsc::Sender<Payload>,
    rx: mpsc::Receiver<Payload>,
}

impl Default for Queue {
    fn default() -> Self {
        Self::new()
    }
}

impl Queue {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(10);
        Self {
            commands: HashMap::new(),
            tx,
            rx,
        }
    }

    pub fn must_wait_before(&self, _command: &CommandName) -> bool {
        !self.commands.is_empty()
    }

    pub fn insert<T: Into<ops::Response>>(&mut self, tag: Tag, command: CommandName) -> Channel<T> {
        if let Some(overwritten) = self.commands.insert(tag.clone(), command) {
            warn!(?tag, ?overwritten, "reused tag of command in progress");
        }

        Channel {
            tag,
            tx: self.tx.clone(),
            phantom: PhantomData,
        }
    }

    /// Returns the next [`Payload`] if one is ready.
    pub fn ready(&mut self) -> Option<Payload> {
        match self.rx.try_recv() {
            Ok((tag, res)) => {
                assert!(self.commands.remove(&tag).is_some(), "tag should be known");
                Some((tag, res))
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => panic!("channel should not be closed"),
        }
    }

    /// Wait for the next [`Payload`].
    pub async fn wait(&mut self) -> Payload {
        let (tag, res) = self.rx.recv().await.expect("channel should not be closed");
        assert!(self.commands.remove(&tag).is_some(), "tag should be known");
        (tag, res)
    }
}
