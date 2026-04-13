//! # Submenú: GPU
//!
//! Agrupa comandos para inspeccionar el estado de la GPU,
//! uso de VRAM y rendimiento gráfico.
//!
//! ## GPUs soportadas
//!
//! - **NVIDIA** — via `nvidia-smi`
//! - **AMD**    — via driver `amdgpu` (sysfs `/sys/class/drm/cardX/device/`)
//! - **Intel**  — via `lspci` e información de driver
//!
//! ## Añadir un nuevo comando
//!
//! Añade un [`MenuItem::new`] al `vec![]` dentro de [`create`]:
//!
//! ```rust
//! MenuItem::new(
//!     "Mi nuevo comando",        // nombre (≤ 32 chars)
//!     "Qué hace este comando",   // descripción (≤ 42 chars)
//!     || run_shell("comando"),   // ejecuta 'sh -c "comando"'
//! ),
//! ```

use super::{run_shell, ChartBar, MenuItem, SubMenu};

/// Construye y devuelve el submenú de GPU.
pub fn create() -> SubMenu {
    SubMenu::new(
        "GPU",
        "Tarjeta gráfica, VRAM y uso de GPU",
        vec![
            // ── Información ───────────────────────────────────────────────
            MenuItem::new(
                "GPUs detectadas",
                "Tarjetas gráficas instaladas (lspci)",
                || run_shell(
                    "lspci | grep -iE 'VGA|3D|Display' \
                     2>/dev/null || echo 'lspci no disponible; instala pciutils'",
                ),
            ),
            MenuItem::new(
                "Driver activo",
                "Driver en uso por cada GPU (lspci -k)",
                || run_shell(
                    "lspci -k 2>/dev/null | grep -A3 -iE 'VGA|3D|Display' \
                     || echo 'lspci no disponible'",
                ),
            ),
            // ── NVIDIA ────────────────────────────────────────────────────
            MenuItem::new(
                "Estado NVIDIA (nvidia-smi)",
                "Resumen completo de nvidia-smi",
                || run_shell(
                    "nvidia-smi 2>/dev/null \
                     || echo 'nvidia-smi no disponible\n\
                              (GPU no es NVIDIA o el driver no está instalado)'",
                ),
            ),
            MenuItem::new(
                "Procesos en GPU NVIDIA",
                "Procesos que usan la GPU NVIDIA",
                || run_shell(
                    "nvidia-smi --query-compute-apps=\
                         pid,process_name,used_gpu_memory \
                         --format=csv 2>/dev/null \
                     || nvidia-smi pmon -c 1 2>/dev/null \
                     || echo 'nvidia-smi no disponible'",
                ),
            ),
            // ── AMD ───────────────────────────────────────────────────────
            MenuItem::new(
                "Estado AMD (sysfs)",
                "Uso, VRAM y temperatura (driver amdgpu)",
                amd_status,
            ),
            // ── General ───────────────────────────────────────────────────
            MenuItem::new(
                "Temperatura GPU",
                "Temperatura vía sensors o sysfs",
                gpu_temp,
            ),
            MenuItem::new(
                "Dispositivos DRM",
                "Nodos de renderizado del kernel (/dev/dri)",
                || run_shell(
                    "ls -la /dev/dri/ 2>/dev/null \
                     || echo 'No se encontraron dispositivos DRM'",
                ),
            ),
            // ── Gráfico ───────────────────────────────────────────────────
            MenuItem::chart(
                "Uso de GPU (gráfico)",
                "Barras de utilización y VRAM por GPU",
                chart_gpu,
            ),
        ],
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// FUNCIONES DE COMANDOS
// ─────────────────────────────────────────────────────────────────────────────

/// Estado de GPUs AMD leyendo sysfs del driver `amdgpu`.
fn amd_status() -> String {
    let mut out = String::new();
    for i in 0..8u8 {
        let base = format!("/sys/class/drm/card{}/device", i);
        let busy_path = format!("{}/gpu_busy_percent", base);
        let Ok(pct_str) = std::fs::read_to_string(&busy_path) else {
            continue;
        };
        out.push_str(&format!("card{}  Uso GPU: {}%\n", i, pct_str.trim()));

        // VRAM
        let vram_used: u64 = std::fs::read_to_string(
            format!("{}/mem_info_vram_used", base),
        )
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);
        let vram_total: u64 = std::fs::read_to_string(
            format!("{}/mem_info_vram_total", base),
        )
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);
        if vram_total > 0 {
            let pct = vram_used as f64 / vram_total as f64 * 100.0;
            out.push_str(&format!(
                "       VRAM: {:.0} / {:.0} MiB  ({:.0}%)\n",
                vram_used as f64 / 1_048_576.0,
                vram_total as f64 / 1_048_576.0,
                pct,
            ));
        }

        // Temperatura (hwmon)
        let hwmon_base = format!("{}/hwmon", base);
        if let Ok(entries) = std::fs::read_dir(&hwmon_base) {
            for entry in entries.flatten() {
                let temp_path = entry.path().join("temp1_input");
                if let Ok(raw) = std::fs::read_to_string(&temp_path) {
                    let milli: i64 = raw.trim().parse().unwrap_or(0);
                    out.push_str(&format!(
                        "       Temperatura: {:.1} °C\n",
                        milli as f64 / 1000.0
                    ));
                    break;
                }
            }
        }
        out.push('\n');
    }

    if out.is_empty() {
        "No se detectaron GPUs AMD con driver amdgpu.\n\
         El driver amdgpu debe estar cargado para acceder a sysfs."
            .to_string()
    } else {
        out
    }
}

/// Temperatura de la GPU vía `sensors`, `nvidia-smi` o sysfs AMD.
fn gpu_temp() -> String {
    // 1. lm-sensors
    let s = run_shell(
        "sensors 2>/dev/null | grep -iE 'GPU|edge|junction|mem'",
    );
    if !s.trim().is_empty() {
        return format!("--- lm-sensors ---\n{}", s);
    }
    // 2. nvidia-smi
    let n = run_shell(
        "nvidia-smi --query-gpu=name,temperature.gpu \
             --format=csv,noheader 2>/dev/null",
    );
    if !n.trim().is_empty() {
        return format!("--- NVIDIA ---\n{}", n);
    }
    // 3. AMD sysfs hwmon
    let mut out = String::from("--- AMD sysfs ---\n");
    let mut found = false;
    for i in 0..8u8 {
        let hwmon_base = format!("/sys/class/drm/card{}/device/hwmon", i);
        if let Ok(entries) = std::fs::read_dir(&hwmon_base) {
            for entry in entries.flatten() {
                let temp_path = entry.path().join("temp1_input");
                if let Ok(raw) = std::fs::read_to_string(&temp_path) {
                    let milli: i64 = raw.trim().parse().unwrap_or(0);
                    out.push_str(&format!(
                        "card{}  {:.1} °C\n",
                        i,
                        milli as f64 / 1000.0
                    ));
                    found = true;
                    break;
                }
            }
        }
    }
    if found {
        out
    } else {
        "No se pudo leer la temperatura de la GPU.\n\
         Prueba: sudo apt install lm-sensors && sudo sensors-detect"
            .to_string()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GRÁFICO DE GPU
// ─────────────────────────────────────────────────────────────────────────────

/// Recopila datos de uso de GPU para la pantalla de gráficos.
///
/// Intenta en orden:
/// 1. **NVIDIA** — `nvidia-smi` (utilización y VRAM en MiB)
/// 2. **AMD**    — sysfs `amdgpu` (`gpu_busy_percent` + `mem_info_vram_*`)
///
/// Cada GPU genera dos barras: una de utilización (total = 100)
/// y otra de VRAM en bytes (para que `fmt_bytes` muestre unidades reales).
fn chart_gpu() -> Vec<ChartBar> {
    let mut bars: Vec<ChartBar> = Vec::new();

    // ── NVIDIA ────────────────────────────────────────────────────────────
    let nvidia = run_shell(
        "nvidia-smi \
             --query-gpu=name,utilization.gpu,memory.used,memory.total \
             --format=csv,noheader,nounits 2>/dev/null",
    );
    if !nvidia.trim().is_empty() {
        for line in nvidia.lines() {
            let parts: Vec<&str> = line.splitn(4, ',').map(str::trim).collect();
            if parts.len() < 4 {
                continue;
            }
            let name            = parts[0];
            let util: u64       = parts[1].parse().unwrap_or(0);
            let vram_used: u64  = parts[2].parse().unwrap_or(0);
            let vram_total: u64 = parts[3].parse().unwrap_or(1);

            bars.push(ChartBar {
                label: format!("{} (uso)", name),
                used:  util,
                total: 100,
            });
            bars.push(ChartBar {
                label: format!("{} (VRAM)", name),
                used:  vram_used * 1_048_576,   // MiB → bytes
                total: vram_total * 1_048_576,
            });
        }
        return bars;
    }

    // ── AMD sysfs ─────────────────────────────────────────────────────────
    for i in 0..8u8 {
        let base = format!("/sys/class/drm/card{}/device", i);
        let Ok(pct_str) = std::fs::read_to_string(format!("{}/gpu_busy_percent", base))
        else {
            continue;
        };
        let util: u64 = pct_str.trim().parse().unwrap_or(0);
        bars.push(ChartBar {
            label: format!("AMD card{} (uso)", i),
            used:  util,
            total: 100,
        });
        let vram_used: u64 = std::fs::read_to_string(
            format!("{}/mem_info_vram_used", base),
        )
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);
        let vram_total: u64 = std::fs::read_to_string(
            format!("{}/mem_info_vram_total", base),
        )
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);
        if vram_total > 0 {
            bars.push(ChartBar {
                label: format!("AMD card{} (VRAM)", i),
                used:  vram_used,
                total: vram_total,
            });
        }
    }

    if bars.is_empty() {
        bars.push(ChartBar {
            label: "GPU (sin datos — nvidia-smi o amdgpu requerido)".to_string(),
            used:  0,
            total: 100,
        });
    }

    bars
}
