# Trust Traefik by pinned address on a dedicated network, plus a token

accit treats a request as coming from Traefik only if its peer is Traefik's pinned address on a dedicated `accit` Docker network and it carries the internal proxy token. accit does not join the shared `saltbox` network, where every app container could reach it and Traefik's address changes on each redeploy. This needs a one-time change to Traefik's inventory (`traefik_role_docker_networks_custom`); we accepted that because trusting the whole shared subnet, or the token alone, would let any compromised app container impersonate Traefik.

## Considered options

- Stay on `saltbox` and trust the token alone: simplest, but the peer check means nothing.
- Resolve the `traefik` name at startup: the address still changes across redeploys and is shared-network reachable.

## Under Saltbox

Saltbox's `traefik-internal` router (Host `traefik`, entrypoint `internal` on :8080) serves `api@internal` without authentication, so any container on the `saltbox` network can read Traefik's dynamic configuration, the proxy token included. We accept this: the token alone does not pass, because accit also requires the peer to be Traefik's pinned address on the `accit` network, which no `saltbox` container holds. The Saltbox role says so after every deploy.
