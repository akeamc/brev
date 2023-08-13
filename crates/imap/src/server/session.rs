use std::{collections::HashSet, fmt::Display};

use auth::Identity;
use line::{
    stream::{MaybeTlsStream, ServerTlsStream},
    Connection,
};
use tokio::{
    io::{AsyncRead, AsyncWrite, AsyncWriteExt},
    sync::mpsc,
};

use crate::{
    authenticate::{self, authenticate},
    command::{self, read_cmd, Command, Request, TaggedCommand},
    protocol::{capability::Capabilities, list},
    response::{Status, StatusResponse, TaggedStatusResponse},
    Tag,
};

struct CommandsInProgress {
    tags: HashSet<Tag>,
    tx: mpsc::Sender<(Tag, ())>,
    rx: mpsc::Receiver<(Tag, ())>,
}

impl CommandsInProgress {
    fn new() -> Self {
        let (tx, rx) = mpsc::channel(10);
        Self {
            tags: HashSet::new(),
            tx,
            rx,
        }
    }

    fn must_wait_before(&self, command: &Command) -> bool {
        todo!()
    }

    async fn recv(&mut self) {
        let (tag, x) = self.rx.recv().await.unwrap();
        assert!(self.tags.remove(&tag), "unwanted tag");

        todo!()
    }
}

#[derive(Default, PartialEq, Eq)]
enum State {
    #[default]
    NotAuthenticated,
    Authenticated,
    Selected {
        mailbox: String,
        read_only: bool,
    },
    Logout,
}

pub struct Session<IO: AsyncRead + AsyncWrite + Unpin, A: auth::Validator> {
    connection: Connection<ServerTlsStream<IO>, IO>,
    state: State,
    identity: Option<auth::Identity>,
    in_progress: CommandsInProgress,
    context: crate::server::Context<A>,
}

impl<IO: AsyncRead + AsyncWrite + Unpin, A: auth::Validator> Session<IO, A> {
    pub fn new(
        stream: impl Into<MaybeTlsStream<ServerTlsStream<IO>, IO>>,
        context: crate::server::Context<A>,
    ) -> Self {
        Self {
            connection: Connection::new(stream),
            state: State::default(),
            identity: None,
            in_progress: CommandsInProgress::new(),
            context,
        }
    }

    pub async fn greet(&mut self) -> std::io::Result<()> {
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
        self.identity = Some(identity);
        self.state = State::Authenticated;
        Ok(())
    }

    pub async fn handle_authenticate(
        &mut self,
        req: Request<command::Authenticate>,
    ) -> std::io::Result<()> {
        if self.state != State::NotAuthenticated {
            return self.respond(req.bad("Already authenticated")).await;
        }

        let (data, req) = req.into_parts();
        match authenticate(
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

    pub async fn handle_login(&mut self, req: Request<command::Login>) -> std::io::Result<()> {
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

    pub async fn next_op(&mut self) -> std::io::Result<Option<()>> {
        loop {
            if self.state == State::Logout {
                return Ok(None);
            }

            let TaggedCommand { tag, command } = read_cmd(self.connection.stream_mut())
                .await?
                .ok_or(std::io::Error::from(std::io::ErrorKind::UnexpectedEof))?;

            if self.in_progress.must_wait_before(&command) {
                todo!()
            }

            match command {
                Command::Capability => self.handle_capability(tag.into()).await?,
                Command::Noop => self.handle_noop(tag.into()).await?,
                Command::Logout => self.handle_logout(tag.into()).await?,
                Command::Starttls => self.handle_starttls(tag.into()).await?,
                Command::Authenticate(authenticate) => {
                    self.handle_authenticate(Request::new(tag, authenticate))
                        .await?
                }
                Command::Login(login) => self.handle_login(Request::new(tag, login)).await?,
                Command::Enable(enable) => self.handle_enable(Request::new(tag, enable)).await?,
                Command::Select(select) => todo!(),
                Command::Examine(examine) => todo!(),
                Command::Create(_) => todo!(),
                Command::Delete(_) => todo!(),
                Command::Rename(_) => todo!(),
                Command::Subscribe(_) => todo!(),
                Command::Unsubscribe(_) => todo!(),
                Command::List(_) => todo!(),
                Command::Namespace => todo!(),
                Command::Status(_) => todo!(),
                Command::Append => todo!(),
                Command::Idle => todo!(),
                Command::Close => todo!(),
                Command::Unselect => todo!(),
                Command::Expunge(_) => todo!(),
                Command::Search { is_uid } => todo!(),
                Command::Fetch(_) => todo!(),
                Command::Store { is_uid } => todo!(),
                Command::Copy { is_uid } => todo!(),
                Command::Move { is_uid } => todo!(),
            }

            // match kind {
            //     Command::Capability {} => {
            //         self.write_untagged(self.capabilities()).await?;
            //         self.respond(StatusResponse::ok(tag, "Capabilities listed"))
            //             .await?;
            //     }
            //     Command::Login { username, password } => {
            //         warn!("Logged in as {}", username);
            //         self.respond(StatusResponse::ok(tag, "Logged in")).await?;
            //         self.state = State::Authenticated;
            //     }
            //     Command::Authenticate { mechanism, initial_response } => {
            //         if self.state != State::NotAuthenticated {
            //             self.respond(StatusResponse::bad(tag, "Already authenticated"))
            //                 .await?;
            //             continue;
            //         }
            //         authenticate(self.connection.stream_mut(), tag, &mechanism).await?;
            //         self.state = State::Authenticated;
            //     }
            //     Command::Starttls {} => self.starttls(tag).await?,
            //     Command::Select { mailbox } => {
            //         if self.state == State::NotAuthenticated {
            //             self.respond(StatusResponse::bad(tag, "Not authenticated"))
            //                 .await?;
            //             continue;
            //         }
            //         self.write_untagged(flags::Response {
            //             flags: vec![
            //                 Flag::Seen,
            //                 Flag::Answered,
            //                 Flag::Flagged,
            //                 Flag::Deleted,
            //                 Flag::Draft,
            //                 Flag::Recent,
            //             ]
            //         })
            //         .await?;
            //         self.write_untagged(exists::Response { n: 1 }).await?;
            //         self.write_untagged(recent::Response { n: 1 }).await?;
            //         self.connection
            //             .write(
            //                 list::Response {
            //                     list_items: self.mailboxes(),
            //                 }
            //                 .to_string(),
            //             )
            //             .await?;
            //         self.respond(StatusResponse::ok(tag, "Selected mailbox"))
            //             .await?;
            //         self.state = State::Selected(mailbox);
            //     }
            //     Command::List { reference, mailbox } => {
            //         self.connection
            //             .write(
            //                 list::Response {
            //                     list_items: self.mailboxes(),
            //                 }
            //                 .to_string(),
            //             )
            //             .await?;
            //         self.respond(StatusResponse::ok(tag, "Selected mailbox"))
            //             .await?;
            //     }
            //     Command::Create { mailbox } => {
            //         self.respond(StatusResponse::ok(tag, "Created mailbox"))
            //             .await?;
            //     }
            //     Command::Logout {} => {
            //         self.state = State::Logout;
            //         self.write_untagged("BYE").await?;
            //         self.respond(StatusResponse::ok(tag, "Logged out")).await?;
            //         self.connection.stream_mut().shutdown().await?;
            //     }
            //     Command::Noop {} => {
            //         self.respond(StatusResponse::ok(tag, "NOOP completed"))
            //             .await?;
            //     }
            //     _ => unimplemented!()
            // }
        }
    }
}
