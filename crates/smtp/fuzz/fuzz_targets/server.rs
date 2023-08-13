#![no_main]

use libfuzzer_sys::fuzz_target;
use tokio::io::AsyncWriteExt;

async fn run(data: Box<[u8]>) {
    let (mut client, server) = tokio::io::duplex(1024);
    let mut session = smtp::session::Session::new(server);

    tokio::spawn(async move {
        client.write_all(&data).await.unwrap();
    });

    while let Ok(Some(message)) = session.next_message().await {}
}

fuzz_target!(|data: Box<[u8]>| {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(run(data));
});
