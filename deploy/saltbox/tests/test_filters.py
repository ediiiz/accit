"""The accit role's filter plugin, at its public functions.

Expected values come from issue #16 (decisions 5, 8, 15, 17, 18, 22) and docs/spec.md sections 3, 4 and 10.
Run: python3 -m unittest discover -s deploy/saltbox/tests
"""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "roles" / "accit" / "filter_plugins"))

import accit  # noqa: E402


class Overlaps(unittest.TestCase):
    def test_disjoint_subnets_do_not_overlap(self):
        self.assertEqual(accit.overlapping("172.31.250.0/29", ["172.19.0.0/16", "fd00:dead:beef::/48"]), [])

    def test_a_containing_subnet_overlaps(self):
        self.assertEqual(accit.overlapping("172.31.250.0/29", ["172.16.0.0/12", "10.0.0.0/8"]), ["172.16.0.0/12"])

    def test_a_contained_subnet_overlaps(self):
        self.assertEqual(accit.overlapping("172.31.250.0/29", ["172.31.250.4/30"]), ["172.31.250.4/30"])


WIRED_COMMAND = [
    "--providers.file.directory=/etc/traefik",
    "--entrypoints.websecure.address=:443",
    "--entrypoints.websecure.http.middlewares=accit@file",
    "--entrypoints.websecure.http.aliasHeadersStrategy=delete",
    "--core.strictTLSOptions=true",
]


def traefik(command, networks):
    return {"Config": {"Cmd": command}, "NetworkSettings": {"Networks": networks}}


def on_accit(address):
    return {"accit": {"IPAMConfig": {"IPv4Address": address}, "IPAddress": address}}


class TraefikWiring(unittest.TestCase):
    def missing(self, container):
        return accit.traefik_missing(container, "accit", "172.31.250.2", "delete")

    def test_a_wired_traefik_misses_nothing(self):
        self.assertEqual(self.missing(traefik(WIRED_COMMAND, on_accit("172.31.250.2"))), [])

    def test_flags_are_case_insensitive_and_the_chain_may_have_neighbours(self):
        command = [
            "--entryPoints.websecure.http.middlewares=other@file,accit@file",
            "--entrypoints.websecure.http.aliasheadersstrategy=delete",
            "--core.stricttlsoptions",
        ]
        self.assertEqual(self.missing(traefik(command, on_accit("172.31.250.2"))), [])

    def test_a_stock_saltbox_traefik_misses_everything(self):
        stock = traefik(["--entrypoints.websecure.address=:443"], {"saltbox": {"IPAddress": "172.19.0.5"}})
        self.assertEqual(
            self.missing(stock),
            [
                "--entrypoints.websecure.http.middlewares with accit@file",
                "--entrypoints.websecure.http.aliasHeadersStrategy=delete",
                "--core.strictTLSOptions=true",
                "accit network address 172.31.250.2",
            ],
        )

    def test_a_dynamic_address_on_the_accit_network_is_not_pinned(self):
        dynamic = {"accit": {"IPAMConfig": None, "IPAddress": "172.31.250.2"}}
        self.assertEqual(self.missing(traefik(WIRED_COMMAND, dynamic)), ["accit network address 172.31.250.2"])

    def test_strict_tls_options_set_false_is_missing(self):
        command = [flag for flag in WIRED_COMMAND if "strict" not in flag] + ["--core.strictTLSOptions=false"]
        self.assertEqual(self.missing(traefik(command, on_accit("172.31.250.2"))), ["--core.strictTLSOptions=true"])


def version_output(version):
    return f"Version:      {version}\nCodename:     ramequin\nGo version:   go1.25.1\nOS/Arch:      linux/amd64\n"


class TraefikVersion(unittest.TestCase):
    def error(self, output):
        return accit.traefik_version_error(output, "3.7.16")

    def test_the_floor_itself_passes(self):
        self.assertEqual(self.error(version_output("3.7.16")), "")

    def test_versions_compare_numerically(self):
        self.assertEqual(self.error(version_output("3.10.0")), "")
        self.assertEqual(self.error(version_output("3.7.9")), "Traefik 3.7.9 is older than 3.7.16")

    def test_unreadable_output_is_an_error(self):
        self.assertEqual(self.error("traefik: command not found"), "Traefik version not found in `traefik version`")


class UpgradePlan(unittest.TestCase):
    def plan(self, deployed, desired, copies=()):
        return accit.upgrade_plan(deployed, desired, list(copies))

    def test_first_install_copies_nothing(self):
        self.assertEqual(self.plan("", "v1.0.0"), {"action": "install", "save": "", "restore": "", "error": ""})

    def test_the_same_tag_keeps_the_database(self):
        self.assertEqual(self.plan("v1.0.0", "v1.0.0")["action"], "keep")

    def test_an_upgrade_copies_the_database_of_the_old_tag(self):
        self.assertEqual(
            self.plan("v1.2.0", "v1.10.0"), {"action": "upgrade", "save": "v1.2.0", "restore": "", "error": ""}
        )

    def test_a_rollback_restores_the_copy_of_the_older_tag(self):
        self.assertEqual(
            self.plan("v1.10.0", "v1.2.0", ["v1.2.0"]),
            {"action": "rollback", "save": "v1.10.0", "restore": "v1.2.0", "error": ""},
        )

    def test_a_downgrade_without_a_copy_is_refused(self):
        self.assertEqual(
            self.plan("v1.10.0", "v1.2.0", ["v1.0.0"])["error"],
            "no database copy from v1.2.0: refusing the downgrade from v1.10.0",
        )

    def test_a_floating_tag_is_refused(self):
        self.assertEqual(
            self.plan("", "latest")["error"],
            "accit_role_docker_image_tag must be an exact release such as v1.2.3, not latest",
        )


def router(name, rule, entrypoints):
    return {"name": name, "rule": rule, "entryPoints": entrypoints, "status": "enabled"}


SELECTED = {"mode": "selected", "hosts": ["jellyfin.example.com"], "portal": "access.example.com"}
ALL = {"mode": "all", "excluded_hosts": ["status.example.com"], "portal": "access.example.com"}


class Coverage(unittest.TestCase):
    def states(self, routers, policy):
        return [(row["host"], row["entrypoint"], row["state"]) for row in accit.coverage(routers, policy)]

    def test_selected_mode_gates_listed_hosts_on_websecure_only(self):
        routers = [
            router("jellyfin@docker", "Host(`Jellyfin.Example.com.`)", ["websecure"]),
            router("jellyfin-http@docker", "Host(`jellyfin.example.com`)", ["web"]),
            router("sonarr@docker", "Host(`sonarr.example.com`)", ["websecure"]),
        ]
        self.assertEqual(
            self.states(routers, SELECTED),
            [
                ("jellyfin.example.com", "web", "Uncovered"),
                ("jellyfin.example.com", "websecure", "Gated"),
                ("sonarr.example.com", "websecure", "Open"),
            ],
        )

    def test_all_mode_opens_only_exclusions_and_the_portal(self):
        routers = [
            router("status@docker", "Host(`status.example.com`)", ["websecure"]),
            router("sonarr@docker", "Host(`sonarr.example.com`) && PathPrefix(`/api`)", ["websecure"]),
            router("accit-portal@file", "Host(`access.example.com`)", ["websecure"]),
        ]
        self.assertEqual(
            self.states(routers, ALL),
            [
                ("access.example.com", "websecure", "Open"),
                ("sonarr.example.com", "websecure", "Gated"),
                ("status.example.com", "websecure", "Open"),
            ],
        )

    def test_tailscale_and_internal_entrypoints_are_uncovered(self):
        routers = [
            router("traefik-internal@docker", "Host(`traefik`)", ["internal"]),
            router("jellyfin-ts@docker", "Host(`jellyfin.example.com`)", ["tailscale-websecure"]),
        ]
        self.assertEqual(
            self.states(routers, SELECTED),
            [
                ("jellyfin.example.com", "tailscale-websecure", "Uncovered"),
                ("traefik", "internal", "Uncovered"),
            ],
        )

    def test_a_rule_without_an_exact_host_matches_any_host(self):
        routers = [router("robots@docker", "Path(`/robots.txt`)", ["web", "websecure"])]
        self.assertEqual(self.states(routers, SELECTED), [("*", "web", "Uncovered"), ("*", "websecure", "Open")])
        self.assertEqual(self.states(routers, ALL), [("*", "web", "Uncovered"), ("*", "websecure", "Gated")])

    def test_a_router_without_entrypoints_listens_on_all_of_them(self):
        routers = [router("whoami@docker", "Host(`whoami.example.com`)", [])]
        self.assertEqual(self.states(routers, ALL), [("whoami.example.com", "(all)", "Uncovered")])

    def test_each_host_of_a_combined_rule_is_a_row(self):
        routers = [router("a@docker", "Host(`a.example.com`) || Host(`jellyfin.example.com`)", ["websecure"])]
        self.assertEqual(
            self.states(routers, SELECTED),
            [("a.example.com", "websecure", "Open"), ("jellyfin.example.com", "websecure", "Gated")],
        )


def chain(members, status="enabled"):
    return {"name": "accit@file", "status": status, "chain": {"middlewares": members}}


def delete(headers, status="enabled"):
    return {"name": "accit-delete-token@file", "status": status, "headers": {"customRequestHeaders": headers}}


MEMBERS = ["accit-set-token", "accit-verify", "accit-delete-token"]
BLANK = {"X-Accit-Proxy-Token": ""}


class LoadedChain(unittest.TestCase):
    def test_the_rendered_chain_passes(self):
        self.assertEqual(accit.chain_error(chain(MEMBERS), delete(BLANK)), "")

    def test_provider_qualified_members_pass(self):
        self.assertEqual(accit.chain_error(chain([f"{name}@file" for name in MEMBERS]), delete(BLANK)), "")

    def test_a_chain_that_does_not_end_by_deleting_the_token_fails(self):
        self.assertEqual(
            accit.chain_error(chain(MEMBERS[:2]), delete(BLANK)),
            "accit@file is not set token, verify, delete token",
        )

    def test_a_delete_step_that_keeps_the_header_fails(self):
        self.assertEqual(
            accit.chain_error(chain(MEMBERS), delete({})),
            "accit-delete-token@file does not remove X-Accit-Proxy-Token",
        )

    def test_a_disabled_middleware_fails(self):
        self.assertEqual(
            accit.chain_error(chain(MEMBERS, status="disabled"), delete(BLANK)), "accit@file is disabled in Traefik"
        )


if __name__ == "__main__":
    unittest.main()
