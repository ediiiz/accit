"""Persistent build slots, shared state and disk control.

A slot is a reused git worktree (and its node_modules). Every release runs `cargo clean` in it,
so no build output outlives the agent that made it; disk is bounded by the agents running at once."""

import contextlib
import fcntl
import json
import shutil
import subprocess
import time
from pathlib import Path

GB = 1024 ** 3


def sh(*args, cwd=None, check=True):
    res = subprocess.run(args, cwd=cwd, capture_output=True, text=True)
    if check and res.returncode != 0:
        raise SystemExit(f"{' '.join(args)}: {res.stderr.strip()[:500]}")
    return res.stdout.strip()


def main_root():
    common = Path(sh("git", "rev-parse", "--path-format=absolute", "--git-common-dir"))
    return common.parent


class Store:
    """JSON state under <main checkout>/.factory, guarded by a file lock."""

    def __init__(self, cfg):
        self.cfg = cfg
        self.root = main_root()
        self.dir = self.root / ".factory"
        self.dir.mkdir(exist_ok=True)
        (self.dir / "reviews").mkdir(exist_ok=True)
        (self.dir / "logs").mkdir(exist_ok=True)
        self.file = self.dir / "state.json"

    @contextlib.contextmanager
    def locked(self):
        with open(self.dir / "state.lock", "w") as lock:
            fcntl.flock(lock, fcntl.LOCK_EX)
            state = json.loads(self.file.read_text()) if self.file.exists() else {}
            state.setdefault("slots", {})
            state.setdefault("attempts", {})
            state.setdefault("runs", {})
            yield state
            self.file.write_text(json.dumps(state, indent=2))

    def slot_ids(self, kind):
        return [f"{kind}-{i}" for i in range(1, self.cfg["slots"][kind] + 1)]

    def slot_path(self, slot):
        return self.dir / "slots" / slot

    def env(self, slot):
        return {
            "CARGO": self.cfg["cargo"],
            "CARGO_INCREMENTAL": "0",
            "CARGO_PROFILE_DEV_DEBUG": "line-tables-only",
            "CARGO_PROFILE_TEST_DEBUG": "line-tables-only",
            "CARGO_BUILD_JOBS": str(self.cfg["cargo_build_jobs"]),
            "FACTORY_SLOT": slot,
        }


def prepare(store, slot, branch, new_branch=False):
    """Check out ref (new branch from base, or an existing PR branch) in the slot."""
    path = store.slot_path(slot)
    base = store.cfg["base"]
    sh("git", "fetch", "-q", "origin", base, *([branch] if branch and not new_branch else []))
    if not path.exists():
        path.parent.mkdir(parents=True, exist_ok=True)
        sh("git", "worktree", "add", "-q", "--detach", str(path), f"origin/{base}")
    sh("git", "reset", "-q", "--hard", cwd=path)
    sh("git", "clean", "-qfd", "-e", "target", "-e", "node_modules", cwd=path)
    if new_branch:
        sh("git", "switch", "-q", "-C", branch, f"origin/{base}", cwd=path)
    else:
        sh("git", "switch", "-q", "-C", branch, f"origin/{branch}", cwd=path)
    env_file = store.dir / "slots" / f"{slot}.env"
    env_file.write_text("".join(f"export {k}={v}\n" for k, v in store.env(slot).items()))
    if not (path / "node_modules").exists():
        sh("bun", "install", "--frozen-lockfile", cwd=path)
    return {"slot": slot, "path": str(path), "env": str(env_file), "branch": branch}


def dir_size(path):
    if not path.exists():
        return 0
    out = sh("du", "-sb", str(path), check=False)
    return int(out.split()[0]) if out else 0


def disk(store):
    free = shutil.disk_usage(store.root).free / GB
    level = "red" if free < store.cfg["disk_critical_free_gb"] else \
        "amber" if free < store.cfg["disk_pause_free_gb"] else "green"
    return {"free_gb": round(free, 1), "level": level}


def clean_slot(store, slot, aggressive=False):
    """After every agent: `cargo clean` in the slot, so its target dir is gone."""
    path = store.slot_path(slot)
    size = dir_size(path / "target")
    sh(store.cfg["cargo"], "clean", cwd=path, check=False)
    shutil.rmtree(path / "target", ignore_errors=True)
    return {"slot": slot, "target_gb": round(size / GB, 1), "action": "cargo clean"}


def gc(store, state, aggressive=False):
    report = []
    for slot in store.slot_ids("impl") + store.slot_ids("merge"):
        if slot not in state["slots"] and store.slot_path(slot).exists():
            report.append(clean_slot(store, slot, aggressive))
    sh("git", "worktree", "prune")
    sh("git", "fetch", "-q", "--prune", "origin")
    merged = sh("git", "branch", "--list", "factory/*", "--merged", f"origin/{store.cfg['base']}")
    busy = {s.get("branch") for s in state["slots"].values()}
    for branch in (b.strip(" *+") for b in merged.splitlines()):
        if branch and branch not in busy:
            sh("git", "branch", "-q", "-D", branch, check=False)
    sh("docker", "image", "prune", "-f", check=False)
    return {"slots": report, "disk": disk(store), "at": int(time.time())}
