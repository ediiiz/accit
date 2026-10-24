//! The Grant commands: `list`, `revoke <address>`, `revoke-all`, `recover --discard-grants|--keep-grants`.
//! Each runs on the daemon's Grant store, so its single mutation path writes the audit events.

use std::fmt::Write;
use std::time::SystemTime;

use access::Address;
use anyhow::{Context, Result, bail};
use store::{Grants, Recovery};

const RECOVER_USAGE: &str = "recover needs exactly one of --discard-grants, --keep-grants";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GrantCommand {
    List,
    Revoke(Address),
    RevokeAll,
    Recover(Recovery),
}

impl GrantCommand {
    /// `None` when `request` names no Grant command; an error when it names one wrongly.
    pub(crate) fn parse(request: &str) -> Result<Option<Self>> {
        let words: Vec<&str> = request.split_whitespace().collect();

        Ok(Some(match words.as_slice() {
            ["list"] => Self::List,
            ["revoke", address] => {
                Self::Revoke(address.parse().with_context(|| format!("invalid address {address:?}"))?)
            }
            ["revoke-all"] => Self::RevokeAll,
            ["recover", "--discard-grants"] => Self::Recover(Recovery::DiscardGrants),
            ["recover", "--keep-grants"] => Self::Recover(Recovery::KeepGrants),
            ["recover", ..] => bail!(RECOVER_USAGE),
            ["revoke", ..] => bail!("revoke needs exactly one address"),
            _ => return Ok(None),
        }))
    }

    pub(crate) async fn run(self, grants: &Grants) -> Result<String> {
        let now = SystemTime::now();

        Ok(match self {
            Self::List => list(grants, now),
            Self::Revoke(address) => revoke(grants, address, now).await?,
            Self::RevokeAll => format!("revoked: {}\n", grants.revoke_all(now).await?),
            Self::Recover(recovery) => recover(grants, recovery, now).await?,
        })
    }
}

async fn revoke(grants: &Grants, address: Address, now: SystemTime) -> Result<String> {
    grants.revoke(address, now).await?;

    Ok(format!("revoked {address}\n"))
}

async fn recover(grants: &Grants, recovery: Recovery, now: SystemTime) -> Result<String> {
    grants.recover(recovery, now).await?;
    tracing::warn!(?recovery, "recovered from failing closed");

    Ok("recovered\n".to_owned())
}

fn list(grants: &Grants, now: SystemTime) -> String {
    grants.list(now).into_iter().fold(String::new(), |mut text, (address, expiry)| {
        let _ = writeln!(text, "{address} {}", httpdate::fmt_http_date(expiry));
        text
    })
}
