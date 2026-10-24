# accit

Lets trusted people temporarily allow their current public address to reach self-hosted web apps behind a shared reverse proxy. It is a network barrier in front of the apps, not an identity system: each app keeps its own login.

## Access

**Grant**:
Permission for one Address to pass the gate until a fixed expiry.
_Avoid_: Allowlist entry, session, unlock (as a noun)

**Address**:
The canonical public IP address accit observed for a request, IPv4 or IPv6, exact and never widened to a range.
_Avoid_: Connection, IP range, client

**Unlock**:
The deliberate user action that creates a Grant for the current Address, or renews it to a full duration from now.
_Avoid_: Enable, extend, renew (as separate actions)

**Revoke**:
Removing the Grant of an Address before it expires.
_Avoid_: Disable, delete

**Credential holder**:
Anyone holding the shared client certificate; all holders are indistinguishable and equally privileged.
_Avoid_: User, account, member

## Coverage

**Policy**:
The administrator's rule for which hosts need a Grant: either selected mode (only listed hosts) or all mode (every host except listed exclusions).
_Avoid_: Allowlist, rules, config

**Covered**:
Said of a route whose requests pass through the accit check.
_Avoid_: Protected

**Gated host**:
A covered host that needs a Grant under the active Policy.
_Avoid_: Protected host, locked host

**Open host**:
A covered host that passes the gate by Policy; its own login still applies.
_Avoid_: Public host, unprotected host, excluded host

**Trusted network**:
A private address range the administrator declares as inside the home; its Addresses pass the gate on every host without a Grant.
_Avoid_: LAN allowlist, internal network, whitelist

**Uncovered**:
Said of a route outside the mechanism, such as a direct port, another entrypoint or a raw TCP/UDP service.
_Avoid_: Unprotected, bypass

**Portal**:
The certificate-protected page where a Credential holder sees the state of their Address and performs Unlock or Revoke.
_Avoid_: Dashboard, admin page
