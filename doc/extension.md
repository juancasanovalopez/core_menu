# Extension Guide

How to extend `core_menu` by adding new submenus and commands without modifying the core logic.

---

## Menu System Architecture

```
src/submenus/
├── mod.rs          ← core types + get_submenus() registry + run_shell()
├── network.rs      ← "Network" submenu
├── users.rs        ← "Users" submenu
├── storage.rs      ← "Storage" submenu
└── <new>.rs        ← ← ← add yours here
```

Each submenu is an independent Rust module that exports a single public function, `create() -> SubMenu`.
The central registry in `mod.rs` is the only integration point.

### Hardware Summary Architecture

The main menu summary is separate from the commands in `src/submenus/`.
[src/hardware.rs](../src/hardware.rs) coordinates collection and re-exports the
data types used by the application and UI:

- [src/hardware/memory.rs](../src/hardware/memory.rs): RAM data and parsing tests.
- [src/hardware/cpu.rs](../src/hardware/cpu.rs): CPU topology and counting tests.
- [src/hardware/gpu.rs](../src/hardware/gpu.rs): GPU detection, memory queries,
  and their tests.

Each collector is visible only to its parent module. RAM is collected before
GPU data because shared GPU memory estimates depend on available system RAM.

### Versioning extensions

Use Conventional Commits for extensions: `feat:` for backward-compatible
functionality, `fix:` for fixes, and `!` or a `BREAKING CHANGE:` footer for
incompatible behavior. Release Please calculates the next version; rebuilding
does not increment it. Follow the
[release preparation and publishing procedure](instalacion.md#publishing-a-version)
before creating a version tag.

---

## Case 1 — Add a Command to an Existing Submenu

Open the corresponding submenu file (for example, [src/submenus/network.rs](../src/submenus/network.rs)) and add a `MenuItem::new(...)` inside the `vec![]` in the `create()` function:

```rust
MenuItem::new(
    "Display name",            // ≤ 32 characters recommended
    "Short description",       // ≤ 42 characters recommended
    || run_shell("command"),   // function that runs the command
),
```

### Real Example — Add "ping gateway" to network.rs

```rust
MenuItem::new(
    "Ping gateway",
    "Check connectivity to the gateway",
    || run_shell(
        "gw=$(ip route | awk '/default/{print $3; exit}'); \
         ping -c 4 \"$gw\" 2>/dev/null \
         || echo 'No default gateway found'",
    ),
),
```

Rebuild the application and the command appears automatically in the submenu:

```bash
cargo build --release
```

---

## Case 2 — Add a Complete New Submenu

### Step 1 — Create the Submenu File

Create `src/submenus/<name>.rs`.
You can copy `network.rs` as a template and adapt its contents.

Minimal structure:

```rust
//! # Submenu: <Name>
//!
//! Brief description of what this submenu groups.

use super::{run_shell, MenuItem, SubMenu};

/// Build and return the <name> submenu.
pub fn create() -> SubMenu {
    SubMenu::new(
        "<Name>",                // appears in the main menu
        "<Short description>",   // appears next to the name
        vec![
            MenuItem::new(
                "First command",
                "Description of the first command",
                || run_shell("shell_command"),
            ),
            // Add more MenuItem::new(...) entries here
        ],
    )
}
```

### Step 2 — Declare the Module in mod.rs

Open [src/submenus/mod.rs](../src/submenus/mod.rs) and add the module declaration in the indicated section:

```rust
// ── Submenu modules ───────────────────────────────────────────────────────────
pub mod network;
pub mod storage;
pub mod users;
pub mod <name>;     // ← add here
```

### Step 3 — Register the Submenu in get_submenus()

In the same `mod.rs` file, add the `create()` call inside `get_submenus()`:

```rust
pub fn get_submenus() -> Vec<SubMenu> {
    vec![
        network::create(),
        users::create(),
        storage::create(),
        <name>::create(),     // ← add here
    ]
}
```

The **order of the `vec![]` determines the display order** in the main menu.

### Step 4 — Build

```bash
cargo build --release
./target/release/core_menu
```

The new submenu appears in the main menu automatically.

---

## run_shell() Function Reference

Defined in `src/submenus/mod.rs`. It runs a string with `sh -c "..."` and returns the output as a `String`.

```rust
pub fn run_shell(cmd: &str) -> String
```

Comportamiento:

| Situation | Returned result |
|-----------|--------------------|
| Command with stdout output | stdout text |
| stdout is empty but stderr has content | `[stderr]:\n<text>` |
| Command produces no output | `(No output)` |
| Error invoking the process | `Error running command: <details>` |

ANSI escape codes are automatically removed before returning the text.

> **Security note:** Use `run_shell` only with **literal** command strings defined in the source code. Never build the `cmd` argument from user-provided data.

---

## Type Reference

### SubMenu

```rust
pub struct SubMenu {
    pub name:        &'static str,   // name in the main menu
    pub description: &'static str,   // description next to the name
    pub items:       Vec<MenuItem>,  // list of commands
}
```

### MenuItem

```rust
pub struct MenuItem {
    pub name:        &'static str,   // display name in the submenu
    pub description: &'static str,   // description next to the name
    pub command:     fn() -> String, // function that runs the command
}
```

The `command` field is a **function pointer** (not a capturing closure). All closures in the form `|| run_shell("...")` meet this requirement as long as they do not capture environment variables.

---

## Complete Example — "Processes" Submenu

`src/submenus/processes.rs`:

```rust
//! # Submenu: Processes
//!
//! Inspect running processes and system load.

use super::{run_shell, MenuItem, SubMenu};

pub fn create() -> SubMenu {
    SubMenu::new(
        "Processes",
        "Running processes and system load",
        vec![
            MenuItem::new(
                "Top CPU",
                "20 processes with the highest CPU usage",
                || run_shell("ps aux --sort=-%cpu | head -21"),
            ),
            MenuItem::new(
                "Top memory",
                "20 processes with the highest RAM usage",
                || run_shell("ps aux --sort=-%mem | head -21"),
            ),
            MenuItem::new(
                "System load",
                "Uptime and load averages (uptime)",
                || run_shell("uptime"),
            ),
            MenuItem::new(
                "Process tree",
                "Process tree (pstree -p)",
                || run_shell("pstree -p 2>/dev/null || ps axjf"),
            ),
            MenuItem::new(
                "Active systemd services",
                "System units in the 'running' state",
                || run_shell("systemctl list-units --type=service --state=running --no-pager"),
            ),
        ],
    )
}
```

`src/submenus/mod.rs` — cambios necesarios:

```rust
pub mod network;
pub mod processes;   // ← declarar
pub mod storage;
pub mod users;

pub fn get_submenus() -> Vec<SubMenu> {
    vec![
        network::create(),
        users::create(),
        storage::create(),
        processes::create(),   // ← register
    ]
}
```
