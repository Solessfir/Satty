#!/usr/bin/env bash
set -euo pipefail

installer=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/install-bin.sh
work=$(mktemp -d)
running_pid=
trap 'if [[ -n $running_pid ]]; then kill "$running_pid" 2>/dev/null || true; fi; rm -rf -- "$work"' EXIT
mkdir -p "$work/mock" "$work/package/assets" "$work/package/man" "$work/package/completions" "$work/downloads" "$work/tmp"
cat > "$work/mock/curl" <<'MOCK'
#!/usr/bin/env bash
set -euo pipefail
[[ $1 == -fL && $3 == -o ]]
[[ ${MOCK_FAILURE:-} != download ]] || exit 22
cp "$MOCK_DOWNLOADS/${2##*/}" "$4"
MOCK
cat > "$work/mock/loader" <<'MOCK'
#!/usr/bin/env bash
if [[ ${MOCK_FAILURE:-} == dependencies ]]; then
    printf 'libgtk-4.so.1 => not found\n'
else
    printf 'libc.so.6 => /usr/lib/libc.so.6\n'
fi
MOCK
cat > "$work/mock/uname" <<'MOCK'
#!/usr/bin/env bash
case "$1" in
    -s) printf 'Linux\n' ;;
    -m) printf '%s\n' "${MOCK_ARCH:-x86_64}" ;;
esac
MOCK
chmod +x "$work/mock/"*
printf '#!/usr/bin/env bash\nprintf "new binary\\n"\n' > "$work/package/satty"
printf 'desktop\n' > "$work/package/satty.desktop"
printf 'icon\n' > "$work/package/assets/satty.svg"
printf 'manual\n' > "$work/package/man/satty.1"
printf 'license\n' > "$work/package/LICENSE"
for completion in satty.bash satty.fish _satty satty.elv satty.nu satty.ts; do
    printf 'completion\n' > "$work/package/completions/$completion"
done
for target in x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu; do
    asset="satty-$target.tar.gz"
    tar -czf "$work/downloads/$asset" -C "$work/package" .
    (cd "$work/downloads"; sha256sum "$asset" > "$asset.sha256")
done
export PATH="$work/mock:$PATH" MOCK_DOWNLOADS="$work/downloads" TMPDIR="$work/tmp" SATTY_INSTALL_LOADER="$work/mock/loader"
fixture_home="$work/home"
export PREFIX="$fixture_home/.local"
mkdir -p "$PREFIX/bin" "$fixture_home/.config/satty"
printf 'old binary\n' > "$PREFIX/bin/satty"
printf 'user config\n' > "$fixture_home/.config/satty/config.toml"
printf 'user CSS\n' > "$fixture_home/.config/satty/style.css"

check_preserved() {
    [[ $(cat "$PREFIX/bin/satty") == 'old binary' ]]
    [[ $(cat "$fixture_home/.config/satty/config.toml") == 'user config' ]]
    [[ $(cat "$fixture_home/.config/satty/style.css") == 'user CSS' ]]
    [[ -z $(find "$work/tmp" -mindepth 1 -print -quit) ]]
    [[ -z $(find "$PREFIX/bin" -name '.satty.*' -print -quit) ]]
}
expect_failure() {
    if bash "$installer" > "$work/output" 2>&1; then
        printf 'Installer unexpectedly succeeded: %s\n' "$1" >&2
        exit 1
    fi
    check_preserved
}

if bash "$installer" uninstall > "$work/output" 2>&1; then
    printf 'Installer unexpectedly accepted uninstall.\n' >&2
    exit 1
fi
check_preserved

MOCK_FAILURE=download expect_failure 'download failure'
MOCK_FAILURE=dependencies expect_failure 'missing dependency'
asset=satty-x86_64-unknown-linux-gnu.tar.gz
cp "$work/downloads/$asset.sha256" "$work/good.sha256"
printf '%064d  %s\n' 0 "$asset" > "$work/downloads/$asset.sha256"
expect_failure 'bad checksum'
cp "$work/good.sha256" "$work/downloads/$asset.sha256"

cp "$(command -v sleep)" "$PREFIX/bin/satty"
chmod +x "$PREFIX/bin/satty"
"$PREFIX/bin/satty" 30 &
running_pid=$!
sleep 0.05
bash "$installer" install > "$work/output" 2>&1
kill -0 "$running_pid"
kill "$running_pid"
wait "$running_pid" 2>/dev/null || true
running_pid=
[[ $("$PREFIX/bin/satty") == 'new binary' ]]
for path in share/applications/satty.desktop share/icons/hicolor/scalable/apps/satty.svg share/man/man1/satty.1 share/bash-completion/completions/satty share/fish/vendor_completions.d/satty.fish share/zsh/site-functions/_satty share/satty/completions/satty.elv share/satty/completions/satty.nu share/satty/completions/satty.ts share/licenses/satty/LICENSE; do
    [[ -f $PREFIX/$path ]]
done
[[ $(cat "$fixture_home/.config/satty/config.toml") == 'user config' ]]
[[ $(cat "$fixture_home/.config/satty/style.css") == 'user CSS' ]]
[[ -z $(find "$work/tmp" -mindepth 1 -print -quit) ]]
[[ -z $(find "$PREFIX/bin" -name '.satty.*' -print -quit) ]]
MOCK_ARCH=aarch64 bash "$installer" > "$work/output" 2>&1
[[ $("$PREFIX/bin/satty") == 'new binary' ]]
printf 'Installer tests passed.\n'
