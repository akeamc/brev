use std::time::Duration;

use lettre::{
    transport::smtp::client::{Tls, TlsParameters},
    Message, SmtpTransport, Transport,
};

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let body = vec![69; 1000];

    let email = Message::builder()
        .from("Günter <günter@domain.tld>".parse()?)
        .to("Åke <åke@domain.tld>".parse()?)
        .subject("Gott nytt år!")
        // .body(String::from("Be happy!"))?;
        .body(body)?;

    let tls_params = TlsParameters::builder("localhost".to_owned())
        .dangerous_accept_invalid_certs(true)
        .build()?;

    let sender = SmtpTransport::builder_dangerous("localhost")
        // .port(25)
        // .tls(Tls::Opportunistic(
        // ))
        .port(465)
        .tls(Tls::Wrapper(tls_params))
        .timeout(Some(Duration::from_secs(5)))
        .build();
    // Send the email via remote relay
    sender.send(&email)?;

    println!("Email sent successfully!");

    Ok(())
}
