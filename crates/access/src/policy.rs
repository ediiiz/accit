//! The Policy: the administrator's rule for which hosts need a Grant (`GLOSSARY.md`).

use std::collections::BTreeSet;

use crate::Hostname;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Policy {
    /// Only these hosts are gated; every other covered host is open.
    Selected { hosts: BTreeSet<Hostname> },
    /// Every covered host is gated except these.
    All { excluded: BTreeSet<Hostname> },
}

impl Policy {
    pub fn is_gated(&self, host: &Hostname) -> bool {
        match self {
            Self::Selected { hosts } => hosts.contains(host),
            Self::All { excluded } => !excluded.contains(host),
        }
    }

    /// Selected mode with an empty list: allowed for staging, but worth a loud warning.
    pub fn gates_nothing(&self) -> bool {
        matches!(self, Self::Selected { hosts } if hosts.is_empty())
    }

    /// The hosts written in the Policy, sorted: the selection or the exclusions.
    pub fn listed(&self) -> impl Iterator<Item = &Hostname> {
        match self {
            Self::Selected { hosts } => hosts.iter(),
            Self::All { excluded } => excluded.iter(),
        }
    }
}
