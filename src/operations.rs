use imap::server::ops::Operation;

macro_rules! operations {
    ($($name:ident,)*) => {
        $(
            pub mod $name;
            pub use $name::$name;
        )*

        pub async fn handle(
            op: Operation,
        ) {
            paste::paste! {
                match op {
                    $(
                        Operation::[<$name:camel>](req, channel) => {
                            let res = $name(req).await;
                            channel.send(res).await.unwrap();
                        }
                    )*
                }
            }
        }
    }
}

operations! {
    fetch,
    list,
    select,
    create,
}
