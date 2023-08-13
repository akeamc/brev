use std::{
    str::{FromStr, Utf8Error},
    time::Duration,
};

use email_address::EmailAddress;
use line::{read_line, write_flush, ReadLineError};
use nom::{
    bytes::streaming::{tag, take_until},
    combinator::map_res,
    sequence::delimited,
    IResult,
};
use tokio::{
    io::{AsyncBufRead, AsyncRead, AsyncReadExt, AsyncWrite},
    time::timeout,
};
use tracing::debug;

use crate::LINE_LIMIT;

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Helo {
        domain: String,
    },
    Ehlo {
        domain: String,
    },
    Mail {
        from: EmailAddress,
    },
    Rcpt {
        to: EmailAddress,
    },
    Rset,
    Data,
    Bdat {
        size: u64,
        last: bool,
    },
    Noop,
    Quit,
    Starttls,
    /// AUTH <mechanism> [initial-response]
    ///
    /// See [RFC 4954](https://datatracker.ietf.org/doc/html/rfc4954#section-4).
    Auth {
        mechanism: auth::sasl::MechanismKind,
        /// Initial client response to save a round-trip.
        initial_response: Option<String>,
    },
}

pub enum Error {
    UnrecognizedCommand,
    Syntax(&'static str),
    InvalidUtf8,
}

impl From<Utf8Error> for Error {
    fn from(_e: Utf8Error) -> Self {
        Error::InvalidUtf8
    }
}

impl TryFrom<&[u8]> for Command {
    type Error = Error;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        let s = std::str::from_utf8(bytes)?;
        debug!(?s, "parsing command");
        let (verb, args) = s.split_once(' ').unwrap_or((s, ""));

        let cmd = match verb.to_ascii_uppercase().as_str() {
            "HELO" => Command::Helo {
                domain: args.to_owned(),
            },
            "EHLO" => Command::Ehlo {
                domain: args.to_owned(),
            },
            "MAIL" => Command::Mail {
                from: mailbox(args).map_err(|_| Error::Syntax("MAIL FROM:<address>"))?,
            },
            "RCPT" => Command::Rcpt {
                to: mailbox(args).map_err(|_| Error::Syntax("RCPT TO:<address>"))?,
            },
            "DATA" => Command::Data,
            "RSET" => Command::Rset,
            "NOOP" => Command::Noop,
            "QUIT" => Command::Quit,
            "BDAT" => {
                // BDAT <size> ["LAST"]

                let mut args = args.splitn(2, ' ');

                Command::Bdat {
                    size: args
                        .next()
                        .and_then(|s| s.parse().ok())
                        .ok_or(Error::Syntax("BDAT <size>"))?,
                    last: args
                        .next()
                        .map_or(false, |s| s.to_ascii_uppercase() == "LAST"),
                }
            }
            "STARTTLS" => Command::Starttls,
            "AUTH" => {
                let mut args = args.splitn(2, ' ');

                Command::Auth {
                    mechanism: args
                        .next()
                        .and_then(|s| s.parse().ok())
                        .ok_or(Error::Syntax("AUTH <mechanism> [initial-response]"))?,
                    initial_response: args.next().map(ToOwned::to_owned),
                }
            }
            _ => return Err(Error::UnrecognizedCommand),
        };

        Ok(cmd)
    }
}

async fn read_cmd_inner<S: AsyncRead + AsyncBufRead + AsyncWrite + Unpin>(
    stream: &mut S,
) -> std::io::Result<Option<Command>> {
    let mut buf = Vec::new();
    loop {
        match read_line(&mut stream.take(LINE_LIMIT as _), &mut buf).await {
            Ok(()) => (),
            Err(ReadLineError::Eof) => return Ok(None),
            Err(ReadLineError::Io(e)) => return Err(e),
        }

        match Command::try_from(buf.as_ref()) {
            Ok(cmd) => return Ok(Some(cmd)),
            Err(Error::InvalidUtf8) => debug!("invalid utf8"),
            Err(Error::Syntax(correct)) => {
                write_flush(stream, format!("501 Syntax: {correct}\r\n")).await?;
            }
            Err(Error::UnrecognizedCommand) => {
                write_flush(stream, "500 Unrecognized command\r\n").await?;
            }
        }

        buf.clear();
    }
}

/// Read the next command from the stream.
pub async fn read_cmd<S: AsyncRead + AsyncBufRead + AsyncWrite + Unpin>(
    stream: &mut S,
) -> std::io::Result<Option<Command>> {
    const TIMEOUT: Duration = Duration::from_secs(300);

    match timeout(TIMEOUT, read_cmd_inner(stream)).await {
        Ok(Ok(cmd)) => Ok(cmd),
        Ok(Err(e)) => Err(e),
        Err(_) => {
            debug!("timeout reading command");
            Err(std::io::Error::from(std::io::ErrorKind::TimedOut))
        }
    }
}

fn parse_mailbox(i: &str) -> IResult<&str, EmailAddress> {
    let (i, _) = take_until("<")(i)?;
    map_res(
        delimited(tag("<"), take_until(">"), tag(">")),
        EmailAddress::from_str,
    )(i)
}

fn mailbox(i: &str) -> Result<EmailAddress, ()> {
    match parse_mailbox(i) {
        Ok((_, mailbox)) => Ok(mailbox),
        Err(e) => {
            debug!(%e, "failed to parse mailbox string {i:?}");
            Err(())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use email_address::EmailAddress;
    use line::write_flush;
    use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};

    use crate::command::{read_cmd, Command};

    #[test]
    fn mailbox() {
        assert_eq!(
            super::mailbox("TO:<alice@example.com>"),
            Ok(EmailAddress::from_str("alice@example.com").unwrap())
        );

        assert_eq!(
            super::mailbox("FROM:<günter@bahn.de> SMTPUTF8 BODY=8BITMIME"),
            Ok(EmailAddress::from_str("günter@bahn.de").unwrap())
        );
    }

    #[tokio::test]
    async fn cmd() -> anyhow::Result<()> {
        let (mut client, server) = tokio::io::duplex(8192);

        let task = tokio::spawn(async move {
            let mut server = BufReader::new(server);

            assert_eq!(
                read_cmd(&mut server).await?,
                Some(Command::Helo {
                    domain: "world".to_owned()
                })
            );
            write_flush(&mut server, "250 yo\r\n").await?;

            assert_eq!(read_cmd(&mut server).await?, Some(Command::Quit));
            write_flush(&mut server, "221 bye\r\n").await?;

            assert_eq!(read_cmd(&mut server).await?, None);

            anyhow::Ok(())
        });

        client.write_all(b"HELO world\r\n").await?;
        client.write_all(b"QUIT\r\n").await?;
        client.shutdown().await?;

        task.await??;

        let mut buf = String::new();
        client.read_to_string(&mut buf).await?;

        assert_eq!(buf, "250 yo\r\n221 bye\r\n");

        Ok(())
    }
}
