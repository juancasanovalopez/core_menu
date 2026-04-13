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
            " ↑↓/jk: Navegar   Enter: Seleccionar   q / Ctrl+C: Salir ",
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
