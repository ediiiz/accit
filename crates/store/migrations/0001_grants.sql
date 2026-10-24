-- Times are Unix milliseconds. Expired rows stay until audit retention passes.
CREATE TABLE grants (
  address TEXT PRIMARY KEY NOT NULL,
  expires_at INTEGER NOT NULL
) STRICT;

CREATE TABLE audit_events (
  id INTEGER PRIMARY KEY,
  action TEXT NOT NULL CHECK (action IN ('unlock', 'revoke')),
  address TEXT NOT NULL,
  at INTEGER NOT NULL
) STRICT;

CREATE INDEX audit_events_at ON audit_events (at);
