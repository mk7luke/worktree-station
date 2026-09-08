#!/usr/bin/env bash
# Build a set of throwaway repositories with worktrees in varied states, for
# screenshots and demos. Everything it creates lives under one directory and
# can be deleted with `--clean`.
#
#   scripts/make-demo.sh                 # create under ~/worktree-station-demo
#   scripts/make-demo.sh --seed-app      # ...and point the app at them
#   scripts/make-demo.sh --clean         # remove everything it created
#
set -euo pipefail

ROOT="${HOME}/worktree-station-demo"
SEED_APP=0
CLEAN=0
APP_CONFIG="${HOME}/Library/Application Support/io.github.mk7luke.worktreestation"

while [ $# -gt 0 ]; do
  case "$1" in
    --dir) ROOT="$2"; shift 2 ;;
    --seed-app) SEED_APP=1; shift ;;
    --clean) CLEAN=1; shift ;;
    -h|--help) sed -n '2,10p' "$0"; exit 0 ;;
    *) echo "unknown option: $1" >&2; exit 1 ;;
  esac
done

if [ "$CLEAN" = 1 ]; then
  # Detach worktrees first so git does not leave administrative files behind.
  for repo in "$ROOT"/repos/*; do
    [ -d "$repo/.git" ] || continue
    git -C "$repo" worktree list --porcelain 2>/dev/null | awk '/^worktree /{print $2}' | tail -n +2 |
      while read -r wt; do git -C "$repo" worktree remove --force "$wt" 2>/dev/null || true; done
  done
  rm -rf "$ROOT"
  echo "removed $ROOT"
  exit 0
fi

if [ -e "$ROOT" ]; then
  echo "refusing to overwrite $ROOT — run with --clean first" >&2
  exit 1
fi

mkdir -p "$ROOT/repos" "$ROOT/worktrees"

# commit <repo> <author> <email> <file> <content> <message>
commit() {
  local dir="$1" author="$2" email="$3" file="$4" content="$5" msg="$6"
  printf '%s\n' "$content" > "$dir/$file"
  git -C "$dir" add -A
  git -C "$dir" -c user.name="$author" -c user.email="$email" commit -qm "$msg"
}

# new_repo <name> <default-branch> <ignore-lines...>
new_repo() {
  local name="$1" branch="$2"; shift 2
  local dir="$ROOT/repos/$name"
  mkdir -p "$dir"
  git -C "$dir" init -q -b "$branch"
  git -C "$dir" config user.name "Ada Okonkwo"
  git -C "$dir" config user.email "ada@example.com"
  printf '%s\n' "$@" > "$dir/.gitignore"
  echo "$name" > "$dir/README.md"
  git -C "$dir" add -A
  git -C "$dir" -c user.name="Ada Okonkwo" -c user.email="ada@example.com" \
    commit -qm "Initial commit"
  echo "$dir"
}

# add_worktree <repo-dir> <branch>
add_worktree() {
  local repo="$1" branch="$2"
  local slug repo_name path
  repo_name="$(basename "$repo")"
  slug="$(printf '%s' "$branch" | tr -c 'a-zA-Z0-9._' '-' | sed 's/--*/-/g; s/^-//; s/-$//')"
  path="$ROOT/worktrees/${repo_name}-${slug}"
  git -C "$repo" branch "$branch" 2>/dev/null || true
  git -C "$repo" worktree add -q "$path" "$branch"
  git -C "$path" config user.name "Claude"
  git -C "$path" config user.email "claude@example.com"
  echo "$path"
}

echo "Creating demo repositories under $ROOT"

# ---------------------------------------------------------------- vben-admin
R=$(new_repo vben-admin main "node_modules/" ".env.local" "dist/")
mkdir -p "$R/node_modules/.bin" && echo '{}' > "$R/node_modules/.bin/vite.json"
echo 'API_TOKEN=dev' > "$R/.env.local"
commit "$R" "Ada Okonkwo" ada@example.com app.ts 'export const version = "5.6.0";' \
  "Merge pull request #812 from vben/release-5.6"

W=$(add_worktree "$R" "feature/login")
commit "$W" Claude claude@example.com auth-guard.ts 'export const guard = true;' \
  "Move session refresh behind the auth guard"
echo 'draft' > "$W/scratch.ts"; echo 'export const guard = false;' > "$W/auth-guard.ts"

W=$(add_worktree "$R" "fix/token-refresh")
commit "$W" Claude claude@example.com refresh.ts 'retry once' \
  "Retry once when the refresh token has expired"
echo 'wip' >> "$W/refresh.ts"

W=$(add_worktree "$R" "chore/bump-deps")
commit "$W" Claude claude@example.com deps.txt 'vite 7.2' \
  "Bump vite to 7.2 and drop the postcss shim"

# -------------------------------------------------------------- api-gateway
R=$(new_repo api-gateway main "target/" ".env")
mkdir -p "$R/target/debug" && echo 'bin' > "$R/target/debug/gateway"
echo 'DATABASE_URL=postgres://localhost/dev' > "$R/.env"
commit "$R" "Ada Okonkwo" ada@example.com main.rs 'fn main() {}' \
  "Split rate limiting out of the request middleware"

W=$(add_worktree "$R" "perf/connection-pool")
commit "$W" Claude claude@example.com pool.rs 'pool tuning' \
  "Reuse pooled connections across upstream retries"
echo 'more' >> "$W/pool.rs"; echo 'bench' > "$W/bench.rs"

W=$(add_worktree "$R" "feat/request-tracing")
commit "$W" Claude claude@example.com trace.rs 'tracing' \
  "Emit a span per upstream call"

# ----------------------------------------------------------- marketing-site
R=$(new_repo marketing-site main "node_modules/" ".next/")
mkdir -p "$R/node_modules/next" && echo '{}' > "$R/node_modules/next/package.json"
commit "$R" "Ada Okonkwo" ada@example.com index.tsx 'export default function Home() {}' \
  "Rewrite the pricing page above the fold"

W=$(add_worktree "$R" "feat/pricing-table")
commit "$W" Claude claude@example.com pricing.tsx 'table' \
  "Add the annual/monthly toggle to the pricing table"
echo 'tweak' > "$W/pricing.css"

# ----------------------------------------------------------- design-system
R=$(new_repo design-system main "node_modules/" "storybook-static/")
mkdir -p "$R/node_modules/.cache" && echo '{}' > "$R/node_modules/.cache/x.json"
commit "$R" "Ada Okonkwo" ada@example.com tokens.css ':root { --space: 4px; }' \
  "Promote the spacing scale to design tokens"

W=$(add_worktree "$R" "feat/dark-tokens")
commit "$W" Claude claude@example.com dark.css '[data-theme=dark] {}' \
  "Add the dark palette to the token set"

# ----------------------------------------------------------- data-pipeline
R=$(new_repo data-pipeline main ".venv/" "__pycache__/" ".env")
mkdir -p "$R/.venv/bin" && echo 'python' > "$R/.venv/bin/python"
echo 'WAREHOUSE=snowflake' > "$R/.env"
commit "$R" "Ada Okonkwo" ada@example.com etl.py 'def run(): pass' \
  "Backfill the sessions table from raw events"

W=$(add_worktree "$R" "fix/late-arriving-rows")
commit "$W" Claude claude@example.com dedupe.py 'dedupe' \
  "Deduplicate late-arriving rows on the natural key"
echo '# wip' >> "$W/dedupe.py"

# -------------------------------------------------------------- mobile-app
R=$(new_repo mobile-app main "node_modules/" "ios/Pods/" ".env")
mkdir -p "$R/node_modules/.bin" "$R/ios/Pods"
echo '{}' > "$R/node_modules/.bin/rn.json"; echo 'pod' > "$R/ios/Pods/Manifest.lock"
echo 'SENTRY_DSN=dev' > "$R/.env"
commit "$R" "Ada Okonkwo" ada@example.com App.tsx 'export default App;' \
  "Move onboarding behind a feature flag"

W=$(add_worktree "$R" "feat/offline-cache")
commit "$W" Claude claude@example.com cache.ts 'cache' \
  "Cache the feed for offline reads"
echo 'wip' > "$W/cache.test.ts"

if [ "$SEED_APP" = 1 ]; then
  mkdir -p "$APP_CONFIG"
  python3 - "$APP_CONFIG/settings.json" "$ROOT" <<'PY'
import json, pathlib, sys
cfg, root = pathlib.Path(sys.argv[1]), sys.argv[2]
existing = {}
if cfg.exists():
    try: existing = json.loads(cfg.read_text())
    except Exception: existing = {}
repos = sorted(str(p) for p in pathlib.Path(root, "repos").iterdir() if p.is_dir())
existing.update({
    "repos": repos,
    "activeRepo": next((r for r in repos if r.endswith("vben-admin")), repos[0]),
    "worktreeRoot": f"{root}/worktrees",
    "repoRoots": {},
})
existing.setdefault("theme", "system")
cfg.write_text(json.dumps(existing, indent=2))
print(f"pointed the app at {len(repos)} repositories")
PY
fi

echo
echo "Done."
echo "  repositories: $ROOT/repos"
echo "  worktrees:    $ROOT/worktrees"
echo "  remove with:  scripts/make-demo.sh --clean"
