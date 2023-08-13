CREATE TABLE mailboxes (
  name TEXT PRIMARY KEY,
  uid_validity INTEGER NOT NULL DEFAULT 0,
  next_uid INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE messages (
  mailbox TEXT NOT NULL REFERENCES mailboxes(name) ON DELETE CASCADE,
  uid INTEGER NOT NULL,
  flags TEXT NOT NULL DEFAULT '',
  data BYTEA NOT NULL,
  PRIMARY KEY (mailbox, uid)
);
