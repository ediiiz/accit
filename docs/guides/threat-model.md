# Threat model

accit is a network barrier in front of self-hosted apps (`../spec.md` section 1). It decides, per request, whether the source Address may reach a Covered host. It is not an identity system, a VPN or a firewall, and each app keeps its own login. Terms: `../../GLOSSARY.md`.

## What it defends against

| Threat                                                      | Control                                                                                                                                                        |
| ----------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Internet scanner reaches a Gated host                       | No Grant for its Address: 403 `Forbidden`, no product name, no redirect (section 3).                                                                           |
| Stranger opens the Portal                                   | The Portal host demands the shared client certificate at the TLS handshake (section 10). Without it there is no page and no API.                               |
| A container on the Docker network impersonates Traefik      | accit joins only the dedicated `accit` network and accepts the pinned Traefik address plus the proxy token, compared in constant time (ADR 0002, section 3).   |
| Client forges `X-Forwarded-For`                             | Traefik does not trust the client's header (`trustForwardHeader: false`); a missing, multi-valued or unparsable value is denied (section 3).                   |
| Cross-site request unlocks or revokes                       | Exact `Origin`, session-bound CSRF token, JSON only, `SameSite=Strict` cookie, no CORS (section 8).                                                            |
| Guessing or flooding Unlock                                 | 5 mutations per minute per Address, at most 1,000 active Grants, bounded limiter and session state (sections 6 and 8).                                         |
| Moving the wall clock to extend a Grant                     | Each Grant also has a monotonic deadline; both must lie in the future. A drift over 5 minutes makes accit unready until restart (section 7).                   |
| Traefik misconfiguration drops client authentication        | `--core.strictTLSOptions=true`: a conflicting router on the Portal host is disabled instead of falling back to TLS options without client authentication (10). |
| A Revoke that could not be saved comes back after a restart | accit becomes unready (503 everywhere) and writes the `fail-closed` marker; it stays unready until `accit admin recover` (section 7).                          |
| accit down or not ready                                     | Every host is refused, never 2xx: 503 from accit when not ready, 500 from Traefik's ForwardAuth when accit cannot be reached (ADR 0001).                       |

## What a leaked certificate allows

The certificate and its password are the only secret a Credential holder has, and every holder has the same one. Whoever has both can open the Portal and Unlock their own Address, then reach the Gated hosts, still facing each app's login. Holders are indistinguishable by design; audit events record action, Address and time, never a person.

There is no per-holder revocation and no CRL or OCSP. Responding means rotating the CA, which replaces the certificate for everyone (operator guide, incident procedure). The `.p12` is password-protected, and the CA private key never exists in the image or a container: `scripts/client-ca` destroys it after signing.

## Failure behaviour (section 7)

- Unlock is visible to verification only after the database commit.
- Revoke removes the Grant from memory first. If the write then fails, accit fails closed and marks the data directory. If the marker cannot be written either, the process exits.
- Startup loads and validates unexpired Grants before it reports ready.
- Expired Grant rows are kept until audit retention passes (default 7 days); events never identify a person.

## Stated limits

- **Shared Addresses.** A Grant is for one Address, not one person or device. Everyone behind the same public Address passes once one of them Unlocks: a household, an office, a hotel or café Wi-Fi, a VPN exit, and carrier-grade NAT, where strangers on a mobile network may share an IPv4 Address. Keep `grant_duration` short where that matters.
- **Reassigned Addresses.** A Grant lasts until its expiry, even if the Address moves to someone else. Disable access when leaving a connection you do not control.
- **Total disk failure.** If a Revoke can be neither saved nor marked, the process exits and the Revoke is lost: after a restart, the Grant is valid again until it expires. After any storage incident, restart with the marker in place and discard Grants (operator guide, backups and restore).
- **One barrier, not a login.** Granted Addresses reach app login pages and anything wrong with them. Exposed app vulnerabilities stay the app's problem.
- **Uncovered routes.** Direct ports, other entrypoints, and raw TCP or UDP services never pass through accit.
- **Trusted networks pass without a Grant.** They are exactly as trustworthy as the source address Traefik sees. A hop that rewrites it, such as a router that masquerades forwarded traffic, can make the whole internet look inside the home. Check from outside (operator guide).
- **No upstream proxies.** Behind Cloudflare or any other proxy, accit sees the proxy's Address, so Grants and Trusted networks lose their meaning. Unsupported.
- **Traefik is trusted.** accit trusts the pinned Traefik address. Anyone who controls Traefik or the `accit` network controls the gate.
- **One Address per Unlock.** Dual-stack probing is deferred: a device that reaches the Portal over one IP family and an app over the other is granted only the Address the Portal saw.
- **Availability.** Because accit gates the entrypoint, an accit outage stops every Covered host, the Portal included. At most 1,000 Grants are active at once.
