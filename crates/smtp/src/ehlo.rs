//! [SMTP EHLO response](https://datatracker.ietf.org/doc/html/rfc5321#section-4.1.1.1)
//! parsing and formatting.
//!
//! Example EHLO exchange:
//!
//! ```txt
//! S: 220 mail.example.com
//! C: EHLO localhost
//! S: 250-mail.example.com
//! 250-PIPELINING
//! 250-SIZE 52428800
//! 250-ETRN
//! 250-AUTH PLAIN LOGIN
//! 250-ENHANCEDSTATUSCODES
//! 250-8BITMIME
//! 250-DSN
//! 250-CHUNKING
//! 250 STARTTLS
//! ```

use std::{borrow::Cow, fmt, iter};

use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncReadExt};
use util::flags;

use crate::LINE_LIMIT;

/// SMTP EHLO response.
///
/// ```
/// # use smtp::ehlo::{Auth, Extensions, Response};
/// let ehlo = Response {
///     domain: "mail.example.com".to_owned(),
///     extensions: Extensions::STARTTLS,
///     size: Some(1024),
///     auth: Auth::PLAIN,
/// };
///
/// assert_eq!(
///     ehlo.to_string(),
///     "250-mail.example.com\r\n\
///     250-STARTTLS\r\n\
///     250-SIZE 1024\r\n\
///     250 AUTH PLAIN\r\n"
/// );
/// ```
#[derive(Debug, PartialEq, Eq)]
pub struct Response {
    /// Domain name of the server.
    pub domain: String,
    /// SMTP extensions advertised.
    pub extensions: Extensions,
    /// Maximum message size in bytes
    /// ([RFC 1870](https://datatracker.ietf.org/doc/html/rfc1870)).
    pub size: Option<u64>,
    /// AUTH mechanisms supported.
    pub auth: Auth,
}

flags! {
    /// AUTH mechanisms advertised by the server.
    pub Auth: u8 {
        (1 << 0, "LOGIN", LOGIN);
        (1 << 1, "PLAIN", PLAIN);
    }
}

impl fmt::Display for Auth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AUTH")?;
        for auth in self.names() {
            write!(f, " {auth}")?;
        }
        Ok(())
    }
}

flags! {
    /// SMTP extensions.
    pub Extensions: u8 {
        (1 << 0, "8BITMIME", _8BITMIME);
        (1 << 1, "SMTPUTF8", SMTPUTF8);
        /// Message chunking per [RFC 3030].
        ///
        /// Also see [`Command::Bdat`].
        ///
        /// [`Command::Bdat`]: crate::command::Command#variant.Bdat
        /// [RFC 3030]: https://datatracker.ietf.org/doc/html/rfc3030
        (1 << 2, "CHUNKING", CHUNKING);
        /// Oppurtunistic TLS support using `STARTTLS`
        /// ([RFC 3207](https://datatracker.ietf.org/doc/html/rfc3207)).
        ///
        /// Also see [`Command::Starttls`].
        ///
        /// [`Command::Starttls`]: crate::command::Command#variant.Starttls
        (1 << 3, "STARTTLS", STARTTLS);
        (1 << 4, "ENHANCEDSTATUSCODES", ENHANCEDSTATUSCODES);
    }
}

impl fmt::Display for Response {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "250-{}\r\n", self.domain)?;
        let mut lines = self
            .extensions
            .names()
            .map(Cow::Borrowed)
            .chain(self.size.map(|s| Cow::Owned(format!("SIZE {s}"))))
            .chain(iter::once(self.auth.to_string().into()))
            .peekable();

        while let Some(ehlo_line) = lines.next() {
            if lines.peek().is_some() {
                write!(f, "250-{ehlo_line}\r\n")?;
            } else {
                write!(f, "250 {ehlo_line}\r\n")?;
            }
        }

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("syntax error")]
    Syntax,
}

impl Response {
    /// Parse an SMTP EHLO response asynchonously.
    ///
    /// ```
    /// # use smtp::ehlo::{Auth, Extensions, Response};
    /// # tokio_test::block_on(async {
    /// let mut ehlo = tokio::io::BufReader::new(
    ///     "250-mail.example.com\r\n\
    ///     250-8BITMIME\r\n\
    ///     250-SMTPUTF8\r\n\
    ///     250-CHUNKING\r\n\
    ///     250-STARTTLS\r\n\
    ///     250-SIZE 1024\r\n\
    ///     250 AUTH LOGIN PLAIN\r\n"
    ///     .as_bytes(),
    /// );
    ///
    /// assert_eq!(
    ///     Response::read(&mut ehlo).await.unwrap(),
    ///     Response {
    ///         domain: "mail.example.com".to_owned(),
    ///         extensions: Extensions::_8BITMIME
    ///          | Extensions::SMTPUTF8
    ///          | Extensions::CHUNKING
    ///          | Extensions::STARTTLS,
    ///         size: Some(1024),
    ///         auth: Auth::LOGIN | Auth::PLAIN,
    ///     },
    /// );
    /// # });
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`ParseError::Syntax`] if the response is malformed.
    pub async fn read<R: AsyncRead + AsyncBufRead + Unpin>(
        reader: &mut R,
    ) -> Result<Self, ParseError> {
        let mut line = Vec::new();
        let mut domain = None;
        let mut size = None;
        let mut eol = false;
        let mut extensions = Extensions::empty();
        let mut auth = Auth::empty();

        loop {
            if eol {
                return Ok(Self {
                    domain: domain.ok_or(ParseError::Syntax)?,
                    extensions,
                    size,
                    auth,
                });
            }

            line.clear();
            if reader
                .take(LINE_LIMIT as _)
                .read_until(b'\n', &mut line)
                .await?
                < 5
            {
                return Err(ParseError::Syntax);
            }

            match line.get(3) {
                Some(b'-') => (),
                Some(b' ') => {
                    eol = true;
                }
                _ => return Err(ParseError::Syntax),
            }

            let line = std::str::from_utf8(&line[4..])
                .map_err(|_| ParseError::Syntax)?
                .trim_end();

            if domain.is_none() {
                domain = Some(line.to_owned());
            } else {
                let (keyword, args) = line.split_once(' ').unwrap_or((line, ""));

                extensions |= match keyword {
                    "8BITMIME" => Extensions::_8BITMIME,
                    "SMTPUTF8" => Extensions::SMTPUTF8,
                    "CHUNKING" => Extensions::CHUNKING,
                    "STARTTLS" => Extensions::STARTTLS,
                    "ENHANCEDSTATUSCODES" => Extensions::ENHANCEDSTATUSCODES,
                    "SIZE" => {
                        size = Some(args.parse().map_err(|_| ParseError::Syntax)?);
                        continue;
                    }
                    "AUTH" => {
                        auth = args.split(' ').collect();
                        continue;
                    }
                    _ => continue,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use tokio::io::BufReader;

    use super::*;

    #[test]
    fn fmt() {
        let ehlo = Response {
            domain: "mail.example.com".to_owned(),
            extensions: Extensions::all(),
            size: Some(1024),
            auth: Auth::all(),
        };

        assert_eq!(
            ehlo.to_string().split("\r\n").collect::<Vec<_>>(),
            [
                "250-mail.example.com",
                "250-8BITMIME",
                "250-SMTPUTF8",
                "250-CHUNKING",
                "250-STARTTLS",
                "250-ENHANCEDSTATUSCODES",
                "250-SIZE 1024",
                "250 AUTH LOGIN PLAIN",
                ""
            ]
        );
    }

    #[tokio::test]
    async fn parse() {
        let ehlo = [
            "250-pio-pvt-msa3.bahnhof.se",
            "250-PIPELINING",
            "250-SIZE 52428800",
            "250-ETRN",
            "250-AUTH PLAIN LOGIN",
            "250-ENHANCEDSTATUSCODES",
            "250-8BITMIME",
            "250-DSN",
            "250-CHUNKING",
            "250 STARTTLS",
            "",
        ]
        .join("\r\n");

        assert_eq!(
            Response::read(&mut BufReader::new(ehlo.as_bytes()))
                .await
                .unwrap(),
            Response {
                domain: "pio-pvt-msa3.bahnhof.se".to_owned(),
                extensions: Extensions::_8BITMIME
                    | Extensions::CHUNKING
                    | Extensions::STARTTLS
                    | Extensions::ENHANCEDSTATUSCODES,
                size: Some(52428800),
                auth: Auth::PLAIN | Auth::LOGIN,
            }
        );

        let ehlo = Response {
            domain: "mail.example.com".to_owned(),
            extensions: Extensions::all(),
            size: None,
            auth: Auth::all(),
        };
        assert_eq!(
            Response::read(&mut BufReader::new(ehlo.to_string().as_bytes()))
                .await
                .unwrap(),
            ehlo
        );
    }
}
