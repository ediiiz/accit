//! The gate decision: steps 3 to 8 of `docs/spec.md` section 3. Readiness (step 1) and proxy trust
//! (step 2) are the verification listener's.

use crate::{Address, Hostname, Policy, TrustedNetworks};

#[derive(Clone, Copy, Debug)]
pub struct Gate<'a> {
    pub policy: &'a Policy,
    pub trusted: &'a TrustedNetworks,
    pub portal_host: Option<&'a Hostname>,
    /// `profile: production`: a non-public Address outside the Trusted networks is rejected at step 3.
    pub public_only: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    Deny,
}

fn exactly_one<'v>(values: impl IntoIterator<Item = &'v [u8]>) -> Option<&'v [u8]> {
    let mut values = values.into_iter();

    match (values.next(), values.next()) {
        (Some(value), None) => Some(value),
        _ => None,
    }
}

impl Gate<'_> {
    /// `forwarded_for` and `forwarded_host`: every value of the request's header.
    /// `has_grant` is asked only when the decision depends on it.
    pub fn decide<'v>(
        &self,
        forwarded_for: impl IntoIterator<Item = &'v [u8]>,
        forwarded_host: impl IntoIterator<Item = &'v [u8]>,
        has_grant: impl FnOnce(Address) -> bool,
    ) -> Verdict {
        let Some(address) = Address::from_forwarded_for(forwarded_for)
            .ok()
            .filter(|address| address.is_public() || !self.public_only || self.trusted.contains(*address))
        else {
            return Verdict::Deny;
        };
        let Some(host) =
            exactly_one(forwarded_host).and_then(|value| Hostname::from_forwarded_host(value).ok())
        else {
            return Verdict::Deny;
        };

        if self.portal_host == Some(&host)
            || self.trusted.contains(address)
            || !self.policy.is_gated(&host)
            || has_grant(address)
        {
            return Verdict::Pass;
        }

        Verdict::Deny
    }
}
