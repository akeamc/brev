use std::sync::Arc;

use brev::{operations, MultiListener};
use futures_util::{stream::FuturesUnordered, FutureExt, StreamExt};
use smtp::server::session::Session;
use sqlx::PgPool;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite},
    net::TcpStream,
};
use tokio_rustls::rustls::{self, Certificate, PrivateKey};
use tracing::{error, info, instrument};

pub struct Auth;

#[async_trait::async_trait]
impl auth::Validator for Auth {
    async fn validate(
        &self,
        credentials: &auth::Credentials,
    ) -> Result<auth::Identity, auth::ValidationError> {
        Ok(auth::Identity(credentials.username.clone()))
        // todo!()
    }
}

pub async fn handle_imap<IO: AsyncRead + AsyncWrite + Unpin, A: auth::Validator>(
    mut session: imap::server::Session<IO, A>,
) -> anyhow::Result<()> {
    let mut futures = FuturesUnordered::new();

    loop {
        tokio::select! {
            Some(()) = futures.next() => {}
            Some(res) = session.next_op().map(Result::transpose) => futures.push(operations::handle(res?)),
            else => {
                return Ok(());
            }
        }
    }
}

#[instrument(skip_all)]
async fn imap<A: auth::Validator + 'static>(
    context: imap::server::Context<A>,
) -> anyhow::Result<()> {
    let mut listener = MultiListener::new("0.0.0.0:143").await?;
    if let Some(tls) = context.tls.clone() {
        listener = listener.with_tls("0.0.0.0:993", tls).await?;
    }

    let server = imap::Server::new(context);

    loop {
        let (socket, addr) = listener.accept().await?;
        info!("Got connection from: {}", addr);
        let session = server.accept::<TcpStream>(socket);

        tokio::spawn(async move {
            if let Err(e) = handle_imap(session).await {
                error!("an error occurred: {e:?}");
            }
        });
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();
    tracing_subscriber::fmt::init();

    let pool = PgPool::connect(&std::env::var("DATABASE_URL")?).await?;

    let cert = rcgen::generate_simple_self_signed(["localhost".to_owned()]).unwrap();

    let tls_config = Arc::new(
        rustls::ServerConfig::builder()
            .with_safe_defaults()
            .with_no_client_auth()
            .with_single_cert(
                vec![Certificate(cert.serialize_der()?)],
                PrivateKey(cert.serialize_private_key_der()),
            )
            .unwrap(),
    );

    let auth = Arc::new(Auth);
    let imap = tokio::spawn(imap(imap::server::Context {
        tls: Some(tls_config.clone()),
        auth: auth.clone(),
    }));
    let smtp = tokio::spawn(smtp(
        smtp::server::Context {
            hostname: "localhost".to_owned(),
            tls: Some(tls_config.clone()),
            auth: auth.clone(),
        },
        pool.clone(),
    ));

    tokio::select! {
        res = imap => res,
        res = smtp => res,
    }?
}

#[instrument(skip_all)]
async fn smtp<A: auth::Validator + 'static>(
    context: smtp::server::Context<A>,
    pool: PgPool,
) -> anyhow::Result<()> {
    let mut listener = MultiListener::new("0.0.0.0:25").await?;
    if let Some(tls) = context.tls.clone() {
        listener = listener.with_tls("0.0.0.0:465", tls).await?;
    }

    let server = smtp::Server::new(context);

    loop {
        let (socket, addr) = listener.accept().await?;

        info!("Got connection from: {}", addr);

        let session = server.accept(socket);
        let pool = pool.clone();

        tokio::spawn(async move {
            if let Err(e) = handle_connection(session, pool).await {
                error!("an error occurred: {e:?}");
            }
        });
    }
}

async fn handle_connection<IO: AsyncRead + AsyncWrite + Unpin + Send + Sync, A: auth::Validator>(
    mut session: Session<IO, A>,
    _pool: PgPool,
) -> anyhow::Result<()> {
    println!("helo");

    while let Some(mut message) = session.next_message().await? {
        println!("Got message: {:?}", message.envelope());

        let mut out = String::new();
        message.read_to_string(&mut out).await?;
        message.accept().await?;

        println!("received {} bytes", out.len());
        println!("{out}");
    }

    println!("Connection closed");

    Ok(())
}
