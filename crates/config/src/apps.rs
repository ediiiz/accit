use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::origin;

/// A link shown in the Portal. Display-only: never part of the Policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppLink {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawApp {
    name: String,
    url: String,
}

impl RawApp {
    pub(crate) fn validate(self) -> Result<AppLink> {
        if self.name.trim().is_empty() {
            bail!("an app link needs a name");
        }

        let Some(authority) = origin::https_authority(&self.url) else {
            bail!("app link `{}` must be an https:// URL", self.name);
        };

        origin::host_and_port(authority).with_context(|| format!("app link `{}`", self.name))?;

        Ok(AppLink { name: self.name, url: self.url })
    }
}
