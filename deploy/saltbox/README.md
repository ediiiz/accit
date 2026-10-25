# Saltbox role

Deploys accit on a [Saltbox](https://docs.saltbox.dev) host through saltbox_mod. Spec: `../../docs/spec.md` sections 10 to 13. Trust model: `../../docs/adr/0002-trust-traefik-by-pinned-address-and-token.md`. Generic Traefik setup and day-to-day commands: `../../docs/guides/operator.md`.

**Never run against a real Saltbox host**; only `check` (below) has run. The first run stops at the Traefik wiring step.

## Install

1. Clone this repository on the host and link the role into saltbox_mod (`sb install saltbox-mod` first if `/opt/saltbox_mod` is missing). The repository is private: clone with GitHub credentials. Link, do not copy: `files/client-ca` is a symlink to `scripts/client-ca`.

   ```sh
   sudo git clone https://github.com/ediiiz/accit.git /srv/git/accit
   sudo ln -s /srv/git/accit/deploy/saltbox/roles/accit /opt/saltbox_mod/roles/accit
   ```

2. Add the role under `# Apps Start` in `/opt/saltbox_mod/saltbox_mod.yml`. `sb install mod-<tag>` runs the playbook tag after `mod-`:

   ```yaml
   - { role: accit, tags: ["accit", "accit-remove"] }
   ```

3. Set the inventory (`sb edit inventory`, `/srv/git/saltbox/inventories/host_vars/localhost.yml`), at least:

   ```yaml
   accit_role_docker_image_tag: v1.2.3 # exact release
   accit_role_protection_mode: selected # or all
   accit_role_protection_hosts:
     - jellyfin.example.com
   ```

4. `sb install mod-accit`. The first run deploys accit, then stops and prints the Traefik lines to add to the inventory. Add them, run `sb install traefik`, then `sb install mod-accit` again.

## Inventory

| Variable                                     | Default           | Meaning                                                                                                  |
| -------------------------------------------- | ----------------- | -------------------------------------------------------------------------------------------------------- |
| `accit_role_docker_image_tag`                | required          | exact release tag (`v1.2.3`); `latest` and other floating tags are refused                               |
| `accit_role_protection_mode`                 | required          | `selected` or `all`                                                                                      |
| `accit_role_protection_hosts`                | `[]`              | Gated hosts, selected mode                                                                               |
| `accit_role_protection_excluded_hosts`       | `[]`              | Open hosts, all mode                                                                                     |
| `accit_role_trusted_networks`, `..._exclude` | `[]`              | Trusted networks (spec section 5)                                                                        |
| `accit_role_apps`                            | `[]`              | Portal app links, `{name, url}`                                                                          |
| `accit_role_help_host`                       | `""`              | host of the out-of-band guide; Open in all mode                                                          |
| `accit_role_config_custom`                   | `{}`              | other `accit.yaml` keys (`title`, `default_language`, `grant_duration`, ...); the keys the role sets win |
| `accit_role_web_subdomain`                   | `access`          | Portal host is `<subdomain>.<user.domain>`; DNS record created the Saltbox way, never Cloudflare-proxied |
| `accit_role_network_subnet`                  | `172.31.250.0/29` | `accit` Docker network; the run fails if it overlaps another Docker network                              |
| `accit_role_network_traefik_address`         | `172.31.250.2`    | Traefik's pinned address, accit's `trusted_proxy`                                                        |
| `accit_role_network_accit_address`           | `172.31.250.3`    | accit's pinned address                                                                                   |
| `accit_role_traefik_alias_headers_strategy`  | `delete`          | `aliasHeadersStrategy` on `websecure`; affects every app there (spec section 10)                         |
| `accit_role_client_ca_file`                  | `""`              | your own client CA certificate on the host; empty = the role generates CA and shared certificate         |

`accit.yaml` is rendered from these on every run (`profile: production`) and checked with the pinned image's `accit admin check`. A change to `protection` alone is applied with `accit admin reload`; any other change restarts accit, since reload applies only `protection`. A run that fails before applying the change leaves `config-pending`, and the next run applies it. `docker_networks` lists every Docker subnet present at the time of the run.

Gated hosts must not be Cloudflare-proxied: accit would see Cloudflare's address instead of the client's (spec section 1).

## Files on the host

| Path                        | Owner, mode          | Holds                                                                                 |
| --------------------------- | -------------------- | ------------------------------------------------------------------------------------- |
| `/opt/accit/config/`        | `root:<gid>` `0750`  | `accit.yaml`, `proxy-token` (`0640`), `client-ca.pem`; mounted read-only at `/config` |
| `/opt/accit/data/`          | `<uid>:<gid>` `0700` | SQLite database, admin socket; the container runs as `<uid>:<gid>`                    |
| `/opt/accit/pki/`           | `root` `0700`        | `ca.pem` (`0644`), `client.p12`, `client.p12.password`, `retired/`; no CA key         |
| `/opt/accit/backups/`       | `root` `0700`        | database copies, one per image tag left                                               |
| `/opt/accit/deployed-tag`   | `root` `0644`        | the running image tag                                                                 |
| `/opt/accit/config-pending` | `root` `0644`        | `reload` or `restart` not yet applied; present only after a failed run                |
| `/opt/traefik/accit.yml`    | `root` `0600`        | chain, Portal router, mTLS option; holds the proxy token                              |

The proxy token and the `.p12` password are generated on the host and never appear in the inventory or Ansible's output. Hand out `client.p12` and its password by different routes, with the onboarding guide.

## Rotation

Rotation flags are extra variables, passed to the playbook directly:

```sh
cd /opt/saltbox_mod
sudo ansible-playbook saltbox_mod.yml --tags accit -e accit_rotate_proxy_token=true # new token, accit restarts
sudo ansible-playbook saltbox_mod.yml --tags accit -e accit_rotate_client_ca=true   # new CA and client.p12
```

After a client CA rotation, `docker restart traefik` closes Portal connections made with the old certificate, then `docker exec accit /accit admin revoke-all` if the old one leaked (operator guide section 8). With `accit_role_client_ca_file`, rotate your own CA and update that file.

## Upgrade and rollback

Change `accit_role_docker_image_tag` and re-run. Before every tag change the role stops accit and copies the database to `/opt/accit/backups/accit-<old tag>.db`. Going back to an older tag restores the copy taken when that tag was left, then discards its Grants (`accit admin recover --discard-grants`): the copy may hold Grants Revoked since. A downgrade without a copy is refused.

Re-running with the same tag and an unchanged config does not restart accit: each restart is an outage for every app on `websecure`. Without a recorded tag, an existing database is copied to `accit-untagged.db` first.

## Checks

Any failure fails the run:

- Traefik's wiring: `accit@file` on `websecure`, `aliasHeadersStrategy`, `core.strictTLSOptions=true`, its pinned address on the `accit` network
- Traefik v3.7.16 or later, accit at its pinned address, `accit admin check` exits 0, `/readyz`
- the `accit@file` chain Traefik loaded sets, verifies, then deletes the token
- a Gated host answers `403 Forbidden` to the host itself
- through the host's public address (`ip_address_public`), with the generated certificate, the Portal's `/api/status` reports a public Address without a Grant, and a Gated host answers `403 Forbidden`; reported as skipped when the Portal does not answer, sees no public Address, or reports a Trusted network or a Grant, and always with your own client CA
- the Portal presents a server certificate, fails the TLS handshake without the client certificate, and answers with the generated one

The probes reach Traefik where it publishes `websecure`: loopback when bound to every address, else the bound address (Saltbox's Tailscale mode binds `ip_address_public`), at the published port.

On a host that owns its public address, the public-address probe usually reports skipped: with Saltbox's `userland-proxy: false`, Docker rewrites the host's own connection to the bridge gateway, a Docker address. Check from outside instead (spec section 5).

From the host itself accit sees a Docker address and refuses it at the Address check, so only the public-address probe reaches the Policy. No Saltbox app echoes request headers: the token's removal is checked on the loaded chain, end to end only in the Compose harness (`crates/e2e`).

Then a coverage report: every router as host, entrypoint, Gated / Open / Uncovered, by the rule of `accit admin check`. Only `websecure` carries the chain; routers on other entrypoints, `tailscale-websecure` included, are Uncovered.

Saltbox's `traefik-internal` router serves Traefik's API without authentication on the `saltbox` network, so any container there can read the proxy token. accit still accepts only Traefik's pinned address on the `accit` network (ADR 0002).

## Remove

`sb install mod-accit-remove` refuses while Traefik still references `accit@file` or joins the `accit` network, and prints the inventory lines to delete first (then `sb install traefik`). It then removes `/opt/traefik/accit.yml`, the container and the network. `/opt/accit` stays unless `accit_remove_purge: true`.

## Verify off the host

```sh
deploy/saltbox/check
```

Runs the filter tests, yamllint, the syntax check, renders the templates against `tests/inventory/` and compares them with `tests/expected/`, then ansible-lint. Needs `python3` and `uv`; the first run downloads Ansible, so it stays out of `scripts/ci`.
