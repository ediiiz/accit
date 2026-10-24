# Gate at the Traefik entrypoint, Policy inside accit

accit is attached once, as a ForwardAuth chain on the `websecure` entrypoint, so every router on it is checked without per-app labels or roles. Which hosts actually need a Grant is decided inside accit by its own Policy (selected or all mode), reloadable without restarting Traefik. We chose this over per-router middleware labels and over rewriting a Traefik IP allowlist on each Unlock, because both need Saltbox per-app changes or runtime writes to proxy config, and new apps would silently start out unchecked.

## Consequences

- Every request on the entrypoint, including open hosts, costs one internal verification call, and an accit outage blocks the whole entrypoint, Portal included. This fail-closed behaviour is intended.
- Traefik offers no per-router opt-out from entrypoint middleware, so open hosts must be expressed in the Policy, never in Traefik.
