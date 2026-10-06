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

fn main() {
    if let Err(e) = app::run() {
        eprintln!("Error fatal: {e}");
        std::process::exit(1);
    }
}
