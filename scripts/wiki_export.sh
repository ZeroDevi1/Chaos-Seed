#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  scripts/wiki_export.sh <out_dir>

Exports a curated set of Markdown files from this repo to a directory that can be
mirrored into GitHub Wiki (*.wiki.git).
EOF
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

out_dir="${1:-}"
if [[ -z "${out_dir}" ]]; then
  usage >&2
  exit 2
fi

if [[ "${out_dir}" == "/" || "${out_dir}" == "." || "${out_dir}" == ".." ]]; then
  echo "error: unsafe out_dir: ${out_dir}" >&2
  exit 2
fi

repo_root="$(git rev-parse --show-toplevel 2>/dev/null || true)"
if [[ -z "${repo_root}" ]]; then
  echo "error: must be run inside a git repository" >&2
  exit 2
fi

cd "${repo_root}"

rm -rf "${out_dir}"
mkdir -p "${out_dir}"

copy() {
  local src="$1"
  local dst="$2"
  if [[ ! -f "${src}" ]]; then
    echo "error: missing source markdown: ${src}" >&2
    exit 1
  fi
  cp -f "${src}" "${out_dir}/${dst}"
}

# Home
copy "README.md" "Home.md"

# Docs
copy "docs/BUILD_WINUI3.md" "BUILD_WINUI3.md"
copy "docs/BUILD_CHAOS_FLUTTER.md" "BUILD_CHAOS_FLUTTER.md"
copy "docs/WINUI3_AUTO_UPDATE.md" "WINUI3_AUTO_UPDATE.md"
copy "docs/LEGAL_MPV.md" "LEGAL_MPV.md"
copy "docs/WIKI_SYNC.md" "WIKI_SYNC.md"

# FFI docs (renamed for clearer Wiki page names)
copy "chaos-ffi/docs/API.md" "FFI_API.md"
copy "chaos-ffi/docs/CSharp.md" "FFI_CSharp.md"
copy "chaos-ffi/docs/BUILD.md" "FFI_BUILD.md"

# Daemon docs
copy "chaos-daemon/docs/API.md" "Daemon_API.md"
copy "chaos-daemon/docs/CSharp.md" "Daemon_CSharp.md"

# Project logs / roadmaps
copy "TODO.md" "TODO.md"
copy "TODO_NEXT.md" "TODO_NEXT.md"
copy "DEVLOG.md" "DEVLOG.md"

cat >"${out_dir}/_Sidebar.md" <<'EOF'
- [[Home]]
- [[BUILD_WINUI3]]
- [[BUILD_CHAOS_FLUTTER]]
- [[WINUI3_AUTO_UPDATE]]
- [[LEGAL_MPV]]
- [[FFI_API]]
- [[FFI_CSharp]]
- [[FFI_BUILD]]
- [[Daemon_API]]
- [[Daemon_CSharp]]
- [[WIKI_SYNC]]
- [[TODO]]
- [[TODO_NEXT]]
- [[DEVLOG]]
EOF

sha="$(git rev-parse --short HEAD 2>/dev/null || echo unknown)"
cat >"${out_dir}/_Footer.md" <<EOF
Synced from \`${sha}\`.
EOF

echo "exported wiki markdown to: ${out_dir}"
