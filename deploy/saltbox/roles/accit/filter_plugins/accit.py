"""Filters of the accit role. Pure functions; tests in deploy/saltbox/tests/test_filters.py."""

import ipaddress
import json
import re


def overlapping(subnet, others):
    """The entries of `others` that share an address with `subnet`."""
    own = ipaddress.ip_network(subnet)
    return [
        other
        for other in others
        if ipaddress.ip_network(other).version == own.version and ipaddress.ip_network(other).overlaps(own)
    ]


def pinned_address_error(address, subnet):
    """Why `address` cannot be pinned on a Docker network with `subnet`, or ""."""
    network = ipaddress.ip_network(subnet)
    hosts = list(network.hosts())
    if ipaddress.ip_address(address) not in hosts:
        return f"{address} is not a host address in {subnet}"
    if ipaddress.ip_address(address) == hosts[0]:
        return f"{address} is Docker's gateway in {subnet}"
    return ""


def _flags(container):
    """Traefik's command-line flags as {lowercase name: value}; a bare boolean flag is "true"."""
    args = (container.get("Config") or {}).get("Cmd") or []
    flags = {}
    for arg in args:
        name, _, value = arg.partition("=")
        flags[name.lower()] = value if "=" in arg else "true"
    return flags


def traefik_missing(container, network, address, alias_strategy):
    """What the running Traefik container (`docker inspect` object) lacks for accit, as the setting to add."""
    flags = _flags(container)
    middlewares = flags.get("--entrypoints.websecure.http.middlewares", "").split(",")
    attached = ((container.get("NetworkSettings") or {}).get("Networks") or {}).get(network) or {}
    pinned = (attached.get("IPAMConfig") or {}).get("IPv4Address")

    checks = [
        ("accit@file" in middlewares, "--entrypoints.websecure.http.middlewares with accit@file"),
        (
            flags.get("--entrypoints.websecure.http.aliasheadersstrategy", "").lower() == alias_strategy,
            f"--entrypoints.websecure.http.aliasHeadersStrategy={alias_strategy}",
        ),
        (flags.get("--core.stricttlsoptions", "").lower() == "true", "--core.strictTLSOptions=true"),
        (pinned == address and attached.get("IPAddress") == address, f"{network} network address {address}"),
    ]
    return [setting for held, setting in checks if not held]


def traefik_websecure(container):
    """Where the host reaches Traefik's websecure entrypoint: its published IPv4 binding, loopback when it
    binds every address. With userland-proxy off, Docker forwards only the bound address."""
    port = _flags(container).get("--entrypoints.websecure.address", ":443").rpartition(":")[2]
    bindings = ((container.get("HostConfig") or {}).get("PortBindings") or {}).get(f"{port}/tcp") or []
    ipv4 = [binding for binding in bindings if ":" not in (binding.get("HostIp") or "")]
    if not ipv4:
        return {"address": "", "port": "", "error": f"Traefik publishes no IPv4 host port for websecure ({port}/tcp)"}
    host = ipv4[0].get("HostIp") or ""
    return {
        "address": "127.0.0.1" if host in ("", "0.0.0.0") else host,
        "port": ipv4[0]["HostPort"],
        "error": "",
    }


def _release(text):
    """(major, minor, patch) of `X.Y.Z` or `vX.Y.Z`, or None."""
    match = re.fullmatch(r"v?(\d+)\.(\d+)\.(\d+)", text.strip())
    return tuple(int(part) for part in match.groups()) if match else None


def traefik_version_error(output, minimum):
    """Why the output of `traefik version` fails the floor, or "" when it passes."""
    match = re.search(r"^Version:\s*(\S+)", output, re.MULTILINE)
    version = _release(match.group(1).split("-")[0]) if match else None
    if version is None:
        return "Traefik version not found in `traefik version`"
    if version < _release(minimum):
        return f"Traefik {match.group(1)} is older than {minimum}"
    return ""


def upgrade_plan(deployed, desired, copies, database):
    """What happens to the database when the image moves from tag `deployed` ("" when none is recorded)
    to `desired`. `copies` are the tags whose database copy exists; `database`: accit.db exists."""
    plan = {"action": "", "save": "", "restore": "", "error": ""}
    if _release(desired) is None:
        plan["error"] = f"accit_role_docker_image_tag must be an exact release such as v1.2.3, not {desired}"
    elif not deployed and database:
        plan.update(action="upgrade", save="untagged")
    elif not deployed:
        plan["action"] = "install"
    elif deployed == desired:
        plan["action"] = "keep"
    elif _release(deployed) is not None and _release(desired) < _release(deployed):
        if desired in copies:
            plan.update(action="rollback", save=deployed, restore=desired)
        else:
            plan["error"] = f"no database copy from {desired}: refusing the downgrade from {deployed}"
    else:
        plan.update(action="upgrade", save=deployed)
    return plan


def config_change(live, new, pending=""):
    """What applies `new` over the running `live` accit.yaml (None when absent): "", "reload" or "restart".
    `accit admin reload` swaps only `protection` and accepts other changes without applying them.
    `pending` is the action a failed earlier run wrote but never applied."""
    if live is None or {**live, "protection": None} != {**new, "protection": None}:
        return "restart"
    change = "reload" if live.get("protection") != new.get("protection") else ""
    return max(change, pending.strip(), key=["", "reload", "restart"].index)


def _hosts(rule):
    """The exact hosts a router rule names, canonical; ["*"] when it names none (it matches any host)."""
    hosts = [
        _canonical(host)
        for arguments in re.findall(r"\bHost\(([^)]*)\)", rule or "")
        for host in re.findall(r"`([^`]*)`", arguments)
    ]
    return hosts or ["*"]


def _canonical(host):
    return host.lower().rstrip(".")


def _state(host, entrypoint, policy):
    """The rule of `accit admin check`: only `websecure` carries the chain; the Portal passes at step 4."""
    if entrypoint != "websecure":
        return "Uncovered"
    if host == policy["portal"]:
        return "Open"
    if policy["mode"] == "selected":
        return "Gated" if host in map(_canonical, policy.get("hosts", [])) else "Open"
    return "Open" if host in map(_canonical, policy.get("excluded_hosts", [])) else "Gated"


def coverage(routers, policy):
    """Traefik's routers (`/api/http/routers`) as rows of host, entrypoint, router and Gated/Open/Uncovered."""
    rows = [
        {"host": host, "entrypoint": entrypoint, "state": _state(host, entrypoint, policy), "router": router["name"]}
        for router in routers
        for host in _hosts(router.get("rule"))
        for entrypoint in router.get("entryPoints") or ["(all)"]
    ]
    return sorted(rows, key=lambda row: (row["host"], row["entrypoint"], row["router"]))


def chain_error(chain, delete):
    """Why the `accit@file` chain as Traefik loaded it (`/api/http/middlewares/<name>`) could let the token
    reach an app, or "". Never given the set-token middleware: its configuration holds the token."""
    members = [name.split("@")[0] for name in (chain.get("chain") or {}).get("middlewares", [])]
    headers = (delete.get("headers") or {}).get("customRequestHeaders") or {}
    for middleware in (chain, delete):
        if middleware.get("status") != "enabled":
            return f"{middleware.get('name')} is {middleware.get('status')} in Traefik"
    if members != ["accit-set-token", "accit-verify", "accit-delete-token"]:
        return "accit@file is not set token, verify, delete token"
    if headers.get("X-Accit-Proxy-Token") != "":
        return "accit-delete-token@file does not remove X-Accit-Proxy-Token"
    return ""


def _denied(probe):
    """accit's 403: status 403 with the body `Forbidden`."""
    lines = probe["stdout"].splitlines()
    return lines[-1:] == ["403"] and lines[:1] == ["Forbidden"]


def public_probe(status_probe, gated_probe):
    """Outcome of the Portal's /api/status and a Gated host, both requested through the host's public
    address (registered curl results, status code on the last line). The Gated host is judged only when the
    Portal reports a public Address that neither a Trusted network nor a Grant lets through (spec section 3)."""
    if status_probe["rc"] != 0:
        return {"outcome": "skipped", "reason": f"the Portal did not answer (curl exit {status_probe['rc']})"}
    *body, code = status_probe["stdout"].splitlines()
    if code == "403":
        return {"outcome": "skipped", "reason": "accit sees no public Address on this path"}
    if code != "200":
        return {"outcome": "skipped", "reason": f"the Portal answered {code}"}
    status = json.loads("\n".join(body))
    address = status["address"]
    if status["state"] == "trusted":
        return {"outcome": "skipped", "reason": f"{address} is in a Trusted network"}
    if status["state"] == "active":
        return {"outcome": "skipped", "reason": f"{address} holds a Grant"}
    if gated_probe["rc"] != 0:
        return {"outcome": "failed", "reason": f"the Gated host did not answer (curl exit {gated_probe['rc']})"}
    if not _denied(gated_probe):
        code = gated_probe["stdout"].splitlines()[-1]
        return {"outcome": "failed", "reason": f"the Gated host answered {code} to {address} without a Grant"}
    return {"outcome": "passed", "reason": f"{address} without a Grant"}


class FilterModule:
    def filters(self):
        return {
            "accit_chain_error": chain_error,
            "accit_config_change": config_change,
            "accit_coverage": coverage,
            "accit_overlapping": overlapping,
            "accit_pinned_address_error": pinned_address_error,
            "accit_public_probe": public_probe,
            "accit_traefik_missing": traefik_missing,
            "accit_traefik_version_error": traefik_version_error,
            "accit_traefik_websecure": traefik_websecure,
            "accit_upgrade_plan": upgrade_plan,
        }
