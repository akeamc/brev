use imap::server::ops::fetch::{Response, Request};
use imap_proto::response::StatusResponse;

pub async fn fetch(req: Request) -> Result<Response, StatusResponse> {
    dbg!(req);
    todo!()
}
