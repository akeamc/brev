use auth::Identity;
use line::{
    stream::{MaybeTls, ServerTlsStream},
    Connection,
};
use tokio::io::{AsyncRead, AsyncWrite, BufReader};
use tracing::{debug, instrument};

use crate::{
    command::{read_cmd, Command},
    ehlo::{self, Extensions},
    io::bye,
    message::{Envelope, Incoming},
};

type BufTlsStream<IO> = BufReader<MaybeTls<ServerTlsStream<IO>, IO>>;

/// SMTP session with a client.
pub struct Session<IO: AsyncRead + AsyncWrite + Unpin, A: auth::Validator> {
    connection: Connection<ServerTlsStream<IO>, IO>,
    envelope: Option<Envelope>,
    helo_domain: Option<String>,
    identity: Option<Identity>,
    greeted: bool,
    config: crate::server::Context<A>,
}

impl<IO: AsyncRead + AsyncWrite + Unpin, A: auth::Validator> Session<IO, A> {
    pub fn new(
        stream: impl Into<MaybeTls<ServerTlsStream<IO>, IO>>,
        config: crate::server::Context<A>,
    ) -> Self {
        Self {
            connection: Connection::new(stream),
            envelope: None,
            helo_domain: None,
            identity: None,
            greeted: false,
            config,
        }
    }

    fn reset_mail_txn(&mut self) {
        self.envelope = None;
    }

    /// Send the SMTP greeting.
    async fn greet(&mut self) -> std::io::Result<()> {
        self.connection
            .write_flush(format!("220 {}\r\n", self.config.hostname))
            .await
    }

    /// Try to the current envelope and complain to the client if
    /// it's not completed.
    async fn take_envelope(&mut self) -> std::io::Result<Option<Envelope>> {
        match self.envelope.take() {
            None => {
                self.connection
                    .write_flush("503 need MAIL command\r\n")
                    .await?;
                Ok(None)
            }
            Some(envelope) if envelope.recipients.is_empty() => {
                self.connection.write_flush("554 no recipients\r\n").await?;
                self.envelope = Some(envelope);
                Ok(None)
            }
            Some(envelope) => {
                debug!(?envelope, "envelope ready");
                Ok(Some(envelope))
            }
        }
    }

    async fn ehlo(&mut self, domain: String) -> std::io::Result<()> {
        debug!(?domain, "received ehlo");
        self.reset_mail_txn();
        self.helo_domain = Some(domain);

        let mut extensions = Extensions::_8BITMIME | Extensions::SMTPUTF8 | Extensions::CHUNKING;

        if self.config.tls.is_some() && self.connection.is_plain() {
            extensions |= Extensions::STARTTLS;
        }

        self.connection
            .write_flush(
                ehlo::Response {
                    domain: self.config.hostname.clone(),
                    extensions,
                    size: None,
                    auth: ehlo::Auth::all(),
                }
                .to_string(),
            )
            .await
    }

    async fn starttls(&mut self) -> std::io::Result<()> {
        if self.connection.is_tls() {
            self.connection
                .write_flush("454 Already using TLS\r\n")
                .await?;
            return Ok(());
        }

        let tls_config = match &self.config.tls {
            Some(tls_config) => tls_config.clone(),
            None => {
                self.connection
                    .write_flush("454 TLS not available\r\n")
                    .await?;
                return Ok(());
            }
        };

        self.connection.write_flush("220 Go ahead\r\n").await?;
        self.connection.upgrade(&tls_config.into()).await?;

        // reset state
        self.helo_domain = None;
        self.identity = None;
        self.reset_mail_txn();

        Ok(())
    }

    #[instrument(skip_all)]
    pub async fn next_message(
        &mut self,
    ) -> std::io::Result<Option<Incoming<'_, BufTlsStream<IO>>>> {
        if !self.greeted {
            self.greet().await?;
            self.greeted = true;
        }

        loop {
            let cmd = match read_cmd(self.connection.stream_mut()).await? {
                None => return Ok(None),
                Some(cmd) => cmd,
            };

            match cmd {
                Command::Helo { domain } => {
                    debug!(?domain, "received helo");
                    self.reset_mail_txn();
                    self.helo_domain = Some(domain);
                    self.connection.write_flush("250 hello\r\n").await?;
                }
                Command::Ehlo { domain } => self.ehlo(domain).await?,
                Command::Mail { from } => {
                    if self.helo_domain.is_none() {
                        self.connection
                            .write_flush("503 say HELO first\r\n")
                            .await?;
                    } else if self.envelope.is_some() {
                        self.connection
                            .write_flush("501 transaction already started\r\n")
                            .await?;
                    } else {
                        self.envelope = Some(Envelope::new(from));
                        self.connection.write_flush("250 ok\r\n").await?;
                    }
                }
                Command::Rcpt { to } => match &mut self.envelope {
                    None => {
                        self.connection
                            .write_flush("503 need MAIL command\r\n")
                            .await?;
                    }
                    Some(envelope) => {
                        envelope.recipients.insert(to);
                        self.connection.write_flush("250 ok\r\n").await?;
                    }
                },
                Command::Data => {
                    if let Some(envelope) = self.take_envelope().await? {
                        self.connection.write_flush("354 go ahead\r\n").await?;
                        return Ok(Some(Incoming::data(envelope, self.connection.stream_mut())));
                    }
                }
                Command::Rset => {
                    self.reset_mail_txn();
                    self.connection.write_flush("250 ok\r\n").await?;
                }
                Command::Bdat { size, last } => {
                    if let Some(envelope) = self.take_envelope().await? {
                        debug!(size, last, "starting bdat");
                        return Ok(Some(Incoming::bdat(
                            envelope,
                            size,
                            last,
                            self.connection.stream_mut(),
                        )));
                    }
                }
                Command::Quit => bye(self.connection.stream_mut()).await?,
                Command::Noop => self.connection.write_flush("250 ok\r\n").await?,
                Command::Starttls => self.starttls().await?,
                Command::Auth {
                    mechanism,
                    initial_response,
                } => {
                    // https://datatracker.ietf.org/doc/html/rfc4954#section-4:
                    // The AUTH command is not permitted during a mail transaction.
                    // An AUTH command issued during a mail transaction MUST be
                    // rejected with a 503 reply.
                    if self.envelope.is_some() {
                        self.connection
                            .write_flush("503 transaction already started\r\n")
                            .await?;
                        continue;
                    }

                    if self.identity.is_some() {
                        self.connection
                            .write_flush("503 already authenticated\r\n")
                            .await?;
                        continue;
                    }

                    self.connection.write_flush("235 welcome\r\n").await?;
                }
            }
        }
    }
}
