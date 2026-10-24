//! Audit events: what happened to which Address, and when. Nothing that identifies a person.

use std::time::SystemTime;

use access::Address;

use crate::StoreError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Unlock,
    Revoke,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuditEvent {
    pub action: Action,
    pub address: Address,
    pub at: SystemTime,
}

impl Action {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Unlock => "unlock",
            Self::Revoke => "revoke",
        }
    }

    pub(crate) fn parse(text: &str) -> Result<Self, StoreError> {
        match text {
            "unlock" => Ok(Self::Unlock),
            "revoke" => Ok(Self::Revoke),
            _ => Err(StoreError::Action(text.to_owned())),
        }
    }
}
