use std::fmt;

use auth::sasl::MechanismKind;
use util::flags;

flags! {
    pub Capabilities: u8 {
        (1 << 0, "IMAP4", IMAP4); // MUST be the first capability listed (RFC 1730)
        (1 << 1, "IMAP4rev1", IMAP4rev1);
        (1 << 2, "IMAP4rev2", IMAP4rev2);
        (1 << 3, "STARTTLS", STARTTLS);
        (1 << 4, "AUTH=PLAIN", AUTH_PLAIN);
        (1 << 5, "LOGINDISABLED", LOGINDISABLED);
        (1 << 6, "SASL-IR", SASL_IR);
    }
}

impl Capabilities {
    #[must_use]
    pub const fn auth(mechanism: MechanismKind) -> Self {
        match mechanism {
            MechanismKind::Plain => Self::AUTH_PLAIN,
        }
    }
}

impl fmt::Display for Capabilities {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CAPABILITY")?;
        for capability in self.names() {
            write!(f, " {capability}",)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Capabilities;

    #[test]
    fn fmt() {
        assert_eq!(
            Capabilities::all().to_string(),
            "CAPABILITY IMAP4 IMAP4rev1 IMAP4rev2 STARTTLS AUTH=PLAIN LOGINDISABLED SASL-IR"
        );
    }

    #[test]
    fn from_iter() {
        assert_eq!(
            Capabilities::all()
                .to_string()
                .split(" ")
                .collect::<Capabilities>(),
            Capabilities::all()
        );
    }
}
