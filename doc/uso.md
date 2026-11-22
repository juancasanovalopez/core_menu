# Usage Guide

---

## Application Screens

The application has three navigation levels traversed in order:

```
Main menu  →  Submenu  →  Command output  →  Submenu  →  Main menu
```

---

## Keyboard Shortcuts

### Global (available on every screen)

| Key | Action |
|-------|--------|
| `Ctrl+C` | Exit the application immediately |

### Main Menu

Above the menu, a minimal hardware summary shows:

- **Available/total RAM**, in binary units (GiB/MiB), using `MemAvailable`
  rather than only free memory.
- **Physical CPU cores and logical threads**, counting online CPUs and their
  physical core groups across all sockets.
- **GPUs**, identified by model and PCI address, with available/total memory and
  separate labels for dedicated VRAM, reserved shared RAM, and dynamic shared RAM.

The summary refreshes every second while the main menu is visible. It reads
`/proc/meminfo`, CPU topology, and PCI/DRM information from the kernel. GPU models
use the local `pci.ids` database if present; otherwise PCI identifiers are shown.
Read or data errors are displayed explicitly instead of showing zero values.

AMD (`amdgpu`) uses the DRM APU flag to distinguish reserved shared RAM from
dedicated VRAM, and sysfs counters for available/total memory. Its dynamic shared
RAM is the GTT pool limit, not additional physical memory. Shared availability
(`~`) is an estimate capped by system `MemAvailable`; allocations are not
guaranteed, and shared pools must not be added to system RAM.

NVIDIA uses `nvidia-smi` from the installed driver, with a one-second timeout.
Intel (`i915`) uses DRM memory regions: system regions are shared and local
regions are dedicated. Free Intel dedicated VRAM is shown as **N/D**, because
the unprivileged query does not provide reliable usage accounting.
AMD and Intel queries require access to the GPU render node in `/dev/dri`, not
root. Unsupported drivers (including `xe`) and inaccessible telemetry are
reported explicitly; no memory type is guessed from the vendor or memory size.
If all summary lines do not fit, an expansion hint preserves space for menu
navigation.

| Key | Action |
|-------|--------|
| `↑` / `k` | Move the cursor up |
| `↓` / `j` | Move the cursor down |
| `Enter` | Enter the selected submenu |
| `q` / `Q` | Exit the application |
| `u` | Open update status/details |

### Updates

On startup, a background check compares the installed SemVer version with the
stable GitHub release marked **latest**. If newer, an offer opens once when the
main menu is visible; submenus and text input are not interrupted. Declining
does not reopen the offer automatically during the same run.

1. Press `s` to download and verify the update for Linux x86_64 or ARM64.
2. Inspect the verified version and installation destination.
3. Press `s` again to install. System installation at `/usr/local/bin/core_menu`
   requests sudo only at this stage; start the application without sudo.
4. On success, note the recovery-copy path and restart core_menu manually.

`n`, `Esc` or `q` cancels/returns without installing; `↑`/`↓` or `k`/`j` scrolls
details on small terminals. The update screen reports progress and errors,
including offline/rate-limited checks and unavailable or unsigned release assets.
Failed checks do not prevent using the menus. The `u` shortcut can reopen details
after declining. Cancelled downloads require restarting the application to retry.

See [installation, publication and recovery](instalacion.md#7-updates-and-release-publishing)
for supported paths, signature verification and signing-secret configuration.

### Inside a Submenu

| Key | Action |
|-------|--------|
| `↑` / `k` | Move the cursor up |
| `↓` / `j` | Move the cursor down |
| `Enter` | Run the selected command |
| `Esc` / `q` / `Backspace` | Return to the main menu |

### Output Screen (command result)

| Key | Action |
|-------|--------|
| `↑` / `k` | Scroll up |
| `↓` / `j` | Scroll down |
| `Enter` / `Esc` / `q` / `Backspace` | Return to the submenu |

---

## Available Submenus

### Network

Groups network and firewall inspection commands.

| Command | Description | Tool |
|---------|-------------|-------------|
| Network interfaces | IPs and status of each interface | `ip addr show` |
| Listening ports | Active TCP/UDP ports with the associated process | `ss -tulnp` |
| Established connections | Active TCP connections | `ss -tnp` |
| Routing table | Active network routes | `ip route show` |
| UFW firewall | Active UFW rules | `ufw status verbose` |
| iptables rules | iptables filter table (requires root) | `iptables -L` |
| DNS configuration | Configured DNS servers | `/etc/resolv.conf` |
| Interface statistics | Packet and byte counters | `ip -s link` |
| ARP / neighbors | ARP and IPv6 Neighbor Discovery table | `ip neigh show` |

---

### Users

Groups commands for inspecting system accounts and sessions.

| Command | Description | Tool |
|---------|-------------|-------------|
| Logged-in users | Active sessions on the system | `who -a` |
| Current activity | Users, uptime, and load | `w` |
| Recent logins | Login history (last 30) | `last -n 30` |
| Failed attempts | Failed logins (requires root) | `lastb -n 20` |
| Users with login shells | Accounts that can log in | Filtered `/etc/passwd` |
| All users | Complete system listing | `/etc/passwd` |
| System groups | Group listing | `/etc/group` |
| Current identity | UID, GID, and groups for the current process | `id` + `groups` |
| Sudo privileges | Commands allowed for the current user | `sudo -l` |

---

### Storage

Groups commands for inspecting disks, partitions, and space usage.

| Command | Description | Tool |
|---------|-------------|-------------|
| Disk usage | Free and used space by filesystem | `df -h` |
| Inode usage | Inode availability | `df -i` |
| Block devices | Disks and partitions with details | `lsblk` |
| Physical disk information | Model, size, and transport type | `lsblk -d` |
| Mount points | Mounted filesystems | `mount` |
| Largest directories | Top 20 directories by size | `du` |
| Current directory usage | Size of each entry in the current directory | `du -sh *` |
| Disk SMART health | SMART status (requires smartmontools) | `smartctl` |
| User quota | Quota usage for the current user | `quota -s` |

---

## Command Behavior

- Commands are run with `sh -c "..."` when `Enter` is pressed.
- Output is shown on a full-screen view with **scroll** support (`↑↓`).
- ANSI escape codes are automatically removed to display clean text.
- If a command writes to `stderr` (for example, because it requires root), it is also shown with the `[stderr]` prefix.
- If a command produces no output, the message `(No output)` is shown.

---

## Complete Session Example

```
1. Run the application
   $ ./target/release/core_menu

2. Use ↓ to select "Network" and press Enter

3. Use ↓ to select "Listening ports" and press Enter
   → The system runs: ss -tulnp
   → The output is shown on screen
   → Use ↑↓ to scroll if the output is long

4. Press Esc to return to the "Network" submenu

5. Press Esc to return to the main menu

6. Select "Exit" or press q to close
```
