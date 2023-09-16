use imap::server::ops::select::{Request, Response};
use imap_proto::{response::StatusResponse, Uid, command::list::{ListItem, Attributes}};

pub async fn select(req: Request) -> Result<Response, StatusResponse> {
    let Request { mailbox, read_only } = req;

    Ok(Response {
        flags: vec![],
        exists: 32,
        uid_validity: 58943,
        next_uid: Uid(432.try_into().unwrap()),
        mailbox: ListItem {
            name: mailbox,
            attributes: Attributes::empty(),
            hierarchy_delimiter: None,
        },
        read_only,
    })
}
