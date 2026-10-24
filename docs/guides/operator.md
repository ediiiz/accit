# Operator guide

For the person who runs accit next to Traefik and Docker. Spec: `../spec.md`. Terms: `../../GLOSSARY.md`. Threat model and limits: `threat-model.md`. Holder instructions: `onboarding.en.md`, `onboarding.de.md`.

The steps are generic Traefik v3.7.16 or later on Docker. A Saltbox role is a later milestone. The tested reference configuration is the Compose harness in `../../harness/`: `compose.yml`, `traefik/traefik.yml`, `traefik/accit.yml.in`, `accit.yaml`. Nothing here was run on real devices or a production host; the harness is what `scripts/e2e` exercises.

## Secrets

| Secret                                           | Where it lives                                        | Never                       |
| ------------------------------------------------ | ----------------------------------------------------- | --------------------------- |
| Proxy token (`proxy-token`)                      | accit's config directory, and in Traefik's chain file | in git, chat, logs          |
| Rendered Traefik chain file (contains the token) | Traefik's dynamic directory                           | in git                      |
| `client.p12` and its password                    | the admin's machine, then the holders                 | together, or in git         |
| CA private key                                   | nowhere: `scripts/client-ca` deletes it after use     | on the server, in the image |

Commit templates with placeholders only. `.gitignore` already excludes `client-ca/` and `*.p12`.

## 1. Client certificate

On your own machine, not the server:

```sh
scripts/client-ca create client-ca    # asks for a password of at least 12 characters
```

This writes `client-ca/ca.pem` (public) and `client-ca/client.p12`. The shared leaf is valid for 365 days and the CA for 366, so accit's warning (30 days before the CA expires) also precedes the leaf's expiry. The command prints the CA fingerprint and both expiry dates. Set `ACCIT_P12_PASSWORD` instead of the prompt for scripted use.

- Server side: copy `ca.pem` to Traefik's certificate directory and to accit's config directory as `client-ca.pem`. It is public.
- Holders: send `client.p12` and its password by different routes, with `onboarding.en.pdf` or `onboarding.de.pdf`.
- Renewal: `scripts/client-ca rotate client-ca` makes a new CA and leaf and moves the old `ca.pem` to `client-ca/retired/<time>/`. For a smooth change, serve both for a while with `cat client-ca/retired/*/ca.pem client-ca/ca.pem > trust.pem`, and drop the old one once everyone has the new file. Do this before the expiry warning of `accit admin check` turns into an error.

## 2. Docker network

accit trusts exactly one peer: Traefik's address on a dedicated network (ADR 0002). Create the network with a fixed subnet and give both containers fixed addresses.

```yaml
# accit service; Traefik joins the same network from its own Compose file
services:
  accit:
    image: ghcr.io/ediiiz/accit:<version> # pin a release tag
    read_only: true
    cap_drop: [ALL]
    security_opt: ["no-new-privileges:true"]
    volumes:
      - ./accit/config:/config:ro
      - ./accit/data:/data # owned by uid 65532
    networks:
      accit:
        ipv4_address: 172.29.20.3
  traefik:
    networks:
      accit:
        ipv4_address: 172.29.20.2 # plus its existing networks
networks:
  accit:
    internal: true
    ipam:
      config:
        - subnet: 172.29.20.0/24
```

Publish no ports on accit. The data directory must be writable by uid 65532: `mkdir -p accit/data && chown 65532:65532 accit/data`.

## 3. Traefik

Static configuration (`harness/traefik/traefik.yml` has the full file):

```yaml
core:
  strictTLSOptions: true # a conflicting router fails closed
entryPoints:
  websecure:
    address: ":443"
    http:
      middlewares: [accit@file]
      tls: {}
```

Dynamic file `accit.yml`, from the harness template `harness/traefik/accit.yml.in`. Replace the hostname, the accit address and the Portal certificate setting for your site:

- Chain `accit`: `headers` sets `X-Accit-Proxy-Token`, `forwardAuth` calls `http://172.29.20.3:8081/verify` with `trustForwardHeader: false` and `authRequestHeaders: [X-Accit-Proxy-Token]`, a second `headers` deletes the token.
- Router `portal`: rule ``Host(`portal.example.com`)``, middleware `accit-set-token`, service `http://172.29.20.3:8080`, `tls.options: mtls`, plus your certificate resolver.
- TLS option `mtls`: `clientAuthType: RequireAndVerifyClientCert`, `caFiles: [/certs/client-ca.pem]` (the `ca.pem` from section 1), `minVersion: VersionTLS12`.

Generate the token once and render the chain file; keep both out of git:

```sh
umask 077
openssl rand -hex 32 > accit/config/proxy-token
sed "s/__PROXY_TOKEN__/$(cat accit/config/proxy-token)/" accit.yml.in > traefik/dynamic/accit.yml
chown 65532:65532 accit/config/proxy-token
```

The proxy token file must stay readable by accit's user; the rendered chain file by Traefik's.

Traefik must be v3.7.16 or later; check with `docker exec traefik traefik version`.

## 4. accit configuration

`/config/accit.yaml`, unknown keys are errors. Example for production:

```yaml
profile: production
public_origin: https://portal.example.com
title: Access
trusted_proxy: 172.29.20.2
proxy_token_file: /config/proxy-token
client_ca_file: /config/client-ca.pem
database: /data/accit.db
docker_networks: [172.29.20.0/24] # every Docker network Traefik is on
protection:
  mode: selected
  hosts: [jellyfin.example.com]
  trusted_networks: [192.168.1.0/24]
  trusted_networks_exclude: [192.168.1.1]
apps:
  - name: Jellyfin
    url: https://jellyfin.example.com
```

| Key                                   | Required          | Default                            | Meaning                                                                                  |
| ------------------------------------- | ----------------- | ---------------------------------- | ---------------------------------------------------------------------------------------- |
| `profile`                             | yes               |                                    | `production`, or `test` (allows private Addresses, logs a warning at startup and hourly) |
| `public_origin`                       | production        |                                    | `https://<hostname>[:port]`; the hostname is the Portal host                             |
| `trusted_proxy`                       | production        |                                    | Traefik's exact address on the `accit` network                                           |
| `proxy_token_file`                    | production        |                                    | file holding the proxy token                                                             |
| `database`                            | production        |                                    | SQLite file; without it Grants live in memory only                                       |
| `client_ca_file`                      | no                |                                    | public CA file; accit reads only its expiry                                              |
| `docker_networks`                     | no                | empty                              | CIDRs of Docker networks; a Trusted network may not overlap them                         |
| `protection.mode`                     | yes               |                                    | `selected` (only `hosts` are gated) or `all` (everything except `excluded_hosts`)        |
| `protection.hosts`                    | no, selected only | empty                              | Gated hosts, exact names, no wildcards                                                   |
| `protection.excluded_hosts`           | no, all only      | empty                              | Open hosts                                                                               |
| `protection.trusted_networks`         | no                | empty                              | private, unique-local or CGNAT ranges that pass without a Grant                          |
| `protection.trusted_networks_exclude` | no                | empty                              | single addresses removed from the Trusted networks                                       |
| `apps`                                | no                | empty                              | `{name, url}` links the Portal shows; `https://` only; display only, never Policy        |
| `title`                               | no                | `Access`                           | Portal title                                                                             |
| `default_language`                    | no                | `en`                               | `en` or `de`; the Portal follows the browser first                                       |
| `grant_duration`                      | no                | `7d`                               | 1 hour to 30 days, written as `90m`, `12h`, `7d`                                         |
| `max_grants`                          | no                | `1000`                             | at least 1                                                                               |
| `audit_retention`                     | no                | `7d`                               | at least 1 hour                                                                          |
| `listen.portal`, `listen.verify`      | no                | `0.0.0.0:8080`, `0.0.0.0:8081`     | listener addresses inside the container                                                  |
| `admin_socket`                        | no                | `admin.sock` in the data directory | Unix socket of `accit admin`                                                             |

The Portal host may not appear in `hosts` or `excluded_hosts`. Switching mode or lists keeps all Grants.

## 5. Day-to-day

`accit admin` runs in the container and talks to the daemon over its socket. With the container named `accit`:

| Task                        | Command                                                                                        |
| --------------------------- | ---------------------------------------------------------------------------------------------- |
| Validate a candidate config | `docker run --rm -v "$PWD/accit/config:/config:ro" ghcr.io/ediiiz/accit:<version> admin check` |
| Apply a changed config      | `docker exec accit /accit admin reload` (or `docker kill -s HUP accit`)                        |
| Active Grants               | `docker exec accit /accit admin list`                                                          |
| Counts                      | `docker exec accit /accit admin status`                                                        |
| Revoke one Address, or all  | `docker exec accit /accit admin revoke <address>`, `admin revoke-all`                          |

`admin check` prints which hosts are gated and warns about an empty selection, app links to Open hosts and a client CA within 30 days of expiry. An expired CA is an error. A changed file does nothing until `reload`; an invalid file keeps the previous Policy and returns the error.

Logs are JSON on the container's output and contain no tokens, cookies or certificate material.

## 6. Trusted networks (home access)

For split DNS, where app hostnames resolve to the server's LAN address at home, devices on the home network pass every gated host without a Grant. Both modes.

1. List the home range in `trusted_networks`. Only `10/8`, `172.16/12`, `192.168/16`, `fc00::/7` and `100.64/10` ranges are accepted, and not ones overlapping `docker_networks`.
2. If a router masquerades forwarded traffic, Traefik sees the router's address instead of the real source. Add it to `trusted_networks_exclude`.
3. Keep the internal DNS override **IPv4-only**: answer with the server's LAN IPv4 address and publish no AAAA record for these names. A public IPv6 Address is never trusted, so a device that reaches the server over IPv6 still needs a Grant.

**Outside-in check**, after every change to the network or this list. From a device outside the home, such as a phone on mobile data, without a Grant:

```sh
curl -sS -o /dev/null -w '%{http_code}\n' https://jellyfin.example.com/    # a gated host
```

It must print `403`. Anything else means Traefik sees outside traffic as inside, typically through a rewriting hop: remove the range or exclude the address, then repeat. A device at home on the same URL should reach the app, and the Portal says "You're on your home network."

## 7. Backups and restore

Your config and secrets are backed up like any other config. The data directory holds only Grants and audit events, which contain Addresses (personal data): keep its backups short-lived.

Online backup, no outage (an accit stop would block every host):

```sh
sudo sqlite3 accit/data/accit.db ".backup 'accit-backup.db'"
```

Restore, **discarding Grants** (default). A restored database can hold Grants that were Revoked after the backup, so none are trusted:

1. `docker stop accit`.
2. Put the backup at `accit/data/accit.db`; remove `accit.db-wal` and `accit.db-shm` beside it; `chown 65532:65532` the files.
3. Create the marker: `touch accit/data/fail-closed`.
4. `docker start accit`. It stays unready (503 on every host) while the marker exists.
5. `docker exec accit /accit admin recover --discard-grants`. Holders Enable access again.

`--keep-grants` instead keeps the stored Grants and may bring back one whose Revoke failed. Use it only when you know nothing was Revoked since the backup.

The same marker and recovery apply after any storage incident, and when accit reports it failed closed on its own.

## 8. Incident: certificate or password leaked, or access misused

Goal: every old certificate stops working, every Grant ends, and open connections are cut. Do it in this order.

1. **Rotate the CA** on your machine: `scripts/client-ca rotate client-ca`. Do not serve the retired CA alongside the new one.
2. **Install the new `ca.pem`** as Traefik's client CA file and as accit's `client_ca_file`, replacing the old one.
3. **Close Portal connections.** Restart Traefik: `docker restart traefik`. A restart is the sure way to apply the new CA file and to end established connections, including any Portal session opened with the old certificate.
4. **Revoke all Grants:** `docker exec accit /accit admin revoke-all`. Confirm with `admin list`.
5. **Check:** `docker exec accit /accit admin check` shows the new CA's expiry, and the old `client.p12` no longer opens the Portal.
6. **Redistribute** the new `client.p12` and its password by separate routes, and delete the old `.p12` files and `client-ca/retired/`.

If the proxy token leaked, or a container on the `accit` network was compromised, also generate a new token, render the chain file again (section 3), then restart Traefik and accit.

## 9. Deployment checks

1. `docker exec accit /accit admin check` lists gated hosts as intended and shows no warning you did not expect.
2. From outside, without a Grant: a gated host answers `403`, an Open host reaches its app, and the Portal fails the TLS handshake without the certificate.
3. With the certificate, the Portal opens; Enable access; the gated host now reaches its app; Disable access; it answers `403` again.
4. The outside-in check from section 6 when Trusted networks are configured.
5. `docker exec accit /accit admin status` counts denials.
