//! # core_menu — Gestor de Sistema Linux (TUI)
//!
//! Aplicación de menú interactivo para administrar recursos del sistema Linux
//! desde cualquier terminal compatible. Construida con [`ratatui`] + [`crossterm`].
//!
//! ## Atajos de teclado globales
//!
//! | Tecla          | Acción                              |
//! |----------------|-------------------------------------|
//! | ↑ / k          | Mover cursor arriba                 |
//! | ↓ / j          | Mover cursor abajo                  |
//! | Enter          | Seleccionar / Ejecutar              |
//! | Esc / q        | Volver al nivel anterior            |
//! | Ctrl+C         | Salir inmediatamente                |
//!
//! ---
//!
//! ## Cómo añadir un **nuevo submenú**
//!
//! 1. Crea el archivo `src/submenus/<nombre>.rs` e implementa la función:
//!    ```rust
//!    pub fn create() -> SubMenu { ... }
//!    ```
//!    Puedes copiar `src/submenus/network.rs` como plantilla.
//!
//! 2. Declara el módulo en `src/submenus/mod.rs`:
//!    ```rust
//!    pub mod <nombre>;
//!    ```
//!
//! 3. Registra el submenú en la función `get_submenus()` de `src/submenus/mod.rs`:
//!    ```rust
//!    pub fn get_submenus() -> Vec<SubMenu> {
//!        vec![
//!            network::create(),
//!            users::create(),
//!            storage::create(),
//!            <nombre>::create(),   // ← añade aquí
//!        ]
//!    }
//!    ```
//!
//! ## Cómo añadir un **nuevo comando** a un submenú existente
//!
//! En el archivo del submenú correspondiente, añade un `MenuItem::new(...)` al `vec![]`:
//!
//! ```rust
//! MenuItem::new(
//!     "Nombre visible",           // ≤ 30 caracteres recomendado
//!     "Descripción corta",        // ≤ 40 caracteres recomendado
//!     || run_shell("tu_comando"), // función que devuelve String
//! ),
//! ```

mod app;
mod hardware;
mod submenus;
mod ui;
mod update;

fn main() {
    if let Err(e) = start() {
        eprintln!("Error fatal: {e}");
        std::process::exit(1);
    }
}

fn start() -> anyhow::Result<()> {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    match arguments.as_slice() {
        [] => app::run(),
        [flag] if flag == "--version" => {
            println!("core_menu {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        [flag] if flag == "--help" || flag == "-h" => {
            println!("core_menu [--version | --help]\nSin argumentos: menú y comprobación de actualizaciones.");
            Ok(())
        }
        [flag] if flag == "--internal-update-check" => {
            serde_json::to_writer(std::io::stdout(), &update::check()?)?;
            Ok(())
        }
        [flag] if flag == "--internal-install-update" => update::install::helper(),
        _ => anyhow::bail!("Argumentos no válidos; usa --help"),
    }
}
