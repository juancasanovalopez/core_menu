//! # Submenú: Usuarios
//!
//! Agrupa comandos para inspeccionar usuarios, grupos y sesiones del sistema.
//!
//! ## Añadir un nuevo comando de usuarios
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

/// Construye y devuelve el submenú de gestión de usuarios.
pub fn create() -> SubMenu {
    SubMenu::new(
        "Usuarios",
        "Usuarios y sesiones del sistema",
        vec![
            // ── Sesiones activas ──────────────────────────────────────────
            MenuItem::new(
                "Usuarios conectados",
                "Sesiones activas en el sistema (who -a)",
                || run_shell("who -a"),
            ),
            MenuItem::new(
                "Actividad actual",
                "Usuarios, tiempo y procesos activos (w)",
                || run_shell("w"),
            ),
            // ── Historial ─────────────────────────────────────────────────
            MenuItem::new(
                "Últimos accesos",
                "Historial de login (last -n 30)",
                || run_shell("last -n 30"),
            ),
            MenuItem::new(
                "Intentos fallidos",
                "Accesos fallidos (lastb, requiere root)",
                || run_shell(
                    "lastb -n 20 2>/dev/null \
                     || echo 'Se requieren permisos root (sudo)'",
                ),
            ),
            // ── Cuentas del sistema ───────────────────────────────────────
            MenuItem::new(
                "Usuarios con shell de login",
                "Cuentas que pueden iniciar sesión",
                || run_shell(
                    r#"grep -v -E '/usr/sbin/nologin|/bin/false|/sbin/nologin' /etc/passwd \
                       | column -t -s ':'"#,
                ),
            ),
            MenuItem::new(
                "Todos los usuarios",
                "Listado completo de /etc/passwd",
                || run_shell("column -t -s ':' /etc/passwd"),
            ),
            // ── Grupos ────────────────────────────────────────────────────
            MenuItem::new(
                "Grupos del sistema",
                "Listado de /etc/group",
                || run_shell("column -t -s ':' /etc/group"),
            ),
            // ── Identidad actual ──────────────────────────────────────────
            MenuItem::new(
                "Identidad actual",
                "UID, GID y grupos del proceso actual (id)",
                || run_shell("id && echo '' && groups"),
            ),
            // ── Sudo ──────────────────────────────────────────────────────
            MenuItem::new(
                "Privilegios sudo",
                "Comandos permitidos al usuario (sudo -l)",
                || run_shell(
                    "sudo -l 2>/dev/null \
                     || echo 'Sin acceso sudo o sudo no configurado'",
                ),
            ),
        ],
    )
}
