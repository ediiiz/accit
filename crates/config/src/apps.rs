use anyhow::{Result, bail};
use serde::Deserialize;

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

        let authority = self.url.strip_prefix("https://").and_then(|rest| rest.split('/').next());

        if authority.is_none_or(str::is_empty) {
            bail!("app link `{}` must be an https:// URL", self.name);
        }

        Ok(AppLink { name: self.name, url: self.url })
    }
}
