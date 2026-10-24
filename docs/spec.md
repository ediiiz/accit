# accit specification

Condensed from "Saltbox Access — Project Specification" v1.2 (24 October 2026) plus the design decisions settled on 24 October 2026. Where a decision changes the source spec, it is marked **(changed)**. Terms are defined in `GLOSSARY.md`.

## 1. Product

accit lets Credential holders temporarily allow their current public Address to reach self-hosted HTTP apps behind Traefik on a Saltbox host. The Portal is protected by one shared client certificate. Apps keep their own login; their browser, mobile and TV clients need no changes. accit is a network barrier, not an identity system, VPN, firewall or SSO.

One Rust binary with the compiled Svelte Portal embedded, shipped as a Docker image, installed by an Ansible role (later milestone). No Node.js at runtime.

Not in v1: per-user permissions, user-entered addresses or ranges, automatic certificate distribution, app discovery, raw TCP/UDP, dual-stack probe hostnames **(changed: deferred)**, public help hostname **(changed: out-of-band guide instead)**, trusted upstream proxies such as Cloudflare **(changed: unsupported)**, metrics **(changed: logs and `admin status` only)**.

## 2. Scope of the current build

1. Core and Portal (source phases 2 and 3).
2. A Docker Compose harness: Traefik v3.7.16 with a test client CA, accit, two `whoami` apps (one gated, one open), and a client container with fixed source addresses. Most acceptance tests in section 10 run here, in CI.
3. Next milestone: the Saltbox role in `deploy/saltbox/roles/accit`, designed in its own session before it is built.

Real-device and real-user gates (source phases 1 and 5) stay manual.

## 3. Gate decision

The verification listener answers `GET /verify` with an empty body. Evaluated in this order:

| Step | Condition                                                                                                                     | Result |
| ---- | ----------------------------------------------------------------------------------------------------------------------------- | ------ |
| 1    | accit not ready (startup, fail-closed, clock anomaly)                                                                         | 503    |
| 2    | Peer is not the pinned Traefik address, or the proxy token is missing or wrong (constant-time compare)                        | 403    |
| 3    | `X-Forwarded-For` is missing, has more than one value or does not parse; `X-Forwarded-Host` is not a valid canonical hostname | 403    |
| 4    | Host is the Portal hostname                                                                                                   | 204    |
| 5    | Address is inside a Trusted network                                                                                           | 204    |
| 6    | Host is an Open host under the Policy                                                                                         | 204    |
| 7    | Address has a Grant valid on both the wall clock and the monotonic deadline                                                   | 204    |
| 8    | Otherwise                                                                                                                     | 403    |

- Hostnames are canonicalized: lowercase, trailing dot removed, port removed, IDNA ASCII form; anything else (userinfo, path, wildcard, IP literal for a covered host) is invalid.
- Addresses are canonicalized; IPv4-mapped IPv6 becomes IPv4. A public IPv6 Address is global unicast `2000::/3` minus `2001::/23`, `2001:db8::/32` and `3fff::/20` (6to4 `2002::/16` stays public). With `profile: production`, a non-public Address that is not in a Trusted network is rejected at step 3.
- Verification never mutates state and never renews a Grant.
- The 403 body is the plain text `Forbidden`, with no product name. No redirect.

## 4. Policy

```yaml
protection:
  mode: selected # or: all
  hosts: # selected mode only
    - jellyfin.example.com
  # excluded_hosts:       # all mode only
  #   - status.example.com
  trusted_networks: # optional, default empty
    - 192.168.1.0/24
  trusted_networks_exclude:
    - 192.168.1.1 # the router, see section 5
```

- `mode` is required; there is no default and no fallback.
- Rejected at load: unknown fields, unknown mode, `hosts` in all mode, `excluded_hosts` in selected mode, wildcards, paths, the Portal hostname in either list. Hosts are canonicalized and deduplicated.
- An empty `hosts` list in selected mode is allowed and reported loudly by `admin check` and at startup.
- Reload: `accit admin reload` over the admin socket, or SIGHUP; both run the same code. An invalid file keeps the previous Policy and returns the error. **(changed: no file watching)**
- Switching mode or lists keeps all Grants.
- App links are display-only and never define Policy.

## 5. Trusted networks (home access)

For split-DNS setups where the app hostnames resolve to the server's LAN address at home.

- Admin-declared ranges whose Addresses pass the gate on every host without a Grant. Apply in both modes.
- Only private, unique-local or CGNAT ranges are accepted: `10/8`, `172.16/12`, `192.168/16`, `fc00::/7`, `100.64/10`. Public ranges and `0.0.0.0/0` are rejected.
- A Trusted network must not overlap the Docker networks configured in `docker_networks` (the `saltbox` and `accit` networks and their gateways).
- `trusted_networks_exclude` removes single addresses, such as a router that masquerades forwarded traffic.
- Deployment check: from outside the home, without a Grant, a gated host must return 403. The role does this where it can; the operator guide documents it as a manual step otherwise.
- Public IPv6 prefixes are never trusted. The operator guide says to keep the internal DNS override IPv4-only.

## 6. Grants

- Unlock creates or renews the Grant of the requesting Address: `expiry = server_now + duration`. Repeated Unlocks never accumulate.
- Duration: default 7 days, allowed 1 hour to 30 days.
- Exact Addresses only, never widened to a range.
- At most 1,000 active Grants. A new Address is refused at the cap; renewing an existing Grant always works. Expired Grants are removed before the cap check.
- Unlock from a Trusted network is refused; the Portal does not offer it there.
- Revoke removes the Grant of the requesting Address only. Ordinary Credential holders cannot list or revoke other Grants.

## 7. State, storage and clocks

- In-memory map for verification, SQLite (sqlx, versioned migrations) for durability. One serialized mutation path.
- Unlock is visible to verification only after the database commit succeeds.
- Revoke removes the Grant from memory first, then writes. If the write fails: accit becomes unready (503 everywhere) and writes a fail-closed marker file in the data directory. On startup with the marker present, accit stays unready until `accit admin recover --discard-grants` (recommended) or `--keep-grants`. If the marker cannot be written either, the process exits with an error. Residual risk (total disk failure, then restart) is a documented limit in the threat model.
- Startup loads and validates unexpired Grants before reporting ready.
- Each Grant has a wall-clock expiry and, in memory, a monotonic deadline set when it is created or loaded. Both must lie in the future.
- A background check compares elapsed wall time with elapsed monotonic time. A difference over 5 minutes makes accit unready until restart.
- Expired Grant rows are kept until audit retention passes, so status can report `expired`.
- Audit events: action, Address, time. Retention 7 days, configurable. Events do not identify a person.

## 8. Portal API (listener 8080)

| Endpoint               | Behaviour                                                                                                                                 |
| ---------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| `GET /`, static assets | Embedded prerendered app; never mutates                                                                                                   |
| `GET /api/status`      | `state` (`none`, `active`, `expired`, `trusted`), expiry, server time, Address and family, app links, title, default language, CSRF token |
| `POST /api/unlock`     | Create or renew the Grant of the requesting Address; returns committed state                                                              |
| `POST /api/revoke`     | Revoke the Grant of the requesting Address; returns resulting state                                                                       |

- Every successful answer (200) from the three endpoints is one JSON object: `state`, `expiry` (`null` without a Grant) and `server_time` as Unix seconds, `address`, `family` (`ipv4` or `ipv6`), `apps` (`{name, url}`), `title`, `default_language` and `csrf_token`. Refusals have a plain status-text body: 400 (body not `{}`), 403, 409 (Unlock at the Grant cap), 413 (body over 1 KiB), 415, 429 (with `Retry-After` in seconds), 503. An unknown `/api/*` path is 404 and a wrong method is 405.
- With `profile: production`, every endpoint answers 403 for a non-public Address that is not in a Trusted network.
- Every request must come from the pinned Traefik address with the correct proxy token and the exact Portal Host; otherwise 403.
- POSTs: `application/json` only, bounded body, unknown fields rejected, exact `Origin` required, session-bound CSRF token.
- Sessions exist only to carry the CSRF token: in memory, at most 10,000, oldest evicted first, 15 minutes, lost on restart. Cookie: `Secure`, `HttpOnly`, `SameSite=Strict`, host-only.
- No CORS.
- Rate limit on mutations: 5 per minute per Address, bounded limiter state. Verification has its own separate budget.
- Headers: strict CSP (`default-src 'self'`, no unsafe eval, `frame-ancestors 'none'`, `object-src 'none'`, `base-uri 'none'`, restricted `connect-src`), `no-store` on HTML, API and verify responses, immutable caching for hashed assets, `nosniff`, `Referrer-Policy: no-referrer`. HSTS off by default.

Verification listener (8081): `/verify`, `/healthz` (liveness), `/readyz` (storage and state readiness). Unknown paths on either listener return 404.

## 9. Portal UX

One prerendered SvelteKit route, Paraglide (English, German; browser language, admin default), Tailwind, and only the components the screen needs. Initial HTML, CSS and JS under 150 KiB compressed, enforced in CI. No service worker, analytics, third-party fonts or CDNs.

| State               | Content                                                                                         | Primary action        |
| ------------------- | ----------------------------------------------------------------------------------------------- | --------------------- |
| Not enabled         | "Access your apps." "Enable access for your current connection."                                | Enable access         |
| Submitting          | "Enabling access…"                                                                              | Disabled, in progress |
| Enabled             | "Access is enabled." "Available until [localized date/time, including timezone]." **(changed)** | Extend access         |
| Expired             | "Your access has expired."                                                                      | Enable access         |
| Connection changed  | "Your connection changed. Enable access again."                                                 | Enable access         |
| Home network        | "You're on your home network. Your apps work without enabling access."                          | none                  |
| Service unavailable | "We couldn't enable access. Please try again."                                                  | Try again             |
| Rate limited        | "Please wait a moment before trying again."                                                     | Retry after countdown |

- "Connection changed": the Portal stores the Address of its last successful Unlock in `localStorage` and compares it with the status Address.
- Secondary action "Disable access for this connection", with confirmation. If it fails, Service unavailable reads "We couldn't disable access. Please try again." **(changed)**
- Tip: "To enable another device, connect this phone to the same Wi-Fi."
- Troubleshooting panel: current Address and family.
- Title configurable, default "Access".
- App links (`https://` only) appear after Unlock and on the home network.
- Status is refreshed on focus and immediately before Unlock.
- 320 px minimum width, large touch targets, keyboard access, WCAG 2.2 AA, announced status changes, reduced motion respected, status conveyed by text as well as colour.

## 10. Traefik integration

- Traefik ≥ v3.7.16 (mTLS and ForwardAuth bypass fixes up to that release). The role checks the running version.
- Entrypoint `websecure` gets one chain, `accit@file`:
  1. `headers` sets `X-Accit-Proxy-Token`.
  2. `forwardAuth` to `http://<accit>:8081/verify` with `trustForwardHeader: false` and `authRequestHeaders: [X-Accit-Proxy-Token]`; no body forwarded.
  3. `headers` deletes `X-Accit-Proxy-Token`.
- The Portal router: exact `Host` rule, its own TLS options with `RequireAndVerifyClientCert` and the dedicated client CA, and a `headers` middleware that sets the token again for the Portal listener.
- `--core.strictTLSOptions=true`, so a conflicting router fails closed instead of silently dropping client authentication.
- Dedicated Docker network `accit` with pinned addresses: Traefik joins it via `traefik_role_docker_networks_custom`; accit joins only this network. accit trusts exactly Traefik's pinned address.
- No dynamic rewrites: Unlock, renew and Revoke never touch Traefik, Docker or the firewall.

## 11. Administration

`accit admin <command>`, run in the container, talks to the daemon over a Unix socket (owner-only permissions, never exposed through Traefik):

- `list`, `revoke <address>`, `revoke-all`
- `check [file]`: validate a config without applying it; dry-run coverage table; client CA expiry; empty-selection and link warnings. Runs locally in the CLI process, not over the socket
- `reload`
- `status`: counts of active Grants, verifications per minute, denials since start
- `recover --discard-grants | --keep-grants` (`--keep-grants` may restore a Grant whose Revoke failed)

Client CA and shared bundle: `scripts/client-ca` on the admin's machine creates the dedicated CA, a one-year shared leaf and the password-protected `.p12`, and handles rotation (new CA). The CA private key never enters the image or container. accit reads only the public trust file and warns from 30 days before expiry.

## 12. Configuration

YAML at `/config/accit.yaml`, unknown fields rejected. Keys: `profile` (`production` or `test`, required; `test` allows private Addresses and logs a warning at startup and hourly), `public_origin`, `title`, `default_language`, `grant_duration`, `max_grants`, `audit_retention`, `apps`, `protection`, `trusted_proxy` (exact address), `proxy_token_file`, `docker_networks`, `client_ca_file`, `listen` (portal, verify), `database`, `admin_socket`.

With `profile: production`, `public_origin`, `trusted_proxy`, `proxy_token_file` and `database` are required; a config without them does not load. With `profile: test` they may be omitted for local development; without `database`, Grants live only in memory and accit logs a warning. `public_origin` is exactly `https://<hostname>` with an optional `:port` (1 to 65535; a default `:443` is dropped, as browsers omit it from `Origin`) and an optional trailing `/`, and no path, query, fragment or userinfo; the Portal host is its hostname, parsed like a Policy entry. The Portal host may not appear in either Policy list whenever `public_origin` is set. `trusted_proxy` is an exact IP address, not an Address in the `GLOSSARY.md` sense; it is stored canonicalized (an IPv4-mapped IPv6 address becomes IPv4).

Value formats: durations are a whole number followed by `s`, `m`, `h` or `d` (for example `90m`, `7d`). `default_language` is `en` or `de`, default `en`. App links are `{name, url}`, with an `https://` URL whose host parses like a Policy entry. `max_grants` is at least 1. `audit_retention` is at least 1 hour.

## 13. Runtime hardening

Non-root, no capabilities, `no-new-privileges`, read-only root filesystem, size-limited tmpfs where needed, writable data directory only, config and secrets read-only, no published ports, no host networking, no Docker socket. Logs are structured JSON and never contain tokens, cookies, request bodies, certificate material or full URLs. Linux amd64 and arm64 images.

## 14. Acceptance targets

As in the source spec section 10, run against the Compose harness where possible. Performance targets: at most 64 MiB RSS idle with 1,000 Grants; p95 verification under 5 ms at 500 checks per second on a documented 2-vCPU host. Additional tests from the design session:

- A Trusted-network source passes every gated host without a Grant; a public IPv6 source inside the home still needs one.
- A Trusted network overlapping a Docker network, or a public range, is rejected at load.
- A multi-valued `X-Forwarded-For` is denied.
- A conflicting second router on the Portal host is refused by Traefik (strict TLS options).
