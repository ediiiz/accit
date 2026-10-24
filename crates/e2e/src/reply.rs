//! What a request through Traefik came back with, read from `curl -i` or `wget -S` output.

/// One HTTP response.
#[derive(Debug, PartialEq, Eq)]
pub struct Reply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl Reply {
    /// Reads `curl -i` output: a status line, headers, a blank line, the body.
    pub fn parse(output: &str) -> Option<Self> {
        let (head, body) = output.split_once("\r\n\r\n")?;
        let mut lines = head.lines();
        let status = lines.next()?.split_whitespace().nth(1)?.parse().ok()?;
        let headers = lines
            .filter_map(|line| line.split_once(':'))
            .map(|(name, value)| (name.trim().to_owned(), value.trim().to_owned()))
            .collect();

        Some(Self { status, headers, body: body.to_owned() })
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(sent, _)| sent.eq_ignore_ascii_case(name)).map(|(_, value)| value.as_str())
    }

    /// A string field of the JSON body.
    pub fn field(&self, name: &str) -> Option<String> {
        let body: serde_json::Value = serde_json::from_str(&self.body).ok()?;

        body.get(name)?.as_str().map(str::to_owned)
    }
}

/// The status in `wget -S` output (stderr): the first `HTTP/1.x NNN` line, also present on errors.
pub fn wget_status(stderr: &str) -> Option<u16> {
    let line = stderr.lines().map(str::trim).find(|line| line.starts_with("HTTP/"))?;

    line.split_whitespace().nth(1)?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_status_headers_and_body() {
        let output = "HTTP/2 200 \r\ncontent-type: application/json\r\ncache-control: no-store\r\n\r\n{\"state\":\"none\"}";

        let reply = Reply::parse(output).unwrap();

        assert_eq!(reply.status, 200);
        assert_eq!(reply.header("Cache-Control"), Some("no-store"));
        assert_eq!(reply.body, "{\"state\":\"none\"}");
    }

    // The shape of docs/spec.md section 8.
    const STATUS: &str = r#"{"state":"active","expiry":1800604800,"server_time":1800000000,"address":"10.89.10.11","family":"ipv4","apps":[],"title":"Access","default_language":"en","csrf_token":"c0ffee"}"#;

    #[test]
    fn reads_a_field_of_the_json_body() {
        let reply = Reply { status: 200, headers: vec![], body: STATUS.to_owned() };

        assert_eq!(reply.field("csrf_token").as_deref(), Some("c0ffee"));
        assert_eq!(reply.field("state").as_deref(), Some("active"));
        assert_eq!(reply.field("expiry"), None);
        assert_eq!(reply.field("missing"), None);
    }

    #[test]
    fn reads_the_status_busybox_wget_prints() {
        let accepted = "  HTTP/1.1 204 No Content\n  Cache-Control: no-store\n  Content-Length: 0\n  \n";
        let refused = "  HTTP/1.1 403 Forbidden\nwget: server returned error: HTTP/1.1 403 Forbidden\n";

        assert_eq!(wget_status(accepted), Some(204));
        assert_eq!(wget_status(refused), Some(403));
        assert_eq!(wget_status("wget: can't connect to remote host: Connection refused\n"), None);
    }
}
