#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: Box<[u8]>| {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(smtp::message::data_fuzz(data));
});
