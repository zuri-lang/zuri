#!/bin/sh
# Installs Zuri on Linux or macOS from its published releases.
#
#   curl -fsSL https://zuri-lang.github.io/zuri/install.sh | sh
#
# Options go after `sh -s --`:
#
#   curl -fsSL https://zuri-lang.github.io/zuri/install.sh | sh -s -- --version 0.2.0
#
#   --version <version>  the release to install; the newest one by default
#   --prerelease         consider pre-releases when picking the newest
#   --no-modify-path     leave the shell's startup files alone
#
# The archive for this machine is downloaded, checked against the
# checksum published beside it, and run once to confirm the version it
# reports. Only then does it replace $ZURI_HOME/runtime (~/.zuri/runtime
# by default), which holds the executable with the standard library and
# the shipped commands beside it, where the runtime looks for them.
# `zuri` is linked into $ZURI_HOME/bin, the directory that globally
# installed tools put their launchers in, and that directory is added
# to PATH in the shell's startup file.
#
# Running it again installs over the top. `zuri upgrade` does the same
# from inside an installation.
#
# ZURI_RELEASES_URL lists releases in place of GitHub, in the same shape,
# for a mirror. GITHUB_TOKEN, when set, is sent to GitHub's API, where it
# raises the rate limit.

set -eu

RELEASES_URL="${ZURI_RELEASES_URL:-https://api.github.com/repos/zuri-lang/zuri/releases}"

say() {
  printf '%s\n' "$*"
}

fail() {
  printf 'zuri install: %s\n' "$*" >&2
  exit 1
}

usage() {
  say 'Installs Zuri from its published releases.'
  say ''
  say 'Usage: install.sh [--version <version>] [--prerelease] [--no-modify-path]'
}

wanted='latest'
prerelease=0
modify_path=1

while [ $# -gt 0 ]; do
  case "$1" in
    --version)
      [ $# -ge 2 ] || fail '--version needs a version, such as 0.2.0'
      wanted="${2#v}"
      shift 2
      ;;
    --version=*)
      wanted="${1#--version=}"
      wanted="${wanted#v}"
      shift
      ;;
    --prerelease)
      prerelease=1
      shift
      ;;
    --no-modify-path)
      modify_path=0
      shift
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      fail "unknown option '$1'; run with --help to see the options"
      ;;
  esac
done

# Downloads $1 to the file $2, or to standard output when $2 is `-`.
# The token goes to GitHub's API and nowhere else.
fetch() {
  if command -v curl > /dev/null 2>&1; then
    case "$1" in
      https://api.github.com/*)
        if [ -n "${GITHUB_TOKEN:-}" ]; then
          curl -fsSL -H "Authorization: Bearer $GITHUB_TOKEN" -H 'Accept: application/vnd.github+json' -o "$2" "$1"
          return
        fi
        ;;
    esac

    curl -fsSL -H 'Accept: application/vnd.github+json' -o "$2" "$1"
  elif command -v wget > /dev/null 2>&1; then
    case "$1" in
      https://api.github.com/*)
        if [ -n "${GITHUB_TOKEN:-}" ]; then
          wget -q --header "Authorization: Bearer $GITHUB_TOKEN" -O "$2" "$1"
          return
        fi
        ;;
    esac

    wget -q -O "$2" "$1"
  else
    fail 'downloading needs curl or wget'
  fi
}

# The SHA-256 of the file $1, in lower case.
sha256_of() {
  if command -v sha256sum > /dev/null 2>&1; then
    sha256sum "$1" | awk '{ print tolower($1) }'
  elif command -v shasum > /dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{ print tolower($1) }'
  else
    fail 'checking the download needs sha256sum or shasum'
  fi
}

# The platform a release names its archive after.
detect_target() {
  case "$(uname -s)" in
    Linux) system='unknown-linux-gnu' ;;
    Darwin) system='apple-darwin' ;;
    MINGW* | MSYS* | CYGWIN*)
      fail 'on Windows, install from PowerShell: irm https://zuri-lang.github.io/zuri/install.ps1 | iex'
      ;;
    *) fail "there is no Zuri release for $(uname -s)" ;;
  esac

  case "$(uname -m)" in
    x86_64 | amd64) cpu='x86_64' ;;
    aarch64 | arm64) cpu='aarch64' ;;
    *) fail "there is no Zuri release for $(uname -m) processors" ;;
  esac

  # A shell running under Rosetta reports x86_64 on an Apple silicon
  # Mac, which runs the native build better.
  if [ "$system" = 'apple-darwin' ] && [ "$cpu" = 'x86_64' ]; then
    if [ "$(sysctl -n sysctl.proc_translated 2> /dev/null || true)" = '1' ]; then
      cpu='aarch64'
    fi
  fi

  printf '%s-%s\n' "$cpu" "$system"
}

# Reads a release listing on standard input and prints the release to
# install as `version archive-url checksum-url`, or nothing when there
# is none. Mirrors what `zuri upgrade` accepts: drafts are skipped, a
# release offers this platform only when both its archive and the
# archive's .sha256 are attached, and a pre-release, by flag or by
# version, is only picked when asked for or named.
pick_release() {
  grep -oE '"(draft|prerelease)": *(true|false)|"browser_download_url": *"[^"]*"' |
    awk -v target="$1" -v wanted="$2" -v allow_pre="$3" '
      # Semantic version precedence: -1, 0 or 1.
      function compare(a, b,    ac, bc, an, bn, ap, bp, ai, bi, i, n) {
        ac = a; bc = b
        sub(/\+.*/, "", ac); sub(/\+.*/, "", bc)
        ap = ""; bp = ""
        if (index(ac, "-")) { ap = substr(ac, index(ac, "-") + 1); ac = substr(ac, 1, index(ac, "-") - 1) }
        if (index(bc, "-")) { bp = substr(bc, index(bc, "-") + 1); bc = substr(bc, 1, index(bc, "-") - 1) }
        split(ac, an, "."); split(bc, bn, ".")
        for (i = 1; i <= 3; i++) {
          if (an[i] + 0 != bn[i] + 0) return an[i] + 0 < bn[i] + 0 ? -1 : 1
        }
        if (ap == bp) return 0
        if (ap == "") return 1
        if (bp == "") return -1
        n = split(ap, ai, "."); split(bp, bi, ".")
        for (i = 1; i <= n || (i in bi); i++) {
          if (!(i in ai)) return -1
          if (!(i in bi)) return 1
          if (ai[i] == bi[i]) continue
          if (ai[i] ~ /^[0-9]+$/ && bi[i] ~ /^[0-9]+$/) return ai[i] + 0 < bi[i] + 0 ? -1 : 1
          if (ai[i] ~ /^[0-9]+$/) return -1
          if (bi[i] ~ /^[0-9]+$/) return 1
          return ai[i] < bi[i] ? -1 : 1
        }
        return 0
      }

      /^"draft"/ { draft = ($0 ~ /true/); next }
      /^"prerelease"/ { flagged = ($0 ~ /true/); next }

      {
        if (draft) next

        url = $0
        sub(/^"browser_download_url": *"/, "", url)
        sub(/"$/, "", url)

        name = url
        sub(/.*\//, "", name)

        checksum = 0
        if (name ~ /\.sha256$/) {
          checksum = 1
          sub(/\.sha256$/, "", name)
        }

        suffix = "-" target ".tar.gz"
        if (substr(name, 1, 5) != "zuri-") next
        if (length(name) <= 5 + length(suffix)) next
        if (substr(name, length(name) - length(suffix) + 1) != suffix) next

        version = substr(name, 6, length(name) - 5 - length(suffix))
        if (version !~ /^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$/) next

        if (checksum) sums[version] = url
        else archives[version] = url

        if (flagged || version ~ /^[^+]*-/) pre[version] = 1
      }

      END {
        best = ""
        for (version in archives) {
          if (!(version in sums)) continue

          if (wanted != "latest") {
            if ((version in pre) && !allow_pre && wanted !~ /^[^+]*-/) continue
            if (compare(version, wanted) == 0) best = version
            continue
          }

          if ((version in pre) && !allow_pre) continue
          if (best == "" || compare(version, best) > 0) best = version
        }

        if (best != "") print best, archives[best], sums[best]
      }
    '
}

target="$(detect_target)"
home="${ZURI_HOME:-$HOME/.zuri}"
runtime="$home/runtime"
bin="$home/bin"

mkdir -p "$home"

# Everything is staged beside the installation, so the final move is a
# rename on one filesystem rather than a copy.
staging="$(mktemp -d "$home/.install.XXXXXX")"
trap 'rm -rf "$staging"' EXIT INT TERM

case "$RELEASES_URL" in
  *\?*) listing_url="$RELEASES_URL&per_page=100" ;;
  *) listing_url="$RELEASES_URL?per_page=100" ;;
esac

fetch "$listing_url" "$staging/releases.json" || fail "could not read the releases at $RELEASES_URL"

release="$(pick_release "$target" "$wanted" "$prerelease" < "$staging/releases.json")"

if [ -z "$release" ]; then
  if [ "$wanted" = 'latest' ]; then
    fail "there is no release of zuri for $target yet"
  fi

  fail "there is no release of zuri $wanted for $target"
fi

version="${release%% *}"
urls="${release#* }"
archive_url="${urls%% *}"
checksum_url="${urls#* }"
asset="zuri-$version-$target.tar.gz"

say "Downloading zuri $version for $target"

fetch "$archive_url" "$staging/$asset" || fail "could not download $archive_url"
fetch "$checksum_url" "$staging/$asset.sha256" || fail "could not download $checksum_url"

expected="$(awk '{ print tolower($1); exit }' "$staging/$asset.sha256")"
actual="$(sha256_of "$staging/$asset")"

if [ -z "$expected" ] || [ "$expected" != "$actual" ]; then
  fail "$asset does not match its published checksum; nothing was installed"
fi

mkdir "$staging/unpacked"
tar -xzf "$staging/$asset" -C "$staging/unpacked" || fail "could not unpack $asset"

unpacked="$staging/unpacked/zuri-$version-$target"

for part in zuri libs cmds; do
  [ -e "$unpacked/$part" ] || fail "$asset has no $part in it; nothing was installed"
done

chmod 755 "$unpacked/zuri"

reported="$("$unpacked/zuri" --version 2> /dev/null || true)"

case "$reported" in
  *"Zuri $version "* | *"Zuri $version") ;;
  "") fail "the downloaded runtime did not start on this machine; nothing was installed" ;;
  *) fail "the downloaded runtime says it is '$(printf '%s' "$reported" | head -n 1)', not zuri $version" ;;
esac

# The old runtime is moved aside rather than deleted, so a failed move
# puts it back.
if [ -e "$runtime" ]; then
  mv "$runtime" "$staging/previous"
fi

if ! mv "$unpacked" "$runtime"; then
  [ -e "$staging/previous" ] && mv "$staging/previous" "$runtime"
  fail "could not move the runtime into $runtime; the previous one is back in place"
fi

mkdir -p "$bin"
ln -sf "$runtime/zuri" "$bin/zuri"

say "Installed zuri $version in $runtime"

# The line a startup file gets, with $HOME left for the shell to expand
# when the directory is under it.
case "$bin" in
  "$HOME"/*) shown_bin="\$HOME${bin#"$HOME"}" ;;
  *) shown_bin="$bin" ;;
esac

on_path=0

case ":$PATH:" in
  *":$bin:"*) on_path=1 ;;
esac

if [ "$on_path" = 0 ] && [ "$modify_path" = 1 ]; then
  shell_name="$(basename "${SHELL:-sh}")"

  case "$shell_name" in
    zsh)
      profile="${ZDOTDIR:-$HOME}/.zshrc"
      line="export PATH=\"$shown_bin:\$PATH\""
      ;;
    bash)
      # Terminal on macOS starts login shells, which read .bash_profile.
      if [ "$(uname -s)" = 'Darwin' ]; then
        profile="$HOME/.bash_profile"
      else
        profile="$HOME/.bashrc"
      fi
      line="export PATH=\"$shown_bin:\$PATH\""
      ;;
    fish)
      profile="${XDG_CONFIG_HOME:-$HOME/.config}/fish/config.fish"
      line="fish_add_path \"$shown_bin\""
      ;;
    *)
      profile="$HOME/.profile"
      line="export PATH=\"$shown_bin:\$PATH\""
      ;;
  esac

  if [ -f "$profile" ] && grep -qF "$line" "$profile"; then
    say "$profile already puts $bin on PATH"
  else
    mkdir -p "$(dirname "$profile")"
    printf '\n# zuri\n%s\n' "$line" >> "$profile"
    say "Added $bin to PATH in $profile"
  fi

  say ''
  say 'Open a new terminal, or run this one to use zuri straight away:'
  say ''
  say "  $line"
elif [ "$on_path" = 0 ]; then
  say ''
  say "Add $bin to PATH to run zuri from anywhere:"
  say ''
  say "  export PATH=\"$shown_bin:\$PATH\""
fi

found="$(command -v zuri 2> /dev/null || true)"

if [ "$on_path" = 1 ] && [ -n "$found" ] && [ "$found" != "$bin/zuri" ]; then
  say ''
  say "$found comes before $bin on PATH, so it is the zuri that runs."
fi

say ''
say 'Get started with:'
say ''
say '  zuri --help'
