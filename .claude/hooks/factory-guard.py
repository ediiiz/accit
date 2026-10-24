#!/usr/bin/env python3
"""PreToolUse guard for factory sessions (loaded by .claude/factory/settings.json).

Permission rules match command text only, so the hard limits live here. Exit 2 blocks the call."""

import json
import re
import shlex
import sys

PROTECTED = {"main", "master", "staging"}


def words(command):
    try:
        return shlex.split(command)
    except ValueError:
        return command.split()


def push_targets(args):
    """Branch names a `git push` writes to (refspec right side, or the bare name)."""
    refspecs = [a for a in args if not a.startswith("-")][2:]  # drop "push" and the remote
    return [r.split(":")[-1].lstrip("+").removeprefix("refs/heads/") for r in refspecs]


def check_push(args):
    targets = push_targets(args)
    if any(t in PROTECTED for t in targets) or "--all" in args or "--mirror" in args:
        return "Push to a protected branch: the factory lands work only through `scripts/factory/factory merge`."
    forced = "--force" in args or "-f" in args or any(a.startswith("+") for a in args)
    if forced:
        return "Plain force push blocked: use `git push --force-with-lease` on a factory/* branch."
    if any(a.startswith("--force-with-lease") for a in args) and not all(t.startswith("factory/") for t in targets):
        return "--force-with-lease is allowed only on factory/* branches; name the branch explicitly."
    return None


def check(command):
    if re.search(r"--no-verify\b|core\.hooksPath|\bHUSKY=0\b", command):
        return "Hooks stay on: fix what the hook reports instead of skipping it."
    if re.search(r"\bfj\s+pr\s+merge\b|/pulls/\d+/merge\b", command):
        return "Merging goes through `scripts/factory/factory merge <pr>`, which checks CI first."
    if re.search(r"\brm\s+-[a-zA-Z]*r[a-zA-Z]*f?\s+(/|~|\$HOME)(\s|$)", command):
        return "Refusing to delete a root or home directory."
    for segment in re.split(r"&&|\|\||;|\|", command):
        args = words(segment.strip())
        if args[:1] == ["git"]:
            rest = [a for a in args[1:] if a != "-C"]
            if "push" in rest:
                reason = check_push(rest[rest.index("push"):])
                if reason:
                    return reason
            if "commit" in rest and any(re.fullmatch(r"-[a-zA-Z]*n[a-zA-Z]*", a) for a in rest):
                return "`git commit -n` skips hooks: commit without it."
    return None


def main():
    event = json.load(sys.stdin)
    if event.get("tool_name") != "Bash":
        return
    reason = check(event.get("tool_input", {}).get("command", ""))
    if reason:
        print(reason, file=sys.stderr)
        sys.exit(2)


if __name__ == "__main__":
    main()
