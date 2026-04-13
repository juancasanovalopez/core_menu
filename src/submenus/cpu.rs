//! # Submenú: CPU
//!
//! Agrupa comandos para inspeccionar el estado del procesador,
//! rendimiento, temperatura y uso por núcleo.
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

/// Construye y devuelve el submenú de CPU.
pub fn create() -> SubMenu {
    SubMenu::new(
        "CPU",
        "Procesador, núcleos, carga y temperatura",
        vec![
            // ── Información ───────────────────────────────────────────────
            MenuItem::new(
                "Información del procesador",
                "Modelo, núcleos y frecuencias (lscpu)",
                || run_shell("lscpu"),
            ),
            MenuItem::new(
                "Carga del sistema",
                "Promedios de carga 1, 5 y 15 min (uptime)",
                || run_shell("uptime && echo '' && cat /proc/loadavg"),
            ),
            // ── Procesos ──────────────────────────────────────────────────
            MenuItem::new(
                "Procesos activos (CPU)",
                "Top 20 procesos ordenados por uso de CPU",
                || run_shell("ps aux --sort=-%cpu | head -21"),
            ),
            MenuItem::new(
                "Estadísticas de CPU (vmstat)",
                "Actividad de CPU en 5 muestras de 1 s",
                || run_shell("vmstat 1 5 2>/dev/null || echo 'vmstat no disponible'"),
            ),
            // ── Frecuencia ────────────────────────────────────────────────
            MenuItem::new(
                "Frecuencia por núcleo",
                "Frecuencia actual de cada núcleo (MHz)",
                cpu_freq,
            ),
            // ── Temperatura ───────────────────────────────────────────────
            MenuItem::new(
                "Temperatura de la CPU",
                "Sensores de temperatura (sensors / sysfs)",
                cpu_temp,
            ),
            // ── Hardware ──────────────────────────────────────────────────
            MenuItem::new(
                "Caché del procesador",
                "Tamaños de L1, L2 y L3 (lscpu -C)",
                || run_shell(
                    "lscpu -C 2>/dev/null \
                     || lscpu | grep -i cache",
                ),
            ),
            MenuItem::new(
                "Interrupciones por núcleo",
                "Resumen de /proc/interrupts (primeras 30 líneas)",
                || run_shell("cat /proc/interrupts | head -30"),
            ),
            // ── Gráfico ───────────────────────────────────────────────────
            MenuItem::chart(
                "Uso por núcleo (gráfico)",
                "Barras de uso de CPU por núcleo (250 ms muestra)",
                chart_cpu_cores,
            ),
        ],
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// FUNCIONES DE COMANDOS
// ─────────────────────────────────────────────────────────────────────────────

/// Frecuencia actual de cada núcleo leyendo sysfs o haciendo fallback a lscpu.
fn cpu_freq() -> String {
    run_shell(
        r#"found=0; \
           for f in /sys/devices/system/cpu/cpu[0-9]*/cpufreq/scaling_cur_freq; do \
               [ -f "$f" ] || continue; \
               core=$(echo "$f" | sed 's|.*/cpu||;s|/.*||'); \
               freq=$(cat "$f" 2>/dev/null); \
               [ -n "$freq" ] && printf "Núcleo %-3s  %d MHz\n" "$core" "$((freq / 1000))"; \
               found=1; \
           done; \
           [ "$found" -eq 0 ] && lscpu | grep -iE 'MHz|GHz|Freq' || true"#,
    )
}

/// Temperatura de la CPU vía `sensors` (lm-sensors) o sysfs como fallback.
fn cpu_temp() -> String {
    let s = run_shell(
        "sensors 2>/dev/null | grep -iE 'Core|Package|Tdie|Tctl|edge|temp'",
    );
    if !s.trim().is_empty() {
        return s;
    }
    run_shell(
        r#"found=0; \
           for d in /sys/class/thermal/thermal_zone*/; do \
               type=$(cat "${d}type" 2>/dev/null) || continue; \
               temp=$(cat "${d}temp" 2>/dev/null) || continue; \
               awk -v t="$type" -v v="$temp" \
                   'BEGIN{printf "%-24s %.1f °C\n", t":", v/1000}'; \
               found=1; \
           done; \
           [ "$found" -eq 0 ] && echo \
               'Sin datos de temperatura.\nInstala lm-sensors: sudo apt install lm-sensors && sudo sensors-detect' \
           || true"#,
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// GRÁFICO DE NÚCLEOS
// ─────────────────────────────────────────────────────────────────────────────

/// Snapshot de uso de un núcleo leído de `/proc/stat`.
struct CpuSnap {
    name:  String,
    total: u64,
    idle:  u64,
}

/// Lee el estado actual de todos los núcleos desde `/proc/stat`.
///
/// Omite la línea `cpu ` (agregado) y devuelve un [`CpuSnap`] por núcleo.
fn read_cpu_stat() -> Vec<CpuSnap> {
    let raw = std::fs::read_to_string("/proc/stat").unwrap_or_default();
    let mut snaps = Vec::new();
    for line in raw.lines() {
        if !line.starts_with("cpu") {
            break;
        }
        let mut parts = line.split_whitespace();
        let name_raw = match parts.next() {
            Some(n) => n,
            None => continue,
        };
        if name_raw == "cpu" {
            continue; // saltar línea de totales
        }
        let fields: Vec<u64> = parts.map(|v| v.parse().unwrap_or(0)).collect();
        if fields.len() < 4 {
            continue;
        }
        let user    = fields[0];
        let nice    = fields[1];
        let system  = fields[2];
        let idle    = fields[3];
        let iowait  = fields.get(4).copied().unwrap_or(0);
        let irq     = fields.get(5).copied().unwrap_or(0);
        let softirq = fields.get(6).copied().unwrap_or(0);
        let steal   = fields.get(7).copied().unwrap_or(0);
        let total   = user + nice + system + idle + iowait + irq + softirq + steal;
        let num     = name_raw.trim_start_matches("cpu");
        snaps.push(CpuSnap {
            name:  format!("Núcleo {}", num),
            total,
            idle: idle + iowait,
        });
    }
    snaps
}

/// Mide el uso de CPU por núcleo comparando dos snapshots de `/proc/stat`
/// separados 250 ms. Devuelve un [`ChartBar`] por núcleo con `used ∈ [0,100]`
/// y `total = 100` (porcentaje).
fn chart_cpu_cores() -> Vec<ChartBar> {
    let snap1 = read_cpu_stat();
    std::thread::sleep(std::time::Duration::from_millis(250));
    let snap2 = read_cpu_stat();

    snap1
        .iter()
        .zip(snap2.iter())
        .map(|(s1, s2)| {
            let delta_total = s2.total.saturating_sub(s1.total);
            let delta_idle  = s2.idle.saturating_sub(s1.idle);
            let used_pct = if delta_total > 0 {
                (delta_total - delta_idle) * 100 / delta_total
            } else {
                0
            };
            ChartBar {
                label: s1.name.clone(),
                used:  used_pct,
                total: 100,
            }
        })
        .collect()
}
