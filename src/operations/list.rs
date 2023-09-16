use imap::server::ops::list::{Request, Response};
use imap_proto::{command::{self, list::{ListItem, Attributes}}, response::StatusResponse};

pub async fn list(req: Request) -> Result<Response, StatusResponse> {
    let Request(command::List { reference: _, mailbox: _ }) = req;

    Ok(Response {
        list_items: vec![
            ListItem::new("INBOX", Attributes::empty()),
            ListItem::new("Drafts", Attributes::DRAFTS),
            ListItem::new("Sent", Attributes::SENT),
            ListItem::new("Archive", Attributes::ARCHIVE),
            ListItem::new("Junk", Attributes::JUNK),
            ListItem::new("Trash", Attributes::TRASH),
        ],
    })
}
