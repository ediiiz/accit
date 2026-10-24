"""Filters of the accit role. Pure functions; tests in deploy/saltbox/tests/test_filters.py."""

import ipaddress
import re


def overlapping(subnet, others):
    """The entries of `others` that share an address with `subnet`."""
    own = ipaddress.ip_network(subnet)
    return [
        other
        for other in others
        if ipaddress.ip_network(other).version == own.version and ipaddress.ip_network(other).overlaps(own)
    ]


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


def upgrade_plan(deployed, desired, copies):
    """What happens to the database when the image moves from tag `deployed` ("" before the first
    install) to `desired`. `copies` are the tags whose database copy exists."""
    plan = {"action": "", "save": "", "restore": "", "error": ""}
    if _release(desired) is None:
        plan["error"] = f"accit_role_docker_image_tag must be an exact release such as v1.2.3, not {desired}"
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


def _hosts(rule):
    """The exact hosts a router rule names, canonical; ["*"] when it names none (it matches any host)."""
    hosts = [
        host.lower().rstrip(".")
        for arguments in re.findall(r"\bHost\(([^)]*)\)", rule or "")
        for host in re.findall(r"`([^`]*)`", arguments)
    ]
    return hosts or ["*"]


def _state(host, entrypoint, policy):
    """The rule of `accit admin check`: only `websecure` carries the chain; the Portal passes at step 4."""
    if entrypoint != "websecure":
        return "Uncovered"
    if host == policy["portal"]:
        return "Open"
    if policy["mode"] == "selected":
        return "Gated" if host in policy.get("hosts", []) else "Open"
    return "Open" if host in policy.get("excluded_hosts", []) else "Gated"


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


class FilterModule:
    def filters(self):
        return {
            "accit_chain_error": chain_error,
            "accit_coverage": coverage,
            "accit_overlapping": overlapping,
            "accit_traefik_missing": traefik_missing,
            "accit_traefik_version_error": traefik_version_error,
            "accit_upgrade_plan": upgrade_plan,
        }
