//! # Submenú: Memoria
//!
//! Agrupa comandos para inspeccionar el uso y estado de la memoria RAM,
//! memoria virtual (swap) y caché del sistema.
//!
//! ## Añadir un nuevo comando de memoria
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

/// Construye y devuelve el submenú de gestión de memoria.
pub fn create() -> SubMenu {
    SubMenu::new(
        "Memoria",
        "Estado y uso de la memoria RAM del sistema",
        vec![
            // ── Resumen general ───────────────────────────────────────────
            MenuItem::new(
                "Resumen de memoria",
                "RAM y swap libre/usada (free -h)",
                || run_shell("free -h"),
            ),
            MenuItem::new(
                "Resumen detallado",
                "RAM en MB con columnas extendidas (free -m -w)",
                || run_shell("free -m -w"),
            ),
            // ── /proc/meminfo ─────────────────────────────────────────────
            MenuItem::new(
                "Info completa del kernel",
                "Todos los campos de memoria (/proc/meminfo)",
                || run_shell("cat /proc/meminfo"),
            ),
            MenuItem::new(
                "Memoria disponible",
                "RAM total, libre y disponible de forma rápida",
                || run_shell(
                    "awk '/MemTotal|MemFree|MemAvailable|Buffers|^Cached|SwapTotal|SwapFree/ \
                           {printf \"%-20s %s %s\\n\", $1, $2, $3}' /proc/meminfo",
                ),
            ),
            // ── Swap ──────────────────────────────────────────────────────
            MenuItem::new(
                "Estado del swap",
                "Particiones y archivos de intercambio (swapon -s)",
                || run_shell(
                    "swapon --show 2>/dev/null \
                     || echo '(Sin swap activo o swapon no disponible)'",
                ),
            ),
            // ── Procesos por consumo de RAM ───────────────────────────────
            MenuItem::new(
                "Top procesos por RAM",
                "20 procesos con mayor consumo de memoria",
                || run_shell("ps aux --sort=-%mem | head -21"),
            ),
            MenuItem::new(
                "Procesos con RSS > 100 MB",
                "Procesos que usan más de 100 MB de RAM",
                || run_shell(
                    r#"ps aux --sort=-%mem \
                       | awk 'NR==1 || $6 > 102400 {print}' \
                       | head -30"#,
                ),
            ),
            // ── Caché y huge pages ────────────────────────────────────────
            MenuItem::new(
                "Uso de caché del kernel",
                "Buffers, caché de página y slab (vmstat -s)",
                || run_shell("vmstat -s 2>/dev/null || sysctl vm.dirty_background_ratio"),
            ),
            MenuItem::new(
                "Huge Pages",
                "Estado de las páginas de memoria grandes",
                || run_shell(
                    "grep -i hugepage /proc/meminfo 2>/dev/null \
                     || echo 'Huge Pages no disponible'",
                ),
            ),
            // ── Estadísticas de vmstat ────────────────────────────────────
            MenuItem::new(
                "Actividad de memoria (vmstat)",
                "Estadísticas de paginación y swap (vmstat 1 5)",
                || run_shell("vmstat 1 5 2>/dev/null || echo 'vmstat no disponible'"),
            ),
            // ── Hardware ──────────────────────────────────────────────────
            MenuItem::new(
                "Módulos de RAM físicos",
                "Info de DIMMs instalados (dmidecode, requiere root)",
                || run_shell(
                    "dmidecode -t memory 2>/dev/null \
                       | grep -E 'Size|Speed|Type|Manufacturer|Part Number|Locator' \
                     || echo 'Se requieren permisos root o dmidecode no está instalado\n\
                              Instala con: sudo apt install dmidecode'",
                ),
            ),
            // ── Gráfico ───────────────────────────────────────────────────────────
            MenuItem::chart(
                "Memoria (gráfico)",
                "Barras de RAM usada, caché y swap",
                chart_memory,
            ),
        ],
    )
}

/// Recopila datos de uso de memoria para la pantalla de gráficos.
///
/// Lee `/proc/meminfo` y devuelve tres [`ChartBar`]:
/// - **RAM usada**     = MemTotal − MemAvailable
/// - **Caché + Buffers** = Buffers + Cached (solo si > 0)
/// - **Swap**          = SwapTotal − SwapFree (solo si SwapTotal > 0)
fn chart_memory() -> Vec<ChartBar> {
    let raw = run_shell("cat /proc/meminfo");

    let mut mem_total     = 0u64;
    let mut mem_available = 0u64;
    let mut swap_total    = 0u64;
    let mut swap_free     = 0u64;
    let mut buffers       = 0u64;
    let mut cached        = 0u64;

    for line in raw.lines() {
        let mut parts = line.split_whitespace();
        let key = parts.next().unwrap_or("");
        // Los valores de /proc/meminfo están en kibibytes
        let val: u64 = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0) * 1024;
        match key {
            "MemTotal:"     => mem_total     = val,
            "MemAvailable:" => mem_available = val,
            "SwapTotal:"    => swap_total    = val,
            "SwapFree:"     => swap_free     = val,
            "Buffers:"      => buffers       = val,
            "Cached:"       => cached        = val,
            _               => {}
        }
    }

    let mut bars = Vec::new();

    if mem_total > 0 {
        bars.push(ChartBar {
            label: "RAM (usada)".to_string(),
            used:  mem_total.saturating_sub(mem_available),
            total: mem_total,
        });
        let cache_used = buffers.saturating_add(cached);
        if cache_used > 0 {
            bars.push(ChartBar {
                label: "Caché + Buffers".to_string(),
                used:  cache_used,
                total: mem_total,
            });
        }
    }

    if swap_total > 0 {
        bars.push(ChartBar {
            label: "Swap".to_string(),
            used:  swap_total.saturating_sub(swap_free),
            total: swap_total,
        });
    }

    bars
}

