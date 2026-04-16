#!/usr/bin/env bash
set -euo pipefail

REPO="AgentsMesh/Loopal"
BINARY="loopal"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

info()  { printf '\033[1;34m[info]\033[0m  %s\n' "$*"; }
error() { printf '\033[1;31m[error]\033[0m %s\n' "$*" >&2; exit 1; }

detect_platform() {
    local os arch
    os="$(uname -s)"
    arch="$(uname -m)"

    case "$os" in
        Darwin) os="apple-darwin" ;;
        Linux)  os="unknown-linux-gnu" ;;
        *)      error "Unsupported OS: $os (supported: macOS, Linux)" ;;
    esac

    case "$arch" in
        arm64|aarch64) arch="aarch64" ;;
        x86_64|amd64)  arch="x86_64" ;;
        *)             error "Unsupported architecture: $arch (supported: arm64, x86_64)" ;;
    esac

    if [ "$os" = "apple-darwin" ] && [ "$arch" = "x86_64" ]; then
        error "macOS x86_64 is not supported — Apple Silicon (arm64) only"
    fi

    TARGET="${arch}-${os}"
}

resolve_version() {
    if [ -n "${VERSION:-}" ]; then
        TAG="$VERSION"
        [[ "$TAG" == v* ]] || TAG="v$TAG"
        info "Using specified version: $TAG"
        return
    fi

    info "Fetching latest release..."
    local api_url="https://api.github.com/repos/${REPO}/releases/latest"
    TAG="$(curl -fsSL "$api_url" | grep '"tag_name"' | head -1 | sed 's/.*"tag_name": *"//;s/".*//')"
    [ -n "$TAG" ] || error "Failed to determine latest version"
    info "Latest version: $TAG"
}

download_and_install() {
    local archive="${BINARY}-${TAG}-${TARGET}.tar.gz"
    local url="https://github.com/${REPO}/releases/download/${TAG}/${archive}"
    local tmpdir
    tmpdir="$(mktemp -d)"
    trap 'rm -rf "$tmpdir"' EXIT

    info "Downloading ${archive}..."
    if command -v curl &>/dev/null; then
        curl -fSL --progress-bar "$url" -o "${tmpdir}/${archive}"
    elif command -v wget &>/dev/null; then
        wget -q --show-progress "$url" -O "${tmpdir}/${archive}"
    else
        error "Neither curl nor wget found — install one and retry"
    fi

    info "Extracting..."
    tar -xzf "${tmpdir}/${archive}" -C "$tmpdir"

    mkdir -p "$INSTALL_DIR"

    local binary_path="${tmpdir}/${BINARY}-${TAG}-${TARGET}/${BINARY}"
    [ -f "$binary_path" ] || binary_path="${tmpdir}/${BINARY}"
    [ -f "$binary_path" ] || error "Binary not found in archive"

    cp -f "$binary_path" "${INSTALL_DIR}/${BINARY}"
    chmod +x "${INSTALL_DIR}/${BINARY}"

    info "Installed to ${INSTALL_DIR}/${BINARY}"
}

check_path() {
    case ":${PATH}:" in
        *":${INSTALL_DIR}:"*) ;;
        *)
            printf '\n\033[1;33m[note]\033[0m  %s is not in your PATH.\n' "$INSTALL_DIR"
            echo "       Add it to your shell profile:"
            echo ""
            echo "         export PATH=\"${INSTALL_DIR}:\$PATH\""
            echo ""
            ;;
    esac
}

main() {
    info "Installing ${BINARY}..."
    detect_platform
    info "Platform: ${TARGET}"
    resolve_version
    download_and_install
    check_path
    info "Done! Run '${BINARY} --help' to get started."
}

main
