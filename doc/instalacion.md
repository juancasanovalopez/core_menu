# Installation and Execution

---

## Requirements

The release workflow builds statically linked Linux executables for **x86_64**
(64-bit Intel/AMD) and **aarch64** (64-bit ARM). Rust and Cargo are not needed
to install or run a published binary. A compatible Linux kernel and an interactive terminal
supporting ANSI escape sequences are required; compatibility with every kernel,
distribution or terminal cannot be guaranteed. Individual menu actions can
require additional system commands or privileges.

The initial installer uses a POSIX shell, Python 3.8+, curl, OpenSSH `ssh-keygen`
with `-Y verify` support, coreutils, grep, and system TLS certificates. For a system
installation it also needs sudo and `/usr/bin/python3`. If dependencies are
missing, the installer offers to install them with apt, dnf or pacman after
confirmation; otherwise install them manually using your distribution's package
manager. Rust is only needed for building from source.

## 1. Install a precompiled release

Download both installer files over HTTPS. This example requires curl for the
initial download; if it is missing, install curl first or obtain both files
through another trusted channel.

```bash
mkdir -p core-menu-installer
cd core-menu-installer
curl --proto '=https' --proto-redir '=https' --tlsv1.2 -fLO \
  https://raw.githubusercontent.com/juancasanovalopez/core_menu/master/scripts/install.sh
curl --proto '=https' --proto-redir '=https' --tlsv1.2 -fLO \
  https://raw.githubusercontent.com/juancasanovalopez/core_menu/master/scripts/install_release.py
```

Inspect both scripts and verify their origin before running them. The installer
contains the release public key; its authenticity depends on how you obtained
the scripts. Compare that key with
[`config/update-public-key.pub`](../config/update-public-key.pub) obtained
independently through a trusted channel. The signed release manifest protects
the binary, but cannot authenticate an installer that was already altered.

```bash
chmod go-w install_release.py
sh install.sh
```

Run this without sudo. Choose **1** for `~/.local/bin/core_menu`, or **2** for
`/usr/local/bin/core_menu`; the latter asks for sudo only for installation.
Dependency installation, if accepted, also requires sudo. The installer
verifies the latest stable release's signed manifest, architecture, size and
SHA-256 before installation; the privileged system helper verifies them again.
Existing executables require confirmation, and a downgrade or an unknown
installed version requires an additional confirmation. Managed packages and
unsafe destinations are not replaced. A replaced executable is retained in the
printed recovery path.

The installer needs a published signed release matching the
[asset contract](#release-asset-contract). If none is available, it stops with
an error. The historical unsigned `v1.0.0` binary cannot be installed by this
flow.

Ensure `~/.local/bin` is in `PATH` for a user installation; for the current
shell, if needed:

```bash
export PATH="$HOME/.local/bin:$PATH"
```

Add that line to your shell configuration to make it permanent. Check the
installation and start the application:

```bash
core_menu --version
core_menu
```

To uninstall, remove the executable from the selected destination:

```bash
rm "$HOME/.local/bin/core_menu"    # user installation
sudo rm /usr/local/bin/core_menu  # system installation
```

Recovery directories and `.core_menu.update.lock` are not automatically
removed; retain any backup you still need.

## 2. Build from source (optional)

Building requires a current stable Rust toolchain and Cargo, installed for
example through [rustup](https://rustup.rs). The locked updater dependencies
were verified with rustc 1.94.1; compatibility with Rust 1.70 is not claimed.

```bash
git clone https://github.com/juancasanovalopez/core_menu.git
cd core_menu
cargo build --locked --release
./target/release/core_menu
```

For development, `cargo run` builds and runs the debug binary. Alternatively,
`cargo install --path .` installs a locally built binary to `~/.cargo/bin`;
remove that installation with `cargo uninstall core_menu`.

## 3. Troubleshooting

### The terminal is left in an unusual state after an error

The application installs a panic hook that restores the terminal automatically.
If the screen remains in raw mode, run:

```bash
reset
```

### The terminal does not show colors or special characters

Make sure the `TERM` variable is configured correctly:

```bash
echo $TERM
# Expected: xterm-256color, screen-256color, tmux-256color...
```

If the variable is not set, force a compatible value before running:

```bash
TERM=xterm-256color core_menu
```

### Some commands report "root permissions are required"

Some system commands (iptables, lastb, etc.) require elevated privileges.
Run the application with `sudo` only if you trust the environment:

```bash
sudo core_menu
```

### Missing installer dependencies or unsupported OpenSSH

Accept the proposed package installation or install Python 3.8+, curl, OpenSSH,
coreutils and grep manually. `ssh-keygen -Y verify` must be supported by the
installed OpenSSH version. If sudo, `/usr/bin/python3`, or a supported package
manager is unavailable, install the missing dependencies through the
distribution's normal administrative process.

### Error: `command not found: cargo` while building from source

The Rust environment is not active in the current session. Run:

```bash
source "$HOME/.cargo/env"
```

Add that line to `~/.bashrc` or `~/.zshrc` to activate it automatically.

---

## 4. Updates and Release Publishing

### Runtime behavior and installation paths

The application checks the stable release marked **latest** on
`juancasanovalopez/core_menu` once per interactive startup. It does not choose
the greatest tag from release history. The check runs in the background with
a five-second HTTP timeout. Downloads have a combined 120-second deadline,
a 64 MiB executable limit, a 16 KiB manifest limit and a 4 KiB signature limit.
No GitHub token is required for public releases.

Version precedence uses SemVer, not string ordering: `1.10.0` is newer than
`1.9.0`. Only a strictly newer stable version is installable. Build metadata
does not increase precedence. `--version` prints the installed package version
without entering the TUI or checking for updates.

Run without sudo. User-owned executables need a writable installation directory.
Root-owned installations are supported only at `/usr/local/bin/core_menu`,
with root-owned, non-group/world-writable path components. The final installer
requests sudo, re-verifies the signed payload, and performs no network access.
When started through sudo, checking can run as the invoking user, but installation
requires restarting without sudo. Direct root sessions cannot check for updates.
Setuid/setgid files and package-managed executables are not updated in place.

No update is installed without both confirmations. The replacement uses the
same filesystem, a concurrent-install lock and an atomic rename. Interrupted,
corrupt or untrusted downloads leave the installed executable unchanged.

### Release asset contract

For each target, a release contains these three files (six assets total):

```text
core_menu-v<VERSION>-<TARGET>
update-manifest-<TARGET>.json
update-manifest-<TARGET>.sig
```

Targets are `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl`.
The manifest is JSON containing `schema` (1), `version` (without the tag's `v`),
`target`, `asset`, `size` (bytes) and `sha256` (lowercase hexadecimal). Its exact
bytes are signed with an Ed25519 SSH signature using namespace `core_menu-release`.
The application embeds the trusted public key from
[`config/update-public-key.pub`](../config/update-public-key.pub).
Unsigned releases, wrong architectures or missing assets cannot be installed.

### Signing-key setup

The maintainer must possess the private Ed25519 key matching the embedded public
key. Never commit or share the private key. Configure its complete OpenSSH private
key text as the repository Actions secret **`UPDATE_SIGNING_KEY`** in
Settings → Secrets and variables → Actions. It must be usable without an
interactive passphrase by the publishing job.

If the matching private key is unavailable, create a dedicated signing key
**outside the repository**, securely retain its private half, and deliberately
replace only the public key before distributing the first updater-enabled binary.
The workflow rejects a private/public mismatch. Replacing the trusted public key
later prevents already-distributed binaries from accepting the new signatures;
key rotation needs a separately designed migration.

### Publishing a version

#### One-time automation setup

The [Release Please configuration](../release-please-config.json) uses the Rust
strategy and starts from `1.0.0`, counting commits after
`c8ffac37bae784b98b90c8b0378909870e60e629` (the production version preparation).
Subsequent versions use the previous release boundary, not this bootstrap commit.
The historical binary directory is not a Git tag.

Enable **Allow GitHub Actions to create and approve pull requests** in repository
Settings → Actions → General. The
[preparation workflow](../.github/workflows/release-please.yml) uses the built-in
`GITHUB_TOKEN` with contents, pull-request and issue permissions; no additional
personal access token is required. Keep the signing secret configured above.

The release workflows use JavaScript actions on Node.js 24 and require runners
that support that runtime. Artifact downloads fail if their digest does not
match, rather than only warning; the signed release asset contract is unchanged.

Pushes to `master` (or manually running **Prepare release** on `master`) create
or update a release PR, but never create a tag or publish a release.
PRs opened with `GITHUB_TOKEN` do not automatically trigger other workflows;
run the local checks below before tagging, including when branch policies
require an explicitly triggered check.

#### Preparing and publishing

1. Use Conventional Commits for changes:

   | Commit | Version increment |
   |--------|-------------------|
   | `fix: correct GPU memory reporting` | PATCH |
   | `feat: add a storage command` | MINOR |
   | `feat!: change the CLI contract` | MAJOR |
   | Any type with a `BREAKING CHANGE:` footer | MAJOR |
   | Only `docs:`, `chore:`, or other non-releasing maintenance | None |

   The highest applicable increment wins and resets lower components according
   to SemVer. Explicitly describe incompatible CLI, workflow, configuration or
   update-contract changes: the tool cannot infer compatibility from code.
   When squash-merging feature PRs, preserve the conventional commit title and
   any breaking-change footer in the final commit.
2. Review and merge the release PR. Release Please updates `Cargo.toml`,
   the local package version in `Cargo.lock`, `.release-please-manifest.json`,
   and `CHANGELOG.md` together. Do not increment versions for each build or
   edit these generated versions independently. Only stable `MAJOR.MINOR.PATCH`
   versions are published, without prereleases, metadata or leading zeroes.
3. Check out the exact merged release commit. Run `cargo test --locked`,
   `cargo check --locked` and
   `cargo clippy --locked --all-targets`. Tests also require `ssh-keygen`
   (OpenSSH client) to verify signing interoperability.
   With Python 3.11+ available, validate the intended tag (replace the example
   with the version in the merged PR):

   ```bash
   git fetch origin --tags
   python3 -m unittest discover -s scripts -p 'test_*.py'
   python3 scripts/validate_release.py v1.1.0
   git tag -a v1.1.0 -m "Release v1.1.0"
   git push origin v1.1.0
   ```

   Never move an existing published tag. Do not include additional development
   commits when choosing the release commit.
4. The [release workflow](../.github/workflows/release.yml) validates the stable
   tag against Cargo, the lockfile and the Release Please manifest, and rejects
   versions not newer than other existing stable tags (numeric comparison).
   It tests and builds natively on x86_64 and ARM64 Linux using musl,
   verifies the static ELF architecture, generates/signs/verifies both manifests,
   and uploads all six files to a draft release.
5. Only after successful upload does the workflow publish the draft and mark it
   latest. It does not overwrite an existing release. If a publishing attempt
   fails, inspect/remove the incomplete draft before retrying.
6. After successful publication, the workflow removes `autorelease: pending`
   from the merged release PR associated with the tagged commit. This closes the
   PR-only Release Please cycle so the next changes can prepare a new release.
   If only this final bookkeeping step fails, remove that label from the release
   PR after verifying all assets and the published release; do not recreate or
   overwrite the release. Do not merge another release PR before finishing the
   current publication.

The binaries stored historically in `releases/v1.0.0/` do not satisfy this new
asset/signature contract. Do not mark an old unsigned release as the updater's
latest release.

### Initial manual install and recovery

An old binary without updater support cannot bootstrap this feature itself.
Use the installer above, build and install the new source, or download the
matching binary and its manifest/signature from a release. For manual
verification, obtain the trusted
public key through a trusted channel, create an allowed-signers file containing
`core_menu <public-key>`, and verify the exact manifest before trusting its hash:

```bash
ssh-keygen -Y verify -f allowed_signers -I core_menu \
  -n core_menu-release -s update-manifest-<TARGET>.sig \
  < update-manifest-<TARGET>.json
```

Replace `<TARGET>` with the actual target before running the command, and check
the manifest's version, target, asset name, file size and SHA-256 against the
downloaded binary. Then install that verified executable, for example in
`~/.local/bin/core_menu` or `/usr/local/bin/core_menu`.

After an integrated update, core_menu prints the exact backup path, such as
`/usr/local/bin/.core_menu-recovery-XXXXXX/previous`, then exits. Restart
`core_menu` manually. To recover, stop other running instances and move that
specific `previous` file over the executable, using sudo only for a system
installation. Keep the backup until the new version has been verified; recovery
directories and `.core_menu.update.lock` are not automatically deleted.

A 404 means no stable release is accessible, not necessarily that the installed
version is current. Rate limits, timeouts, missing assets and verification failures
are shown in the update screen (`u`); normal system-management menus remain usable.
