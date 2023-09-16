use imap::{server::ops::create::{Request, Response}};
use imap_proto::response::StatusResponse;

pub async fn create(req: Request) -> Result<Response, StatusResponse> {
    dbg!(req);
    todo!()
}
