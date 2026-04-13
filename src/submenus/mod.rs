//! # Sistema de submenús
//!
//! Este módulo define los tipos centrales del sistema de menús y el **registro**
//! donde se declaran todos los submenús activos.
//!
//! ## Tipos principales
//!
//! - [`MenuItem`] — un comando ejecutable: nombre, descripción y función.
//! - [`SubMenu`]  — agrupación de `MenuItem`s con nombre y descripción propia.
//!
//! ## Añadir un nuevo submenú (guía rápida)
//!
//! ```text
//! 1. Crea src/submenus/mi_modulo.rs
//!    └── implementa: pub fn create() -> SubMenu { … }
//!
//! 2. Declara el módulo aquí:
//!    pub mod mi_modulo;
//!
//! 3. Regístralo en get_submenus():
//!    vec![ …, mi_modulo::create() ]
//! ```
//!
//! Consulta `network.rs` como plantilla completa.

// ── Módulos de submenús ───────────────────────────────────────────────────────
// Declara aquí cada nuevo módulo de submenú que crees.
pub mod containers;
pub mod cpu;
pub mod gpu;
pub mod memory;
pub mod network;
pub mod storage;
pub mod users;

// ─────────────────────────────────────────────────────────────────────────────
// TIPOS
// ─────────────────────────────────────────────────────────────────────────────

// ── Tipo de comando ───────────────────────────────────────────────────────────

/// Barra de datos para la pantalla de gráficos.
///
/// Representa una unidad medible (p.ej. un disco, la RAM, el swap) con su
/// cantidad usada y su capacidad total, ambas expresadas en **bytes**.
///
/// La pantalla de gráficos calcula automáticamente el porcentaje
/// `used * 100 / total` y elige el color del indicador:
/// verde (< 70 %), amarillo (70–90 %), rojo (≥ 90 %).
#[derive(Clone)]
pub struct ChartBar {
    /// Etiqueta visible (nombre del sistema de archivos, "RAM", "Swap", etc.).
    pub label: String,
    /// Bytes usados actualmente.
    pub used: u64,
    /// Capacidad total en bytes.
    pub total: u64,
}

/// Tipo de comando asociado a un [`MenuItem`].
///
/// Permite diferenciar entre comandos que se ejecutan directamente,
/// comandos que requieren un parámetro y comandos que muestran gráficos.
///
/// ## Cómo elegir la variante
///
/// - [`CommandKind::Direct`]    → ejecución inmediata, muestra texto.
/// - [`CommandKind::WithInput`] → pide un parámetro al usuario antes de ejecutar.
/// - [`CommandKind::Chart`]     → muestra una pantalla de barras gráficas.
///
/// Todos los campos de tipo `fn` son punteros de función (implementan `Copy`).
pub enum CommandKind {
    /// Comando que se ejecuta directamente al pulsar Enter.
    Direct(fn() -> String),

    /// Comando que solicita al usuario un parámetro de texto antes de ejecutarse.
    WithInput {
        /// Texto de la etiqueta mostrada al usuario en el campo de entrada.
        prompt: &'static str,
        /// Valor predeterminado pre-rellenado en el campo de entrada.
        default: &'static str,
        /// Función que recibe la entrada del usuario y devuelve la salida del comando.
        execute: fn(&str) -> String,
    },

    /// Comando que produce datos estructurados para mostrar como barras gráficas.
    Chart(fn() -> Vec<ChartBar>),
}

/// Entrada individual dentro de un submenú.
///
/// Encapsula el nombre visible, una descripción corta y el tipo de comando
/// a ejecutar cuando el usuario pulsa Enter.
pub struct MenuItem {
    /// Nombre corto visible en la lista (≤ 32 caracteres recomendado).
    pub name: &'static str,
    /// Descripción breve visible junto al nombre (≤ 42 caracteres recomendado).
    pub description: &'static str,
    /// Tipo de comando: directo o con entrada de usuario.
    pub command: CommandKind,
}

impl MenuItem {
    /// Crea un [`MenuItem`] de ejecución directa (sin entrada de usuario).
    ///
    /// # Ejemplo
    ///
    /// ```rust
    /// MenuItem::new(
    ///     "Interfaces de red",
    ///     "Muestra IPs activas (ip addr)",
    ///     || run_shell("ip addr show"),
    /// )
    /// ```
    pub fn new(
        name: &'static str,
        description: &'static str,
        command: fn() -> String,
    ) -> Self {
        Self { name, description, command: CommandKind::Direct(command) }
    }

    /// Crea un [`MenuItem`] que solicita un parámetro al usuario antes de ejecutarse.
    ///
    /// # Argumentos
    ///
    /// - `prompt`  — etiqueta del campo de entrada mostrada al usuario.
    /// - `default` — valor predeterminado pre-rellenado en el campo.
    /// - `execute` — función que recibe la entrada y devuelve la salida del comando.
    ///
    /// # Ejemplo
    ///
    /// ```rust
    /// MenuItem::with_input(
    ///     "Estado del módem",
    ///     "mmcli -m <índice>",
    ///     "Índice del módem (0, 1, 2…):",
    ///     "0",
    ///     |idx| run_shell(&format!("mmcli -m {idx}")),
    /// )
    /// ```
    pub fn with_input(
        name: &'static str,
        description: &'static str,
        prompt: &'static str,
        default: &'static str,
        execute: fn(&str) -> String,
    ) -> Self {
        Self {
            name,
            description,
            command: CommandKind::WithInput { prompt, default, execute },
        }
    }

    /// Crea un [`MenuItem`] que muestra una pantalla de barras gráficas.
    ///
    /// La función `data` debe devolver un `Vec<ChartBar>` donde cada elemento
    /// representa una unidad medible (disco, RAM, etc.) con `used` y `total`
    /// expresados en **bytes**.
    ///
    /// # Ejemplo
    ///
    /// ```rust
    /// MenuItem::chart(
    ///     "Uso de disco (gráfico)",
    ///     "Barras de ocupación por sistema de archivos",
    ///     chart_disk_usage,
    /// )
    /// ```
    pub fn chart(
        name: &'static str,
        description: &'static str,
        data: fn() -> Vec<ChartBar>,
    ) -> Self {
        Self { name, description, command: CommandKind::Chart(data) }
    }
}

// ─────────────────────────────────────────────────────────────────────────────

/// Agrupación de comandos relacionados que aparece en el menú principal.
///
/// Para crear un nuevo submenú implementa `pub fn create() -> SubMenu` en un
/// módulo propio y regístralo en [`get_submenus`].
pub struct SubMenu {
    /// Nombre del submenú que aparece en el menú principal.
    pub name: &'static str,
    /// Descripción corta visible junto al nombre en el menú principal.
    pub description: &'static str,
    /// Lista ordenada de comandos disponibles en este submenú.
    pub items: Vec<MenuItem>,
}

impl SubMenu {
    /// Crea un nuevo [`SubMenu`].
    pub fn new(name: &'static str, description: &'static str, items: Vec<MenuItem>) -> Self {
        Self { name, description, items }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// REGISTRO DE SUBMENÚS
// ─────────────────────────────────────────────────────────────────────────────
//
// ┌─────────────────────────────────────────────────────────────────────────┐
// │  Para añadir un nuevo submenú:                                          │
// │  1. Declara `pub mod <nombre>;` en la sección de módulos (arriba).      │
// │  2. Añade `<nombre>::create()` al Vec de esta función.                  │
// │  El orden del Vec determina el orden en pantalla.                       │
// └─────────────────────────────────────────────────────────────────────────┘

/// Devuelve todos los submenús registrados en la aplicación.
///
/// Esta es la única función que hay que modificar cuando se añade o elimina
/// un submenú.
pub fn get_submenus() -> Vec<SubMenu> {
    vec![
        network::create(),      // Red, firewall y módem
        users::create(),        // Usuarios y sesiones
        storage::create(),      // Almacenamiento y discos
        memory::create(),       // Memoria RAM y swap
        cpu::create(),          // CPU, núcleos y temperatura
        gpu::create(),          // GPU, VRAM y rendimiento
        containers::create(),   // KVM, Docker/Podman, Virtualización
        // ← Añade aquí los nuevos submenús
    ]
}

// ─────────────────────────────────────────────────────────────────────────────
// UTILIDADES COMPARTIDAS DE EJECUCIÓN
// ─────────────────────────────────────────────────────────────────────────────

/// Ejecuta una expresión de shell con `sh -c` y devuelve su salida como `String`.
///
/// - La salida estándar (stdout) tiene prioridad.
/// - Si stdout está vacío, se devuelve stderr con prefijo `[stderr]`.
/// - Los códigos de escape ANSI se eliminan antes de devolver el texto.
/// - Los errores de invocación se devuelven como mensaje legible.
///
/// # Nota de seguridad
///
/// Usa esta función únicamente con comandos de **solo lectura**.
/// Nunca construyas `cmd` con datos introducidos por el usuario.
pub fn run_shell(cmd: &str) -> String {
    use std::process::Command;

    match Command::new("sh").args(["-c", cmd]).output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);

            let raw = if !stdout.trim().is_empty() {
                stdout.to_string()
            } else if !stderr.trim().is_empty() {
                format!("[stderr]:\n{}", stderr)
            } else {
                "(Sin salida)".to_string()
            };

            strip_ansi(&raw)
        }
        Err(e) => format!("Error al ejecutar el comando: {e}"),
    }
}

/// Elimina secuencias de escape ANSI de una cadena de texto.
///
/// Soporta las secuencias CSI más comunes (`ESC [ … m`, `ESC [ … A/B/C/D`, etc.).
/// El resultado es texto plano apto para ser renderizado por ratatui.
fn strip_ansi(s: &str) -> String {
    let mut out    = String::with_capacity(s.len());
    let mut escape = false;
    let mut bracket = false;

    for ch in s.chars() {
        if escape {
            if ch == '[' {
                bracket = true;
            } else if bracket && ch.is_ascii_alphabetic() {
                // Fin de secuencia CSI
                escape  = false;
                bracket = false;
            } else if !bracket {
                // Secuencia de escape sin '[', descartar el carácter terminator
                escape = false;
            }
            // En medio de una secuencia: seguir descartando
        } else if ch == '\x1b' {
            escape = true;
        } else {
            out.push(ch);
        }
    }

    out
}
