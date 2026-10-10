#!/usr/bin/env bash
set -euo pipefail

fail() { printf 'Error: %s\n' "$*" >&2; exit 1; }

[[ $# -le 1 && ${1:-install} == install ]] || fail 'Usage: bash install-bin.sh [install]'

[[ $(uname -s) == Linux ]] || fail 'Only Linux is supported.'
case $(uname -m) in
    x86_64) target=x86_64-unknown-linux-gnu; loader=/lib64/ld-linux-x86-64.so.2 ;;
    aarch64|arm64) target=aarch64-unknown-linux-gnu; loader=/lib/ld-linux-aarch64.so.1 ;;
    *) fail 'Only x86_64 and aarch64 are supported.' ;;
esac
for command in curl sha256sum tar install mktemp; do
    command -v "$command" >/dev/null || fail "Required command not found: $command"
done

prefix=${PREFIX:-"$HOME/.local"}
asset="satty-${target}.tar.gz"
url="https://github.com/Solessfir/Satty/releases/latest/download"
work=$(mktemp -d)
binary_stage=
trap 'rm -rf -- "$work"; if [[ -n $binary_stage ]]; then rm -f -- "$binary_stage"; fi' EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

printf 'Downloading %s...\n' "$asset"
curl -fL "$url/$asset" -o "$work/$asset"
curl -fL "$url/$asset.sha256" -o "$work/$asset.sha256"
(
    cd "$work"
    read -r checksum filename < "$asset.sha256"
    [[ $checksum =~ ^[[:xdigit:]]{64}$ && ${filename#\*} == "$asset" ]] || exit 1
    printf '%s  %s\n' "$checksum" "$asset" | sha256sum --check --strict
) || fail 'Release checksum verification failed.'
mkdir "$work/package"
tar -xzf "$work/$asset" -C "$work/package"
package="$work/package"
[[ -f $package/satty && ! -L $package/satty ]] || fail 'Release archive does not contain the satty binary.'

# Check compatibility before replacing an existing working binary. The system loader lists
# libraries without running the downloaded file, which ldd may do through its embedded loader.
loader=${SATTY_INSTALL_LOADER:-$loader}
[[ -x $loader ]] || fail "System dynamic loader not found: $loader"
if ! dependencies=$("$loader" --list "$package/satty" 2>&1); then
    printf '%s\n' "$dependencies" >&2
    fail 'Cannot load this release. It requires Linux runtime libraries compatible with Ubuntu 24.04.'
fi
if [[ $dependencies == *'not found'* ]]; then
    printf '%s\n' "$dependencies" >&2
    fail 'Missing runtime libraries. See README.md for the required runtime packages.'
fi

mkdir -p "$prefix/bin"
binary_stage=$(mktemp "$prefix/bin/.satty.XXXXXX")
install -m755 "$package/satty" "$binary_stage"

install_optional() {
    if [[ -f $package/$1 ]]; then
        install -Dm644 "$package/$1" "$prefix/$2"
    fi
}
install_optional satty.desktop share/applications/satty.desktop
install_optional assets/satty.svg share/icons/hicolor/scalable/apps/satty.svg
install_optional man/satty.1 share/man/man1/satty.1
install_optional completions/satty.bash share/bash-completion/completions/satty
install_optional completions/satty.fish share/fish/vendor_completions.d/satty.fish
install_optional completions/_satty share/zsh/site-functions/_satty
for completion in satty.elv satty.nu satty.ts; do
    install_optional "completions/$completion" "share/satty/completions/$completion"
done
install_optional LICENSE share/licenses/satty/LICENSE

# Rename on the same filesystem also works while the old executable is running.
mv -f -- "$binary_stage" "$prefix/bin/satty"
binary_stage=
printf 'Installed Satty to %s/bin/satty. Ensure %s/bin is in PATH.\n' "$prefix" "$prefix"
