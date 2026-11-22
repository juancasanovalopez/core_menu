//! Renderizado visual de la interfaz de usuario mediante [`ratatui`].
//!
//! ## Estructura de pantallas
//!
//! ```text
//! ┌─────────────────────────────────────────────────────┐
//! │  CORE MENU  —  Gestor de Sistema Linux              │  ← cabecera (3 líneas)
//! ├─────────────────────────────────────────────────────┤
//! │                                                     │
//! │   [contenido según AppMode]                         │  ← área principal
//! │                                                     │
//! ├─────────────────────────────────────────────────────┤
//! │  ↑↓/jk: Navegar   Enter: Seleccionar   q: Salir    │  ← barra de ayuda (3 líneas)
//! └─────────────────────────────────────────────────────┘
//! ```

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

use crate::app::{App, AppMode};
use crate::hardware::GpuMemoryKind;
use crate::submenus::ChartBar;

// ─────────────────────────────────────────────────────────────────────────────
// PALETA DE COLORES
// ─────────────────────────────────────────────────────────────────────────────

const BORDER_COLOR:   Color = Color::Cyan;
const TITLE_COLOR:    Color = Color::Cyan;
const SELECTED_BG:    Color = Color::DarkGray;
const SELECTED_FG:    Color = Color::Yellow;
const ITEM_NAME_FG:   Color = Color::White;
const ITEM_DESC_FG:   Color = Color::DarkGray;
const SEPARATOR_FG:   Color = Color::DarkGray;
const FOOTER_FG:      Color = Color::DarkGray;
const EXIT_FG:        Color = Color::Red;
const OUTPUT_TITLE_FG: Color = Color::Yellow;

// ─────────────────────────────────────────────────────────────────────────────
// PUNTO DE ENTRADA DEL RENDERIZADO
// ─────────────────────────────────────────────────────────────────────────────

/// Función principal de renderizado. Se invoca en cada frame del bucle de eventos.
///
/// Divide la pantalla en tres zonas verticales y delega en las funciones
/// específicas según el [`AppMode`] activo.
pub fn render(f: &mut Frame, app: &App) {
    let area   = f.size();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // cabecera
            Constraint::Min(5),    // contenido principal
            Constraint::Length(3), // barra de ayuda
        ])
        .split(area);

    render_header(f, chunks[0]);

    match &app.mode {
        AppMode::MainMenu              => render_main_menu(f, app, chunks[1]),
        AppMode::Update => render_update(f, app, chunks[1]),
        AppMode::SubMenu(idx)         => render_submenu(f, app, *idx, chunks[1]),
        AppMode::Output { title, content, .. } => {
            render_output(f, app, title, content, chunks[1]);
        }
        AppMode::InputPrompt { prompt, buffer, .. } => {
            render_input_prompt(f, prompt, buffer, chunks[1]);
        }
        AppMode::Chart { title, bars, .. } => {
            render_chart(f, title, bars, chunks[1]);
        }
    }

    render_footer(f, &app.mode, chunks[2]);
}

// ─────────────────────────────────────────────────────────────────────────────
// CABECERA
// ─────────────────────────────────────────────────────────────────────────────

fn render_header(f: &mut Frame, area: Rect) {
    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            " CORE MENU ",
            Style::default()
                .fg(Color::Black)
                .bg(TITLE_COLOR)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            "Gestor de Sistema Linux",
            Style::default().fg(Color::White),
        ),
    ]))
    .alignment(Alignment::Center)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_COLOR)),
    );

    f.render_widget(header, area);
}

// ─────────────────────────────────────────────────────────────────────────────
// MENÚ PRINCIPAL
// ─────────────────────────────────────────────────────────────────────────────

/// Renderiza la lista de submenús + entrada "Salir" con un separador visual.
///
/// ## Mapeo cursor ↔ índice visual
///
/// Se inserta un `ListItem` vacío como separador antes de "Salir".
/// El cursor lógico no incluye ese separador, por lo que hay que compensarlo:
/// - cursor < n_submenús  → índice visual = cursor
/// - cursor == n_submenús → índice visual = cursor + 1  (salta el separador)
fn render_main_menu(f: &mut Frame, app: &App, area: Rect) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(3)])
        .split(area);
    f.render_widget(
        Paragraph::new(format!(" {}", app.updater.state.summary()))
            .style(Style::default().fg(ITEM_DESC_FG)),
        sections[0],
    );
    let area = sections[1];
    let summary = hardware_lines(app);
    let summary_height = (summary.len().saturating_add(1).min(u16::MAX as usize) as u16)
        .min(area.height.saturating_sub(5));
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(summary_height), Constraint::Min(3)])
        .split(area);
    render_hardware(f, summary, chunks[0]);
    let area = chunks[1];
    let n = app.submenus.len();

    // Entradas de submenús
    let mut items: Vec<ListItem> = app
        .submenus
        .iter()
        .map(|sm| {
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{:<30}", sm.name),
                    Style::default()
                        .fg(ITEM_NAME_FG)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" {}", sm.description),
                    Style::default().fg(ITEM_DESC_FG),
                ),
            ]))
        })
        .collect();

    // Separador visual (no seleccionable)
    items.push(ListItem::new(Span::styled(
        "─".repeat(50),
        Style::default().fg(SEPARATOR_FG),
    )));

    // Entrada "Salir"
    items.push(ListItem::new(Line::from(vec![
        Span::styled(
            "Salir",
            Style::default().fg(EXIT_FG).add_modifier(Modifier::BOLD),
        ),
    ])));

    // Mapear cursor lógico a índice visual (omitiendo el separador)
    let visual_idx = if app.main_cursor < n {
        app.main_cursor
    } else {
        app.main_cursor + 1 // salta el listitem separador
    };

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(BORDER_COLOR))
                .title(Span::styled(
                    " Menú Principal ",
                    Style::default()
                        .fg(TITLE_COLOR)
                        .add_modifier(Modifier::BOLD),
                )),
        )
        .highlight_style(
            Style::default()
                .bg(SELECTED_BG)
                .fg(SELECTED_FG)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    let mut state = ListState::default();
    state.select(Some(visual_idx));

    f.render_stateful_widget(list, area, &mut state);
}

fn render_update(f: &mut Frame, app: &App, area: Rect) {
    use crate::update::{CheckOutcome, State};
    let mut lines = vec![
        Line::from(format!("Versión instalada: {}", env!("CARGO_PKG_VERSION"))),
    ];
    match &app.updater.state {
        State::Checked(CheckOutcome::Available(release)) => {
            lines.push(Line::from(format!("Versión disponible: {}", release.tag_name)));
            lines.push(Line::from("¿Descargar y verificar esta versión?"));
            lines.push(Line::from("[s] Sí / [n] No"));
            lines.push(Line::from("Todavía no se modificará el ejecutable."));
            lines.push(Line::from("La firma Ed25519 y SHA-256 se validarán antes de instalar."));
            lines.push(Line::from(release.html_url.clone()));
        }
        State::Downloading => {
            lines.push(Line::from(app.updater.state.summary()));
            if let Some((bytes, total)) = app.updater.progress {
                lines.push(Line::from(format!("{} / {} descargados", fmt_bytes(bytes), fmt_bytes(total))));
            }
            lines.push(Line::from("Esc/n: cancelar sin modificar el ejecutable."));
        }
        State::Ready(prepared) => {
            lines.push(Line::from(format!("Versión verificada: {}", prepared.release.tag_name)));
            lines.push(Line::from("¿Reemplazar el ejecutable?"));
            lines.push(Line::from("[s] Sí / [n] No"));
            lines.push(Line::from(format!("Destino: {}", prepared.destination.path.display())));
            lines.push(Line::from(if prepared.destination.elevated {
                "Instalación del sistema: se solicitarán permisos con sudo."
            } else {
                "Instalación del usuario: sin permisos de administrador."
            }));
            lines.push(Line::from("Se conservará una copia; después la aplicación se cerrará."));
        }
        _ => {
            lines.push(Line::from(app.updater.state.summary()));
            lines.push(Line::from(crate::update::RELEASES_URL));
        }
    }
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((app.update_scroll, 0))
            .block(Block::default().borders(Borders::ALL).title(" Actualizaciones ")
                .border_style(Style::default().fg(BORDER_COLOR))),
        area,
    );
}

fn hardware_lines(app: &App) -> Vec<Line<'static>> {
    let memory = match &app.hardware.memory {
        Ok(memory) => Line::from(vec![
            Span::styled(" RAM disponible/total: ", Style::default().fg(ITEM_DESC_FG)),
            Span::styled(
                format!(
                    "{} / {}",
                    fmt_bytes(memory.available),
                    fmt_bytes(memory.total),
                ),
                Style::default().fg(ITEM_NAME_FG),
            ),
        ]),
        Err(error) => Line::styled(
            format!(" RAM: Error: {error}"),
            Style::default().fg(Color::Red),
        ),
    };
    let cpu = match &app.hardware.cpu {
        Ok(cpu) => Line::from(vec![
            Span::styled(" CPU: ", Style::default().fg(ITEM_DESC_FG)),
            Span::styled(
                format!("{} núcleos / {} hilos", cpu.cores, cpu.threads),
                Style::default().fg(ITEM_NAME_FG),
            ),
        ]),
        Err(error) => Line::styled(
            format!(" CPU: Error: {error}"),
            Style::default().fg(Color::Red),
        ),
    };
    let mut lines = vec![memory, cpu];
    match &app.hardware.gpus {
        Ok(gpus) if gpus.is_empty() => lines.push(Line::styled(
            " GPU: No se detectaron procesadores gráficos PCI",
            Style::default().fg(ITEM_DESC_FG),
        )),
        Ok(gpus) => {
            for (index, gpu) in gpus.iter().enumerate() {
                lines.push(Line::from(vec![
                    Span::styled(format!(" GPU {index}: "), Style::default().fg(ITEM_DESC_FG)),
                    Span::styled(gpu.name.clone(), Style::default().fg(ITEM_NAME_FG)),
                ]));
                match &gpu.memory {
                    Ok(pools) => {
                        for pool in pools {
                            let label = match pool.kind {
                                GpuMemoryKind::Dedicated => "VRAM dedicada",
                                GpuMemoryKind::Shared => "RAM compartida (~)",
                                GpuMemoryKind::ReservedShared => "Compartida reservada",
                            };
                            let available = pool.available.map(fmt_bytes)
                                .unwrap_or_else(|| "N/D".to_string());
                            lines.push(Line::from(vec![
                                Span::styled(format!(" {label}: "), Style::default().fg(ITEM_DESC_FG)),
                                Span::styled(
                                    format!("{available} / {}", fmt_bytes(pool.total)),
                                    Style::default().fg(ITEM_NAME_FG),
                                ),
                            ]));
                        }
                    }
                    Err(error) => lines.push(Line::styled(
                        format!(" Memoria/tipo GPU: Error: {error}"),
                        Style::default().fg(Color::Red),
                    )),
                }
            }
        }
        Err(error) => lines.push(Line::styled(
            format!(" GPU: Error: {error}"),
            Style::default().fg(Color::Red),
        )),
    }
    lines
}

fn render_hardware(f: &mut Frame, mut lines: Vec<Line<'static>>, area: Rect) {
    let visible = area.height as usize;
    if lines.len() > visible && visible > 0 {
        let hidden = lines.len() - visible + 1;
        lines.truncate(visible - 1);
        lines.push(Line::styled(
            format!(" +{hidden} líneas (amplía la ventana)"),
            Style::default().fg(ITEM_DESC_FG),
        ));
    }
    f.render_widget(Paragraph::new(lines), area);
}

// ─────────────────────────────────────────────────────────────────────────────
// SUBMENÚ
// ─────────────────────────────────────────────────────────────────────────────

/// Renderiza los comandos disponibles dentro del submenú seleccionado.
fn render_submenu(f: &mut Frame, app: &App, idx: usize, area: Rect) {
    let submenu = &app.submenus[idx];

    let items: Vec<ListItem> = submenu
        .items
        .iter()
        .map(|item| {
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{:<32}", item.name),
                    Style::default()
                        .fg(ITEM_NAME_FG)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" {}", item.description),
                    Style::default().fg(ITEM_DESC_FG),
                ),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(BORDER_COLOR))
                .title(Span::styled(
                    format!(" {} ", submenu.name),
                    Style::default()
                        .fg(TITLE_COLOR)
                        .add_modifier(Modifier::BOLD),
                )),
        )
        .highlight_style(
            Style::default()
                .bg(SELECTED_BG)
                .fg(SELECTED_FG)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    let mut state = ListState::default();
    state.select(Some(app.sub_cursor));

    f.render_stateful_widget(list, area, &mut state);
}

// ─────────────────────────────────────────────────────────────────────────────
// PANTALLA DE SALIDA
// ─────────────────────────────────────────────────────────────────────────────

/// Renderiza el resultado de un comando con soporte de scroll vertical.
fn render_output(f: &mut Frame, app: &App, title: &str, content: &str, area: Rect) {
    let paragraph = Paragraph::new(content)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(BORDER_COLOR))
                .title(Span::styled(
                    format!(" {} ", title),
                    Style::default()
                        .fg(OUTPUT_TITLE_FG)
                        .add_modifier(Modifier::BOLD),
                )),
        )
        .wrap(Wrap { trim: false })
        .scroll((app.output_scroll, 0));

    f.render_widget(paragraph, area);
}

// ─────────────────────────────────────────────────────────────────────────────// CAMPO DE ENTRADA DE PARÁMETRO
// ────────────────────────────────────────────────────────────────────────────────

/// Renderiza el cuadro de entrada de parámetro centrado verticalmente en `area`.
///
/// Muestra la etiqueta [`prompt`], el texto acumulado en [`buffer`] y un cursor
/// intermitente visual (`█`).
fn render_input_prompt(f: &mut Frame, prompt: &str, buffer: &str, area: Rect) {
    // Centrar el cuadro verticalmente: 40% de espacio arriba, caja de 7 líneas
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Length(7),
            Constraint::Min(0),
        ])
        .split(area);

    // Centrar el cuadro horizontalmente: dejar márgenes laterales
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(15),
            Constraint::Min(0),
            Constraint::Percentage(15),
        ])
        .split(rows[1]);

    let box_area = cols[1];

    let content = vec![
        Line::from(Span::styled(
            prompt,
            Style::default().fg(Color::White),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "> ",
                Style::default().fg(TITLE_COLOR).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                buffer,
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
            // Cursor visual
            Span::styled(
                "█",
                Style::default().fg(Color::Yellow),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "(Backspace: borrar carácter   Esc: cancelar)",
            Style::default().fg(ITEM_DESC_FG),
        )),
    ];

    let paragraph = Paragraph::new(content)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(BORDER_COLOR))
                .title(Span::styled(
                    " Parámetro requerido ",
                    Style::default().fg(TITLE_COLOR).add_modifier(Modifier::BOLD),
                )),
        )
        .alignment(Alignment::Left);

    f.render_widget(paragraph, box_area);
}

// ─────────────────────────────────────────────────────────────────────────────
// GRÁFICOS DE OCUPADO / DISPONIBLE
// ─────────────────────────────────────────────────────────────────────────────

/// Renderiza una pantalla de barras de ocupación (gauge por cada [`ChartBar`]).
///
/// Cada entrada ocupa 4 líneas:
/// ```text
/// etiqueta     usado / total   XX%
/// [██████████████████          ]
/// [██████████████████          ]
/// (separador)
/// ```
/// Color del gauge: verde (< 70 %), amarillo (70–89 %), rojo (≥ 90 %).
fn render_chart(f: &mut Frame, title: &str, bars: &[ChartBar], area: Rect) {
    let outer = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BORDER_COLOR))
        .title(Span::styled(
            format!(" {} ", title),
            Style::default().fg(TITLE_COLOR).add_modifier(Modifier::BOLD),
        ));
    let inner = outer.inner(area);
    f.render_widget(outer, area);

    if bars.is_empty() {
        let msg = Paragraph::new("No hay datos disponibles.")
            .alignment(Alignment::Center)
            .style(Style::default().fg(ITEM_DESC_FG));
        f.render_widget(msg, inner);
        return;
    }

    // 1 línea etiqueta + 2 líneas gauge + 1 separador = 4 por barra
    let rows_per_bar: u16 = 4;
    let max_bars   = (inner.height / rows_per_bar) as usize;
    let visible    = bars.len().min(max_bars.max(1));

    let mut constraints: Vec<Constraint> = Vec::with_capacity(visible * 3 + 1);
    for _ in 0..visible {
        constraints.push(Constraint::Length(1)); // etiqueta
        constraints.push(Constraint::Length(2)); // gauge
        constraints.push(Constraint::Length(1)); // separador
    }
    constraints.push(Constraint::Min(0));

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    for (i, bar) in bars.iter().take(visible).enumerate() {
        let pct = if bar.total > 0 {
            ((bar.used as f64 / bar.total as f64) * 100.0).round().min(100.0) as u16
        } else {
            0
        };
        let bar_color = if pct < 70 {
            Color::Green
        } else if pct < 90 {
            Color::Yellow
        } else {
            Color::Red
        };

        // — Línea de etiqueta —
        // Cuando total == 100 la barra representa un porcentaje directo
        // (CPU, GPU…); en ese caso no tiene sentido mostrar "45 B / 100 B".
        let detail = if bar.total == 100 {
            String::new()
        } else {
            format!("  {} / {}", fmt_bytes(bar.used), fmt_bytes(bar.total))
        };
        let label_line = Line::from(vec![
            Span::styled(
                format!("{:<28}", bar.label),
                Style::default().fg(ITEM_NAME_FG).add_modifier(Modifier::BOLD),
            ),
            Span::styled(detail, Style::default().fg(ITEM_DESC_FG)),
            Span::styled(
                format!("   {}%", pct),
                Style::default().fg(bar_color).add_modifier(Modifier::BOLD),
            ),
        ]);
        f.render_widget(Paragraph::new(label_line), rows[i * 3]);

        // — Gauge —
        let gauge = Gauge::default()
            .gauge_style(Style::default().fg(bar_color).bg(Color::DarkGray))
            .percent(pct)
            .label(Span::raw(""));
        f.render_widget(gauge, rows[i * 3 + 1]);
    }

    // Aviso si no caben todas las barras en pantalla
    if bars.len() > max_bars {
        let overflow = rows[visible * 3];
        let msg = Paragraph::new(format!(
            "  …{} entradas más (amplía la ventana para verlas)",
            bars.len() - max_bars,
        ))
        .style(Style::default().fg(ITEM_DESC_FG));
        f.render_widget(msg, overflow);
    }
}

/// Formatea bytes en una cadena legible (GiB / MiB / KiB / B).
fn fmt_bytes(b: u64) -> String {
    const GIB: u64 = 1 << 30;
    const MIB: u64 = 1 << 20;
    const KIB: u64 = 1 << 10;
    if b >= GIB      { format!("{:.1} GiB", b as f64 / GIB as f64) }
    else if b >= MIB { format!("{:.1} MiB", b as f64 / MIB as f64) }
    else if b >= KIB { format!("{:.1} KiB", b as f64 / KIB as f64) }
    else             { format!("{} B", b) }
}

// ────────────────────────────────────────────────────────────────────────────────// BARRA DE AYUDA (pie de página)
// ─────────────────────────────────────────────────────────────────────────────

/// Renderiza las instrucciones de teclado contextuales según el modo actual.
fn render_footer(f: &mut Frame, mode: &AppMode, area: Rect) {
    let hints = match mode {
        AppMode::MainMenu =>
            " ↑↓/jk: Navegar   Enter: Seleccionar   u: Actualizaciones   q: Salir ",
        AppMode::Update =>
            " s: Sí  n/Esc: Volver  ↑↓: Scroll ",
        AppMode::SubMenu(_) =>
            " ↑↓/jk: Navegar   Enter: Ejecutar   Esc/q: Menú principal ",
        AppMode::Output { .. } =>
            " ↑↓/jk: Scroll   Enter / Esc / q: Volver al submenú ",
        AppMode::InputPrompt { .. } =>
            " Escribe el valor   Enter: Confirmar   Esc: Cancelar ",
        AppMode::Chart { .. } =>
            " Enter / Esc / q: Volver al submenú ",
    };

    let footer = Paragraph::new(hints)
        .alignment(Alignment::Center)
        .style(Style::default().fg(FOOTER_FG))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(FOOTER_FG)),
        );

    f.render_widget(footer, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hardware::{CpuInfo, GpuInfo, GpuMemory, HardwareInfo, MemoryInfo};
    use crossterm::event::{KeyCode, KeyModifiers};
    use ratatui::{backend::TestBackend, Terminal};

    fn app_with_hardware() -> App {
        let mut app = App::new();
        app.hardware = HardwareInfo {
            memory: Ok(MemoryInfo {
                available: 4 << 30,
                total: 8 << 30,
            }),
            cpu: Ok(CpuInfo { cores: 4, threads: 8 }),
            gpus: Ok(Vec::new()),
        };
        app
    }

    fn screen(app: &App, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| render(frame, app)).unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    #[test]
    fn update_offer_and_errors_are_visible_at_small_sizes() {
        let mut app = crate::app::tests::available_app();
        app.poll_updates();
        for (width, height) in [(80, 24), (40, 14)] {
            let text = screen(&app, width, height);
            assert!(text.contains("Versión instalada:"));
            assert!(text.contains("Versión disponible: v99.0.0"));
            assert!(text.contains("[s] Sí / [n] No"));
        }
        app.updater.state = crate::update::State::Error("Sin conexión".into());
        assert!(screen(&app, 40, 14).contains("Sin conexión"));
        app.handle_update_key(KeyCode::Down);
        assert_eq!(app.update_scroll, 1);
        app.handle_update_key(KeyCode::Esc);
        assert_eq!(app.update_scroll, 0);
        app.mode = AppMode::Update;
        app.updater.state = crate::update::State::Ready(crate::update::Prepared::fixture());
        for (width, height) in [(80, 24), (40, 14)] {
            let text = screen(&app, width, height);
            assert!(text.contains("Versión verificada: v99.0.0"));
            assert!(text.contains("[s] Sí / [n] No"));
        }
    }

    #[test]
    fn main_menu_shows_minimal_summary_at_normal_and_narrow_sizes() {
        let app = app_with_hardware();
        for (width, height) in [(80, 24), (40, 18)] {
            let text = screen(&app, width, height);
            assert!(text.contains("RAM disponible/total: 4.0 GiB / 8.0 GiB"));
            assert!(text.contains("CPU: 4 núcleos / 8 hilos"));
            assert!(text.contains("Menú Principal"));
            assert!(text.contains(app.submenus[0].name));
            if height == 24 {
                assert!(text.contains("Salir"));
            }
        }
    }

    #[test]
    fn summary_does_not_change_navigation_or_submenus() {
        let mut app = app_with_hardware();
        assert!(!app.handle_main_key(KeyCode::Enter, KeyModifiers::NONE));
        let text = screen(&app, 80, 24);
        assert!(!text.contains("RAM disponible/total:"));
        app.handle_submenu_key(KeyCode::Esc, KeyModifiers::NONE, 0);
        assert!(screen(&app, 80, 24).contains("RAM disponible/total:"));
        for _ in 0..app.submenus.len() {
            assert!(!app.handle_main_key(KeyCode::Down, KeyModifiers::NONE));
        }
        assert!(screen(&app, 40, 14).contains("Salir"));
        assert!(app.handle_main_key(KeyCode::Enter, KeyModifiers::NONE));
    }

    #[test]
    fn errors_are_visible_without_hiding_other_metrics() {
        let mut app = app_with_hardware();
        app.hardware.memory = Err("Falta MemAvailable".into());
        let text = screen(&app, 80, 24);
        assert!(text.contains("RAM: Error: Falta MemAvailable"));
        assert!(text.contains("CPU: 4 núcleos / 8 hilos"));
        app.hardware.cpu = Err("No se pudo leer la topología".into());
        assert!(screen(&app, 80, 24).contains("CPU: Error: No se pudo leer la topología"));
    }

    #[test]
    fn summary_shows_multiple_gpus_and_memory_types() {
        let mut app = app_with_hardware();
        app.hardware.gpus = Ok(vec![
            GpuInfo {
                name: "AMD integrada".into(),
                memory: Ok(vec![
                    GpuMemory { kind: GpuMemoryKind::ReservedShared, available: Some(1 << 30), total: 2 << 30 },
                    GpuMemory { kind: GpuMemoryKind::Shared, available: Some(3 << 30), total: 4 << 30 },
                ]),
            },
            GpuInfo {
                name: "NVIDIA dedicada".into(),
                memory: Ok(vec![GpuMemory {
                    kind: GpuMemoryKind::Dedicated, available: Some(6 << 30), total: 8 << 30,
                }]),
            },
        ]);
        let text = screen(&app, 80, 30);
        for value in [
            "GPU 0: AMD integrada", "GPU 1: NVIDIA dedicada",
            "Compartida reservada: 1.0 GiB / 2.0 GiB",
            "RAM compartida (~): 3.0 GiB / 4.0 GiB",
            "VRAM dedicada: 6.0 GiB / 8.0 GiB",
        ] {
            assert!(text.contains(value), "{value}");
        }
        let small = screen(&app, 40, 14);
        assert!(small.contains("amplía la ventana"));
        assert!(small.contains("Menú Principal"));
        let narrow = screen(&app, 40, 26);
        assert!(narrow.contains("Compartida reservada: 1.0 GiB / 2.0 GiB"));
        assert!(narrow.contains("RAM compartida (~): 3.0 GiB / 4.0 GiB"));
        assert!(narrow.contains("VRAM dedicada: 6.0 GiB / 8.0 GiB"));
    }

    #[test]
    fn gpu_errors_and_unavailable_memory_are_explicit() {
        let mut app = app_with_hardware();
        app.hardware.gpus = Ok(vec![GpuInfo {
            name: "Intel Arc".into(),
            memory: Ok(vec![GpuMemory { kind: GpuMemoryKind::Dedicated, available: None, total: 8 << 30 }]),
        }]);
        assert!(screen(&app, 80, 24).contains("VRAM dedicada: N/D / 8.0 GiB"));
        app.hardware.gpus.as_mut().unwrap()[0].memory = Err("Sin permisos DRM".into());
        assert!(screen(&app, 80, 24).contains("Memoria/tipo GPU: Error: Sin permisos DRM"));
        app.hardware.gpus = Err("No se pudo consultar PCI".into());
        assert!(screen(&app, 80, 24).contains("GPU: Error: No se pudo consultar PCI"));
    }
}
