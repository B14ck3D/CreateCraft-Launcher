#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="${ROOT_DIR}/packaging/arch/out"
TEMPLATE="${ROOT_DIR}/packaging/arch/PKGBUILD.in"
PACKAGE_JSON="${ROOT_DIR}/package.json"

mkdir -p "${OUT_DIR}"

VERSION="$(node -p "require('${PACKAGE_JSON}').version")"
APPIMAGE_URL="https://github.com/createcrafts/CreateCraft-Launcher/releases/download/v${VERSION}/CreateCrafts-installer.AppImage"

sed \
  -e "s|__VERSION__|${VERSION}|g" \
  -e "s|__APPIMAGE_URL__|${APPIMAGE_URL}|g" \
  "${TEMPLATE}" > "${OUT_DIR}/PKGBUILD"

cat > "${OUT_DIR}/README-ARCH.md" <<EOF
# Arch package skeleton

Generated from template for version ${VERSION}.

## Build with makepkg

\`\`\`bash
cd packaging/arch/out
makepkg -si
\`\`\`

If your release host differs, update the \`source=\` URL in \`PKGBUILD\`.
EOF

echo "OK: ${OUT_DIR}/PKGBUILD"
