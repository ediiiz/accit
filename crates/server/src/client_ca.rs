//! The public trust file named by `client_ca_file` (docs/spec.md section 11): accit reads only
//! the certificates' expiry.

use anyhow::{Context, Result};
use x509_parser::pem::Pem;

const CERTIFICATE: &str = "CERTIFICATE";

/// The earliest `notAfter` among the file's certificates, in Unix seconds.
pub(crate) fn earliest_expiry(pem: &[u8]) -> Result<i64> {
    let mut expiries = Vec::new();

    for block in Pem::iter_from_buffer(pem) {
        let block = block.context("not valid PEM")?;

        if block.label == CERTIFICATE {
            let certificate = block.parse_x509().context("not a valid certificate")?;
            expiries.push(certificate.validity().not_after.timestamp());
        }
    }

    expiries.into_iter().min().context("no certificate")
}
