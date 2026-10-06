//! Estado de la aplicación y bucle principal de eventos.
//!
//! Este módulo gestiona:
//! - El estado global ([`App`])
//! - Los modos de visualización ([`AppMode`])
//! - Las transiciones entre pantallas
//! - La inicialización y restauración del terminal

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::{Duration, Instant};

use crate::hardware::HardwareInfo;
use crate::submenus::{get_submenus, ChartBar, CommandKind, SubMenu};

// ─────────────────────────────────────────────────────────────────────────────
// MODO DE VISUALIZACIÓN
// ─────────────────────────────────────────────────────────────────────────────

/// Representa la pantalla activa de la aplicación.
///
/// La máquina de estados sigue el flujo:
/// `MainMenu` → `SubMenu(idx)` → `Output { … }` → `SubMenu(idx)` → `MainMenu`
#[derive(Clone)]
pub enum AppMode {
    /// Pantalla inicial: lista de submenús disponibles.
    MainMenu,

    /// Submenú expandido. El `usize` es el índice en [`App::submenus`].
    SubMenu(usize),

    /// Muestra la salida de un comando ejecutado.
    Output {
        /// Índice del submenú padre (para volver a él con Esc).
        submenu_idx: usize,
        /// Título visible en la cabecera del bloque de salida.
        title: String,
        /// Texto completo devuelto por el comando.
        content: String,
    },

    /// Solicita al usuario un parámetro de texto antes de ejecutar el comando.
    ///
    /// Transición: `SubMenu(idx)` → `InputPrompt { … }` → `Output { … }`
    InputPrompt {
        /// Índice del submenú padre.
        submenu_idx: usize,
        /// Índice del item dentro del submenú.
        item_idx: usize,
        /// Texto de la etiqueta mostrada al usuario.
        prompt: &'static str,
        /// Texto introducido hasta el momento.
        buffer: String,
    },

    /// Muestra una pantalla de barras gráficas (ocupado/disponible).
    ///
    /// Transición: `SubMenu(idx)` → `Chart { … }` → `SubMenu(idx)`
    Chart {
        /// Índice del submenú padre (para volver a él con Esc).
        submenu_idx: usize,
        /// Título visible en la cabecera de la pantalla.
        title: String,
        /// Datos de las barras a renderizar.
        bars: Vec<ChartBar>,
    },
}

// ─────────────────────────────────────────────────────────────────────────────
// ESTADO GLOBAL
// ─────────────────────────────────────────────────────────────────────────────

/// Estado global de la aplicación que se pasa por referencia a cada render.
pub struct App {
    /// Pantalla actualmente visible.
    pub mode: AppMode,
    /// Posición del cursor en el menú principal.
    pub main_cursor: usize,
    /// Posición del cursor dentro del submenú activo.
    pub sub_cursor: usize,
    /// Desplazamiento vertical en la pantalla de salida de un comando.
    pub output_scroll: u16,
    /// Lista de submenús registrados (orden = orden en pantalla).
    pub submenus: Vec<SubMenu>,
    pub hardware: HardwareInfo,
    hardware_updated_at: Instant,
}

impl App {
    /// Construye el estado inicial de la aplicación.
    pub fn new() -> Self {
        Self {
            mode:          AppMode::MainMenu,
            main_cursor:   0,
            sub_cursor:    0,
            output_scroll: 0,
            submenus:      get_submenus(),
            hardware:      HardwareInfo::read(),
            hardware_updated_at: Instant::now(),
        }
    }

    /// Número de entradas visibles en el menú principal (submenús + "Salir").
    pub fn main_menu_len(&self) -> usize {
        self.submenus.len() + 1 // +1 = entrada "Salir"
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GESTIÓN DE EVENTOS POR MODO
// ─────────────────────────────────────────────────────────────────────────────

impl App {
    /// Procesa teclas en el **menú principal**.
    ///
    /// Devuelve `true` cuando la aplicación debe cerrarse.
    pub fn handle_main_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> bool {
        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.main_cursor > 0 {
                    self.main_cursor -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.main_cursor < self.main_menu_len() - 1 {
                    self.main_cursor += 1;
                }
            }
            KeyCode::Enter => {
                if self.main_cursor >= self.submenus.len() {
                    return true; // "Salir" seleccionado
                }
                self.sub_cursor = 0;
                self.mode = AppMode::SubMenu(self.main_cursor);
            }
            KeyCode::Char('q') | KeyCode::Char('Q') => return true,
            _ => {
                // Ctrl+C se maneja en el bucle principal antes de llegar aquí
                let _ = modifiers;
            }
        }
        false
    }

    /// Procesa teclas dentro de un **submenú**.
    pub fn handle_submenu_key(&mut self, code: KeyCode, _modifiers: KeyModifiers, idx: usize) {
        let items_len = self.submenus[idx].items.len();

        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.sub_cursor > 0 {
                    self.sub_cursor -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.sub_cursor < items_len.saturating_sub(1) {
                    self.sub_cursor += 1;
                }
            }
            KeyCode::Enter => {
                let item_idx = self.sub_cursor;
                let name = self.submenus[idx].items[item_idx].name.to_string();
                // Usar un enum local para evitar conflictos de préstamo entre
                // la lectura de submenus[] y la escritura de self.mode
                enum Action {
                    Output(String),
                    Input(&'static str, String),
                    Chart(Vec<ChartBar>),
                }
                let action = match &self.submenus[idx].items[item_idx].command {
                    CommandKind::Direct(f) => Action::Output(f()),
                    CommandKind::WithInput { prompt, default, .. } => {
                        Action::Input(*prompt, default.to_string())
                    }
                    CommandKind::Chart(f) => Action::Chart(f()),
                };
                self.output_scroll = 0;
                self.mode = match action {
                    Action::Output(content) => {
                        AppMode::Output { submenu_idx: idx, title: name, content }
                    }
                    Action::Input(prompt, buffer) => {
                        AppMode::InputPrompt { submenu_idx: idx, item_idx, prompt, buffer }
                    }
                    Action::Chart(bars) => {
                        AppMode::Chart { submenu_idx: idx, title: name, bars }
                    }
                };
            }
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Backspace => {
                self.mode = AppMode::MainMenu;
            }
            _ => {}
        }
    }

    /// Procesa teclas en la **pantalla de salida** de un comando.
    pub fn handle_output_key(&mut self, code: KeyCode, _modifiers: KeyModifiers, submenu_idx: usize) {
        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.output_scroll = self.output_scroll.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.output_scroll = self.output_scroll.saturating_add(1);
            }
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') | KeyCode::Backspace => {
                self.output_scroll = 0;
                self.mode = AppMode::SubMenu(submenu_idx);
            }
            _ => {}
        }
    }

    /// Procesa teclas en el **campo de entrada de parámetro**.
    ///
    /// - Los caracteres se añaden al buffer conforme se escriben.
    /// - `Backspace` elimina el último carácter.
    /// - `Enter` ejecuta el comando con el buffer como argumento.
    /// - `Esc` cancela y vuelve al submenú sin ejecutar nada.
    pub fn handle_input_prompt_key(
        &mut self,
        code: KeyCode,
        _modifiers: KeyModifiers,
        submenu_idx: usize,
        item_idx: usize,
    ) {
        match code {
            KeyCode::Char(c) => {
                if let AppMode::InputPrompt { ref mut buffer, .. } = self.mode {
                    buffer.push(c);
                }
            }
            KeyCode::Backspace => {
                if let AppMode::InputPrompt { ref mut buffer, .. } = self.mode {
                    buffer.pop();
                }
            }
            KeyCode::Enter => {
                // Clonar el buffer antes de acceder a submenus (campos distintos de App)
                let buffer_val = if let AppMode::InputPrompt { ref buffer, .. } = self.mode {
                    buffer.clone()
                } else {
                    return;
                };
                // fn(&str)->String implementa Copy: se puede copiar del patrón
                let execute_fn =
                    if let CommandKind::WithInput { execute, .. } =
                        &self.submenus[submenu_idx].items[item_idx].command
                    {
                        *execute
                    } else {
                        return;
                    };
                let title   = self.submenus[submenu_idx].items[item_idx].name.to_string();
                let content = execute_fn(&buffer_val);
                self.output_scroll = 0;
                self.mode = AppMode::Output { submenu_idx, title, content };
            }
            KeyCode::Esc => {
                self.mode = AppMode::SubMenu(submenu_idx);
            }
            _ => {}
        }
    }

    /// Procesa teclas en la **pantalla de gráficos**.
    ///
    /// Cualquier tecla de acción vuelve al submenú padre.
    pub fn handle_chart_key(&mut self, code: KeyCode, _modifiers: KeyModifiers, submenu_idx: usize) {
        match code {
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') | KeyCode::Backspace => {
                self.mode = AppMode::SubMenu(submenu_idx);
            }
            _ => {}
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// BUCLE PRINCIPAL
// ─────────────────────────────────────────────────────────────────────────────

/// Inicializa el terminal, ejecuta el bucle de eventos y restaura el estado al salir.
pub fn run() -> Result<()> {
    // Hook de pánico: garantiza restaurar el terminal aunque el programa falle.
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(info);
    }));

    // Configurar terminal en modo raw y pantalla alternativa
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend  = CrosstermBackend::new(stdout);
    let mut term = Terminal::new(backend)?;
    let mut app  = App::new();

    // ── Bucle principal ──────────────────────────────────────────────────────
    loop {
        if matches!(app.mode, AppMode::MainMenu)
            && app.hardware_updated_at.elapsed() >= Duration::from_secs(1)
        {
            app.hardware = HardwareInfo::read();
            app.hardware_updated_at = Instant::now();
        }
        term.draw(|f| crate::ui::render(f, &app))?;

        if !event::poll(Duration::from_millis(250))? {
            continue;
        }
        if let Event::Key(key) = event::read()? {
            // Ctrl+C global: salir desde cualquier modo
            if key.modifiers.contains(KeyModifiers::CONTROL)
                && key.code == KeyCode::Char('c')
            {
                break;
            }

            // Clonar el modo para evitar conflictos de préstamo al llamar &mut self
            let should_quit = match app.mode.clone() {
                AppMode::MainMenu => {
                    app.handle_main_key(key.code, key.modifiers)
                }
                AppMode::SubMenu(idx) => {
                    app.handle_submenu_key(key.code, key.modifiers, idx);
                    false
                }
                AppMode::Output { submenu_idx, .. } => {
                    app.handle_output_key(key.code, key.modifiers, submenu_idx);
                    false
                }
                AppMode::InputPrompt { submenu_idx, item_idx, .. } => {
                    app.handle_input_prompt_key(key.code, key.modifiers, submenu_idx, item_idx);
                    false
                }
                AppMode::Chart { submenu_idx, .. } => {
                    app.handle_chart_key(key.code, key.modifiers, submenu_idx);
                    false
                }
            };

            if should_quit {
                break;
            }
        }
    }

    // ── Restaurar terminal ───────────────────────────────────────────────────
    disable_raw_mode()?;
    execute!(term.backend_mut(), LeaveAlternateScreen)?;
    term.show_cursor()?;

    Ok(())
}
