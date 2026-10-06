# Installation and Execution

---

## Requirements

| Requirement | Minimum version | Notes |
|-----------|---------------|-------|
| Rust + Cargo | 1.70 | Installed through [rustup](https://rustup.rs) |
| Terminal | Any | xterm, GNOME Terminal, Alacritty, tmux, etc. |
| Operating system | Linux (any distribution) | — |

> **Why 1.70?** It is the oldest stable version compatible with the declared dependencies (`ratatui 0.26`, `crossterm 0.27`, `anyhow 1.0`).
> The project was developed and verified with **rustc 1.94.1**.

---

## 1. Install Rust (if not already installed)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Follow the installer instructions (the default option is recommended).
Then activate the environment in the current session:

```bash
source "$HOME/.cargo/env"
```

Verify the installation:

```bash
rustc --version
cargo --version
```

---

## 2. Obtain the Source Code

### Option A — Clone the repository (if hosted in Git)

```bash
git clone <REPOSITORY_URL>
cd core_menu
```

### Option B — From a local copy

```bash
cd /path/to/core_menu
```

---

## 3. Build

### Development mode (faster to compile, unoptimized binary)

```bash
cargo build
```

The binary is generated at:

```
target/debug/core_menu
```

### Release mode (recommended for daily use, optimized binary ~963 KB)

```bash
cargo build --release
```

The binary is generated at:

```
target/release/core_menu
```

---

## 4. Run

### From the project directory

```bash
# Debug mode
./target/debug/core_menu

# Release mode (recommended)
./target/release/core_menu
```

### Cargo shortcut

```bash
cargo run           # build in debug mode and run
cargo run --release # build in release mode and run
```

---

## 5. Global Installation (optional)

To run `core_menu` from any directory without specifying its path:

```bash
cargo install --path .
```

This copies the binary to `~/.cargo/bin/core_menu`.
Make sure `~/.cargo/bin` is in your `PATH` (the rustup installer configures it automatically).

Verify that it is available globally:

```bash
core_menu
```

To uninstall it:

```bash
cargo uninstall core_menu
```

---

## 6. Troubleshooting

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
TERM=xterm-256color ./target/release/core_menu
```

### Some commands report "root permissions are required"

Some system commands (iptables, lastb, etc.) require elevated privileges.
Run the application with `sudo` only if you trust the environment:

```bash
sudo ./target/release/core_menu
```

### Error: `command not found: cargo`

The Rust environment is not active in the current session. Run:

```bash
source "$HOME/.cargo/env"
```

Add that line to `~/.bashrc` or `~/.zshrc` to activate it automatically.
