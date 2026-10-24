#!/usr/bin/env bash
#
# Encrypt every plaintext dotenv file in the repo with SOPS.
#
# We encrypt any `.env*` file that is neither an `.example` template nor an
# already-encrypted `.enc` file, writing the ciphertext to the same path with
# an `.enc` suffix appended (e.g. `apps/web/.env` -> `apps/web/.env.enc`).
# `.example` files hold no secrets and `.enc` files are the output, so both are
# skipped.
#
# Encryption only needs the age *public* key, which sops reads from
# `.sops.yaml`. The `--filename-override` makes sops match that rule even
# though the input file isn't named `*.enc`; the explicit dotenv types are
# required because `--filename-override` also drives type inference and the
# `.enc` extension is otherwise unrecognised.
#
# Usage: bun run encrypt:env   (from the repo root)

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

if ! command -v sops >/dev/null 2>&1; then
	echo "error: sops is not installed or not on PATH" >&2
	exit 1
fi

status=0

# Find every plaintext dotenv file, skipping .example templates, already
# encrypted .enc files, and dependencies.
while IFS= read -r plain; do
	enc="${plain}.enc"

	sops encrypt \
		--filename-override "$enc" \
		--input-type dotenv \
		--output-type dotenv \
		"$plain" >"$enc"

	echo "encrypted: $plain -> $enc"
done < <(find . -name '.env*' \
	-not -name '*.example' \
	-not -name '*.enc' \
	-not -path '*/node_modules/*' | sort)

exit "$status"
