use std::fmt::Display;

use auth::Identity;
use imap_proto::{
    command::{self, capability::Capabilities, Command, Request, TaggedCommand},
    response::{Status, StatusResponse, TaggedStatusResponse},
    Tag,
};
use line::{
    stream::{MaybeTls, ServerTlsStream},
    Connection,
};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tracing::instrument;

use crate::authenticate;

use super::{
    ops::{self, IntoOperation, IntoTaggedResponse, Operation},
    queue::{self, Queue},
    read_cmd,
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SelectedState {
    pub mailbox: String,
    pub read_only: bool,
    pub identity: Identity,
}

#[derive(Default, PartialEq, Eq)]
enum State {
    #[default]
    NotAuthenticated,
    Authenticated(Identity),
    Selected(SelectedState),
    Logout,
}

pub struct Session<IO: AsyncRead + AsyncWrite + Unpin, A: auth::Validator> {
    connection: Connection<ServerTlsStream<IO>, IO>,
    state: State,
    queue: Queue,
    greeted: bool,
    context: crate::server::Context<A>,
}

macro_rules! operation {
    ($cmd:expr, $queue:expr, $tag:expr, $ctx:expr) => {
        return Ok(Some($cmd.into_operation($queue, $tag, $ctx)))
    };
    ($cmd:expr, $queue:expr, $tag:expr) => {
        operation!($cmd, $queue, $tag, ())
    };
}

impl<IO: AsyncRead + AsyncWrite + Unpin, A: auth::Validator> Session<IO, A> {
    pub fn new(
        stream: impl Into<MaybeTls<ServerTlsStream<IO>, IO>>,
        context: crate::server::Context<A>,
    ) -> Self {
        Self {
            connection: Connection::new(stream),
            state: State::default(),
            queue: Queue::new(),
            greeted: false,
            context,
        }
    }

    /// Send the IMAP greeting.
    #[instrument(skip(self))]
    async fn greet(&mut self) -> std::io::Result<()> {
        self.connection
            .write_flush(format!("* OK [{}] Server ready\r\n", self.capabilities()))
            .await
    }

    fn capabilities(&self) -> Capabilities {
        let mut capabilities = Capabilities::IMAP4rev1
            | Capabilities::IMAP4rev2
            | Capabilities::AUTH_PLAIN
            | Capabilities::SASL_IR;
        if self.connection.is_plain() {
            capabilities |= Capabilities::LOGINDISABLED;
            if self.context.tls.is_some() {
                capabilities |= Capabilities::STARTTLS;
            }
        }
        capabilities
    }

    async fn write_untagged(&mut self, data: impl Display) -> std::io::Result<()> {
        self.connection.write(format!("* {data}\r\n")).await
    }

    async fn respond(&mut self, res: TaggedStatusResponse) -> std::io::Result<()> {
        self.connection.write_flush(res.to_string()).await
    }

    async fn respond_with_tag(
        &mut self,
        tag: Tag,
        res: impl IntoTaggedResponse,
    ) -> std::io::Result<()> {
        self.connection
            .write_flush(res.into_tagged_response(tag))
            .await
    }

    async fn handle_capability(&mut self, req: Request<()>) -> std::io::Result<()> {
        self.write_untagged(self.capabilities()).await?;
        self.respond(req.ok("CAPABILITY completed")).await
    }

    async fn handle_noop(&mut self, req: Request<()>) -> std::io::Result<()> {
        self.respond(req.ok("NOOP completed")).await
    }

    async fn handle_logout(&mut self, req: Request<()>) -> std::io::Result<()> {
        self.state = State::Logout;
        self.write_untagged("BYE").await?;
        self.respond(req.ok("Logged out")).await?;
        self.connection.stream_mut().shutdown().await
    }

    async fn handle_starttls(&mut self, req: Request<()>) -> std::io::Result<()> {
        if self.connection.is_tls() {
            return self
                .respond(req.into_res(Status::Bad, "Already using TLS"))
                .await;
        }

        let tls_config = match &self.context.tls {
            Some(tls_config) => tls_config.clone(),
            None => {
                return self.respond(req.bad("TLS not available")).await;
            }
        };

        self.respond(req.bad("Begin TLS negotiation")).await?;
        self.connection.upgrade(&tls_config.into()).await?;

        Ok(())
    }

    async fn auth_success(&mut self, req: Request<()>, identity: Identity) -> std::io::Result<()> {
        self.respond(req.ok("Logged in")).await?;
        self.state = State::Authenticated(identity);
        Ok(())
    }

    async fn handle_authenticate(
        &mut self,
        req: Request<command::Authenticate>,
    ) -> std::io::Result<()> {
        if self.state != State::NotAuthenticated {
            return self.respond(req.bad("Already authenticated")).await;
        }

        let (data, req) = req.into_parts();
        match authenticate::authenticate(
            self.connection.stream_mut(),
            data,
            self.context.auth.as_ref(),
        )
        .await
        {
            Ok(identity) => {
                self.auth_success(req, identity).await?;
            }
            Err(authenticate::Error::Io(io)) => return Err(io),
            Err(authenticate::Error::Mechanism(mechanism)) => {
                self.respond(StatusResponse::from(mechanism).with_tag(req.tag))
                    .await?;
            }
        }
        Ok(())
    }

    async fn handle_login(&mut self, req: Request<command::Login>) -> std::io::Result<()> {
        if self.state != State::NotAuthenticated {
            return self.respond(req.bad("Already authenticated")).await;
        }

        let (data, req) = req.into_parts();
        match self.context.auth.validate(&data.into()).await {
            Ok(identity) => self.auth_success(req, identity).await?,
            Err(e) => {
                self.respond(StatusResponse::from(e).with_tag(req.tag))
                    .await?;
            }
        }
        Ok(())
    }

    async fn handle_enable(&mut self, req: Request<command::Enable>) -> std::io::Result<()> {
        if req.data.capabilities.is_empty() {
            return self.respond(req.bad("No capabilities specified")).await;
        }

        self.respond(req.bad("ENABLE not supported")).await
    }

    /// Consume a ready payload from the queue.
    async fn consume_ready(&mut self, (tag, res): queue::Payload) -> std::io::Result<()> {
        use ops::Response;

        match res {
            Ok(Response::Select(res)) => {
                let identity = match &self.state {
                    State::Authenticated(identity) => identity.clone(),
                    _ => unreachable!(),
                };

                self.state = State::Selected(SelectedState {
                    mailbox: res.mailbox.name.clone(),
                    read_only: res.read_only,
                    identity,
                });

                self.respond_with_tag(tag, res).await?;
            }
            Ok(Response::List(res)) => {
                self.respond_with_tag(tag, res).await?;
            }
            Ok(Response::Fetch(res)) => {
                self.respond_with_tag(tag, res).await?;
            }
            Ok(Response::Create(res)) => {
                self.respond_with_tag(tag, res).await?;
            }
            Err(err) => {
                self.respond(err.with_tag(tag)).await?;
            }
        }

        Ok(())
    }

    async fn next_cmd(&mut self) -> std::io::Result<TaggedCommand> {
        while let Some(payload) = self.queue.ready() {
            self.consume_ready(payload).await?;
        }

        let tagged = read_cmd(self.connection.stream_mut())
            .await?
            .ok_or(std::io::Error::from(std::io::ErrorKind::UnexpectedEof))?;

        while self.queue.must_wait_before(&tagged.command.name()) {
            let payload = self.queue.wait().await;
            self.consume_ready(payload).await?;
        }

        Ok(tagged)
    }

    /// Returns the next operation to be executed.
    ///
    /// If the connection is closed correctly, `Ok(None)` is returned.
    ///
    /// # Errors
    ///
    /// Upon any IO error, the error is returned.
    pub async fn next_op(&mut self) -> std::io::Result<Option<Operation>> {
        if !self.greeted {
            self.greet().await?;
            self.greeted = true;
        }

        loop {
            if self.state == State::Logout {
                return Ok(None);
            }

            let TaggedCommand { tag, command } = self.next_cmd().await?;

            match command {
                Command::Capability => self.handle_capability(tag.into()).await?,
                Command::Noop => self.handle_noop(tag.into()).await?,
                Command::Logout => self.handle_logout(tag.into()).await?,
                Command::Starttls => self.handle_starttls(tag.into()).await?,
                Command::Authenticate(authenticate) => {
                    self.handle_authenticate(Request::new(tag, authenticate))
                        .await?;
                }
                Command::Login(login) => self.handle_login(Request::new(tag, login)).await?,
                Command::Enable(enable) => self.handle_enable(Request::new(tag, enable)).await?,
                Command::Select(select) => {
                    let identity = match &self.state {
                        State::NotAuthenticated => {
                            self.respond(Request::from(tag).bad("not authenticated"))
                                .await?;
                            continue;
                        }
                        State::Authenticated(identity) => todo!(),
                        State::Selected(SelectedState { identity, .. }) => identity,
                        State::Logout => unreachable!(),
                    };
                }
                Command::Examine(examine) => operation!(examine, &mut self.queue, tag),
                Command::Create(_) => todo!(),
                Command::Delete(_) => todo!(),
                Command::Rename(_) => todo!(),
                Command::Subscribe(_) => todo!(),
                Command::Unsubscribe(_) => todo!(),
                Command::List(list) => operation!(list, &mut self.queue, tag),
                Command::Namespace => todo!(),
                Command::Status(_) => todo!(),
                Command::Append => todo!(),
                Command::Idle => todo!(),
                Command::Close => todo!(),
                Command::Unselect => todo!(),
                Command::Expunge(_) => todo!(),
                Command::Search { is_uid } => todo!(),
                Command::Fetch(fetch) => match &self.state {
                    State::Selected(selected) => {
                        operation!(fetch, &mut self.queue, tag, selected.clone())
                    }
                    _ => {
                        self.respond(Request::new(tag, ()).bad("not in selected state"))
                            .await?;
                    }
                },
                Command::Store { is_uid } => todo!(),
                Command::Copy { is_uid } => todo!(),
                Command::Move { is_uid } => todo!(),
            }
        }
    }
}
