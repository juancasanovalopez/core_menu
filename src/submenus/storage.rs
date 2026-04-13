//! # Submenú: Almacenamiento
//!
//! Agrupa comandos para inspeccionar discos, particiones, sistemas de archivos
//! y uso de espacio en el sistema.
//!
//! ## Añadir un nuevo comando de almacenamiento
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

/// Construye y devuelve el submenú de gestión de almacenamiento.
pub fn create() -> SubMenu {
    SubMenu::new(
        "Almacenamiento",
        "Discos, particiones y uso de espacio",
        vec![
            // ── Espacio en disco ──────────────────────────────────────────
            MenuItem::new(
                "Uso de disco",
                "Espacio libre/usado por sistema de archivos (df -h)",
                || run_shell("df -h"),
            ),
            MenuItem::new(
                "Uso de inodos",
                "Disponibilidad de inodos (df -i)",
                || run_shell("df -i"),
            ),
            // ── Dispositivos ──────────────────────────────────────────────
            MenuItem::new(
                "Dispositivos de bloque",
                "Discos y particiones con detalles (lsblk)",
                || run_shell(
                    "lsblk -o NAME,SIZE,TYPE,FSTYPE,MOUNTPOINT,LABEL",
                ),
            ),
            MenuItem::new(
                "Info de discos físicos",
                "Modelo, tamaño y tipo de transporte",
                || run_shell(
                    "lsblk -d -o NAME,SIZE,ROTA,TRAN,MODEL 2>/dev/null",
                ),
            ),
            // ── Montajes ──────────────────────────────────────────────────
            MenuItem::new(
                "Puntos de montaje",
                "Sistemas de archivos actualmente montados",
                || run_shell("mount | column -t"),
            ),
            // ── Uso por directorio ────────────────────────────────────────
            MenuItem::new(
                "Directorios más grandes",
                "Top 20 dirs por tamaño (du, puede tardar)",
                || run_shell(
                    "du -hx --max-depth=2 / 2>/dev/null \
                     | sort -rh | head -20",
                ),
            ),
            MenuItem::new(
                "Uso del directorio actual",
                "Tamaño de cada entrada en $PWD (du -sh *)",
                || run_shell("du -sh -- * 2>/dev/null | sort -rh"),
            ),
            // ── Salud ─────────────────────────────────────────────────────
            MenuItem::new(
                "Salud SMART de discos",
                "Estado SMART (requiere smartmontools)",
                || run_shell(
                    r#"if command -v smartctl >/dev/null 2>&1; then
                         smartctl --scan 2>/dev/null \
                           | awk '{print $1}' \
                           | while read -r dev; do
                               echo "=== $dev ==="; \
                               smartctl -H "$dev" 2>/dev/null; \
                             done
                       else
                         echo 'smartmontools no está instalado'
                         echo 'Instala con: sudo apt install smartmontools'
                       fi"#,
                ),
            ),
            // ── Cuotas ────────────────────────────────────────────────────
            MenuItem::new(
                "Cuota de usuario",
                "Uso de cuota del usuario actual (quota)",
                || run_shell(
                    "quota -s 2>/dev/null \
                     || echo 'Sin sistema de cuotas configurado'",
                ),
            ),
            // ── Gráfico ───────────────────────────────────────────────────────────
            MenuItem::chart(
                "Uso de disco (gráfico)",
                "Barras ocupado/disponible por sistema de archivos",
                chart_disk_usage,
            ),
        ],
    )
}

/// Recopila datos de uso de disco para la pantalla de gráficos.
///
/// Ejecuta `df -B1 -P` (formato POSIX, tamaños en bytes) y devuelve una
/// [`ChartBar`] por cada sistema de archivos real (se filtran `tmpfs`,
/// `devtmpfs`, `squashfs`, `efivarfs` y `overlay`).
fn chart_disk_usage() -> Vec<ChartBar> {
    let raw  = run_shell("df -B1 -P 2>/dev/null");
    let skip = ["tmpfs", "devtmpfs", "squashfs", "efivarfs", "overlay", "udev"];

    raw.lines()
        .skip(1) // saltar cabecera
        .filter_map(|line| {
            let cols: Vec<&str> = line.split_whitespace().collect();
            if cols.len() < 6 { return None; }
            let fs    = cols[0];
            let total: u64 = cols[1].parse().ok()?;
            let used:  u64 = cols[2].parse().ok()?;
            let mount = cols[5].to_string();
            // Filtrar seudo-sistemas de archivos y tamaños triviales (< 1 MiB)
            if skip.iter().any(|s| fs.starts_with(s)) { return None; }
            if total < 1 << 20 { return None; }
            Some(ChartBar { label: mount, used, total })
        })
        .collect()
}

