//! # Submenú: Red
//!
//! Agrupa comandos para inspeccionar el estado de la red y el firewall.
//!
//! ## Añadir un nuevo comando de red
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

use super::{run_shell, MenuItem, SubMenu};

/// Construye y devuelve el submenú de gestión de red.
pub fn create() -> SubMenu {
    SubMenu::new(
        "Red",
        "Estado de red y firewall",
        vec![
            // ── Interfaces ────────────────────────────────────────────────
            MenuItem::new(
                "Interfaces de red",
                "Muestra interfaces y sus IPs (ip addr)",
                || run_shell("ip addr show"),
            ),
            // ── Puertos y conexiones ──────────────────────────────────────
            MenuItem::new(
                "Puertos en escucha",
                "Lista puertos TCP/UDP activos (ss -tulnp)",
                || run_shell("ss -tulnp"),
            ),
            MenuItem::new(
                "Conexiones establecidas",
                "Conexiones TCP activas (ss -tnp)",
                || run_shell("ss -tnp"),
            ),
            // ── Routing ───────────────────────────────────────────────────
            MenuItem::new(
                "Tabla de rutas",
                "Rutas de red activas (ip route show)",
                || run_shell("ip route show"),
            ),
            // ── Firewall ──────────────────────────────────────────────────
            MenuItem::new(
                "Firewall UFW",
                "Reglas activas de UFW (ufw status verbose)",
                || run_shell(
                    "ufw status verbose 2>/dev/null \
                     || echo 'UFW no disponible o no instalado'",
                ),
            ),
            MenuItem::new(
                "Reglas iptables",
                "Tabla filter de iptables (requiere root)",
                || run_shell(
                    "iptables -L -n --line-numbers 2>/dev/null \
                     || echo 'Se requieren permisos root (sudo)'",
                ),
            ),
            // ── DNS ───────────────────────────────────────────────────────
            MenuItem::new(
                "Configuración DNS",
                "Servidores DNS configurados (/etc/resolv.conf)",
                || run_shell("cat /etc/resolv.conf"),
            ),
            // ── Estadísticas ──────────────────────────────────────────────
            MenuItem::new(
                "Estadísticas de interfaces",
                "Contadores de paquetes/bytes (ip -s link)",
                || run_shell("ip -s link"),
            ),
            MenuItem::new(
                "ARP / vecinos",
                "Tabla ARP e IPv6 ND (ip neigh show)",
                || run_shell("ip neigh show"),
            ),
            // ── Módem (ModemManager / mmcli) ──────────────────────────────
            MenuItem::new(
                "Módems detectados",
                "Lista módems gestionados por ModemManager",
                || run_shell(
                    "mmcli -L 2>/dev/null \
                     || echo 'mmcli no disponible o ModemManager no está en ejecución\n\
                              Instala con: sudo apt install modemmanager'",
                ),
            ),
            MenuItem::with_input(
                "Estado del módem",
                "Info detallada del módem (mmcli -m <índice>)",
                "Índice del módem (0, 1, 2…):",
                "0",
                |idx| run_shell(&format!(
                    "mmcli -m {idx} 2>/dev/null \
                     || echo 'No se encontró módem con índice {idx}.\n\
                              Comprueba los disponibles con: mmcli -L'",
                )),
            ),
            MenuItem::with_input(
                "Estado de bearers",
                "Portadores de datos del módem (mmcli -m <índice>)",
                "Índice del módem (0, 1, 2…):",
                "0",
                |idx| run_shell(&format!(
                    "mmcli -m {idx} --bearers 2>/dev/null \
                     || echo 'Sin bearers activos o módem {idx} no encontrado'",
                )),
            ),
            MenuItem::with_input(
                "Estado SIM",
                "Información de la SIM del módem (mmcli -m <índice>)",
                "Índice del módem (0, 1, 2…):",
                "0",
                |idx| run_shell(&format!(
                    "sim=$(mmcli -m {idx} 2>/dev/null | awk '/sim/{{print $NF; exit}}'); \
                     if [ -n \"$sim\" ]; then mmcli -i \"$sim\" 2>/dev/null; \
                     else echo 'SIM no encontrada o módem {idx} no disponible'; fi",
                )),
            ),
            MenuItem::with_input(
                "Señal del módem",
                "Calidad de señal del módem (mmcli -m <índice>)",
                "Índice del módem (0, 1, 2…):",
                "0",
                |idx| run_shell(&format!(
                    "mmcli -m {idx} --signal-get 2>/dev/null \
                     || echo 'Sin datos de señal o módem {idx} no disponible'",
                )),
            ),
            MenuItem::new(
                "Estado ModemManager",
                "Estado del servicio ModemManager (systemctl)",
                || run_shell(
                    "systemctl status ModemManager 2>/dev/null \
                     || echo 'systemctl no disponible'",
                ),
            ),
        ],
    )
}
