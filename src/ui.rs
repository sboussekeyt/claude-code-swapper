use crate::app::{AppState, Modal, Panel};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

const ACCENT: Color = Color::Cyan;
const HEADER_ACCENT: Color = Color::Magenta;
const MUTED: Color = Color::DarkGray;
const ON_COLOR: Color = Color::Green;
const OFF_COLOR: Color = Color::DarkGray;
const BADGE_COLOR: Color = Color::Green;
const SEARCH_ACCENT: Color = Color::Yellow;
const WARN_COLOR: Color = Color::Yellow;

pub fn render(frame: &mut Frame, state: &AppState) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Length(4),
        ])
        .split(area);

    render_status(frame, state, chunks[0]);
    render_panels(frame, state, chunks[1]);
    render_footer(frame, state, chunks[2]);

    if let Some(modal) = &state.modal {
        render_modal(frame, modal, area);
    }
}

fn render_status(frame: &mut Frame, state: &AppState, area: Rect) {
    let provider = state.current_provider.as_deref().unwrap_or("-");
    let model = state.current_model.as_deref().unwrap_or("-");
    let line = Line::from(vec![
        Span::styled("Provider: ", Style::default().fg(MUTED)),
        Span::styled(provider, Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
        Span::styled("   Model: ", Style::default().fg(MUTED)),
        Span::styled(model, Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
        Span::styled("   RTK: ", Style::default().fg(MUTED)),
        on_off_span(state.rtk_enabled),
        Span::styled("   Auto-accept: ", Style::default().fg(MUTED)),
        on_off_span(state.auto_accept),
    ]);
    let block = Block::default()
        .title(Span::styled(
            " Claude Code Swapper ",
            Style::default().fg(HEADER_ACCENT).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(HEADER_ACCENT));
    frame.render_widget(Paragraph::new(line).block(block), area);
}

fn on_off_span(enabled: bool) -> Span<'static> {
    if enabled {
        Span::styled("ON", Style::default().fg(ON_COLOR).add_modifier(Modifier::BOLD))
    } else {
        Span::styled("OFF", Style::default().fg(OFF_COLOR))
    }
}

fn render_panels(frame: &mut Frame, state: &AppState, area: Rect) {
    let panels = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    let providers_focused = state.focused_panel == Panel::Providers;
    let provider_items: Vec<ListItem> = state
        .providers
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let style = if i == state.provider_cursor {
                cursor_style(providers_focused)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(p.as_str())).style(style)
        })
        .collect();
    let providers_border = panel_border_style(providers_focused);
    let mut provider_state = ListState::default().with_selected(Some(state.provider_cursor));
    frame.render_stateful_widget(
        List::new(provider_items).block(
            Block::default()
                .title(panel_title("Providers", providers_focused))
                .borders(Borders::ALL)
                .border_style(providers_border),
        ),
        panels[0],
        &mut provider_state,
    );

    // While searching, carve a dedicated strip off the top of the Models
    // column for the search bar rather than floating it over the list — an
    // overlay would otherwise sit on top of (and hide) the very first result.
    let (search_area, models_list_area) = if state.search_active {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(panels[1]);
        (Some(rows[0]), rows[1])
    } else {
        (None, panels[1])
    };

    let models_focused = state.focused_panel == Panel::Models;
    let focused_context_windows = state
        .focused_provider()
        .and_then(|p| state.config.providers.get(p))
        .map(|p| &p.context_windows);
    let model_items: Vec<ListItem> = state
        .models_for_focused_provider
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let style = if i == state.model_cursor {
                cursor_style(models_focused)
            } else {
                Style::default()
            };
            let line = match focused_context_windows.and_then(|w| w.get(m)) {
                Some(tokens) => Line::from(vec![
                    Span::raw(m.as_str()),
                    Span::raw("  "),
                    Span::styled(
                        format!("[{}]", format_context_tokens(*tokens)),
                        Style::default().fg(BADGE_COLOR).add_modifier(Modifier::BOLD),
                    ),
                ]),
                None => Line::from(m.as_str()),
            };
            ListItem::new(line).style(style)
        })
        .collect();
    let models_border = panel_border_style(models_focused);
    let mut model_state = ListState::default().with_selected(Some(state.model_cursor));
    frame.render_stateful_widget(
        List::new(model_items).block(
            Block::default()
                .title(panel_title("Models", models_focused))
                .borders(Borders::ALL)
                .border_style(models_border),
        ),
        models_list_area,
        &mut model_state,
    );

    if let Some(search_area) = search_area {
        render_search_bar(frame, state, search_area);
    }
}

fn format_context_tokens(tokens: u64) -> String {
    // Real-world context windows are often powers of two (1_048_576,
    // 1_310_720) rather than round decimal multiples, so round to one
    // decimal place instead of only special-casing exact multiples.
    if tokens >= 1_000_000 {
        format_rounded(tokens, 1_000_000, "M")
    } else if tokens >= 1_000 {
        format_rounded(tokens, 1_000, "K")
    } else {
        tokens.to_string()
    }
}

fn format_rounded(tokens: u64, unit: u64, suffix: &str) -> String {
    let mut whole = tokens / unit;
    let remainder = tokens % unit;
    // Round remainder/unit to the nearest tenth (not always up), carrying
    // into the whole part if it rounds all the way up to the next unit.
    let mut tenths = (remainder * 10 + unit / 2) / unit;
    if tenths == 10 {
        whole += 1;
        tenths = 0;
    }
    if tenths == 0 {
        format!("{whole}{suffix}")
    } else {
        format!("{whole}.{tenths}{suffix}")
    }
}

fn panel_title(title: &str, focused: bool) -> Span<'static> {
    let color = if focused { ACCENT } else { MUTED };
    Span::styled(format!(" {title} "), Style::default().fg(color).add_modifier(Modifier::BOLD))
}

fn panel_border_style(focused: bool) -> Style {
    if focused {
        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(MUTED)
    }
}

fn cursor_style(panel_focused: bool) -> Style {
    if panel_focused {
        Style::default()
            .fg(Color::Black)
            .bg(SEARCH_ACCENT)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White).bg(MUTED)
    }
}

const FOOTER_KEYS: &[(&str, &str)] = &[
    ("Tab", "switch"),
    ("↑/↓", "move"),
    ("Enter", "select"),
    ("/", "search"),
    ("l", "launch"),
    ("n", "native"),
    ("a", "add"),
    ("x", "remove"),
    ("s", "api key"),
    ("r", "rtk"),
    ("p", "auto-accept"),
    ("q", "quit"),
];

const SEARCH_FOOTER_KEYS: &[(&str, &str)] =
    &[("type", "filter"), ("↑/↓", "move"), ("Enter", "select"), ("Esc", "cancel search")];

fn footer_key_spans(keys: &[(&str, &str)]) -> Vec<Span<'static>> {
    let mut spans = Vec::with_capacity(keys.len() * 3);
    for (i, (key, label)) in keys.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        spans.push(Span::styled(
            format!("[{key}]"),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(format!(" {label}"), Style::default().fg(MUTED)));
    }
    spans
}

fn render_footer(frame: &mut Frame, state: &AppState, area: Rect) {
    let mut lines = if state.search_active {
        vec![Line::from(footer_key_spans(SEARCH_FOOTER_KEYS))]
    } else {
        vec![Line::from(footer_key_spans(FOOTER_KEYS))]
    };
    if let Some(msg) = &state.status_message {
        lines.push(Line::from(Span::styled(
            format!("[{msg}]"),
            Style::default().fg(WARN_COLOR).add_modifier(Modifier::BOLD),
        )));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), area);
}

fn render_search_bar(frame: &mut Frame, state: &AppState, area: Rect) {
    // A dedicated strip carved out of the Models column (see render_panels),
    // not an overlay — so the filtered list underneath is never obscured.
    let text = format!("{}_", state.search_query);
    let block = Block::default()
        .title(Span::styled(
            " Search models ",
            Style::default().fg(SEARCH_ACCENT).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(SEARCH_ACCENT).add_modifier(Modifier::BOLD));
    frame.render_widget(
        Paragraph::new(Span::styled(text, Style::default().fg(Color::White))).block(block),
        area,
    );
}

fn render_modal(frame: &mut Frame, modal: &Modal, area: Rect) {
    let (title, provider, input) = match modal {
        Modal::AddModel { provider, input } => ("Add model", provider, input),
        Modal::SetApiKey { provider, input } => ("Set API key", provider, input),
    };
    let width = area.width.min(60);
    let height = 3;
    let popup = Rect {
        x: (area.width.saturating_sub(width)) / 2,
        y: (area.height.saturating_sub(height)) / 2,
        width,
        height,
    };
    let masked = match modal {
        Modal::SetApiKey { .. } => "*".repeat(input.len()),
        _ => input.clone(),
    };
    let block = Block::default()
        .title(Span::styled(
            format!(" {title} ({provider}) "),
            Style::default().fg(HEADER_ACCENT).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(HEADER_ACCENT).add_modifier(Modifier::BOLD));
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(Span::styled(masked, Style::default().fg(Color::White))).block(block),
        popup,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, Last, Provider};
    use indexmap::IndexMap;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn render_does_not_panic() {
        let mut providers = IndexMap::new();
        providers.insert(
            "lmstudio".to_string(),
            Provider {
                base_url: "http://localhost:1234".to_string(),
                api_key: "lm-studio".to_string(),
                models: vec!["local-model".to_string()],
                ..Default::default()
            },
        );
        let state = AppState::new(Config { providers }, &Last::default());

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame, &state)).unwrap();
    }

    #[test]
    fn render_scrolls_long_model_list_to_keep_cursor_visible() {
        let mut providers = IndexMap::new();
        let models: Vec<String> = (0..50).map(|i| format!("model-{i}")).collect();
        providers.insert(
            "lmstudio".to_string(),
            Provider {
                base_url: "http://localhost:1234".to_string(),
                api_key: "lm-studio".to_string(),
                models: models.clone(),
                ..Default::default()
            },
        );
        let mut state = AppState::new(Config { providers }, &Last::default());
        state.switch_focus();
        for _ in 0..49 {
            state.move_cursor(1);
        }
        assert_eq!(state.model_cursor, 49);

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame, &state)).unwrap();

        let content = terminal.backend().buffer().content.iter().map(|c| c.symbol()).collect::<String>();
        assert!(
            content.contains("model-49"),
            "expected the selected last item to be scrolled into view"
        );
        assert!(
            !content.contains("model-0 "),
            "expected the first item to have scrolled out of view"
        );
    }

    #[test]
    fn render_with_open_modal_clears_area_beneath_it() {
        // The popup renders at rows 10-12 (top/bottom border) with its single
        // interior content row at row 11, columns 10-70, on an 80x24 backend
        // (see popup Rect math in render_modal). The modal's own Paragraph is
        // empty (freshly-opened Add Model input), so it writes no glyphs of
        // its own into that interior row — only Clear stands between it and
        // whatever was drawn there before. Put the marker as item index 7 in
        // the Models list so it lands at row 4+7=11 — the popup's interior
        // row, not a border row the Block would overwrite regardless of Clear.
        let mut providers = IndexMap::new();
        let mut models: Vec<String> = (0..7).map(|i| format!("filler-{i}")).collect();
        models.push("bleed-marker".to_string());
        providers.insert(
            "lmstudio".to_string(),
            Provider {
                base_url: "http://localhost:1234".to_string(),
                api_key: "lm-studio".to_string(),
                models,
                ..Default::default()
            },
        );
        let mut state = AppState::new(Config { providers }, &Last::default());
        state.open_add_model_modal();

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame, &state)).unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
        // The modal's own title/content must be present...
        assert!(content.contains("Add model"));
        // ...and the background model name must not bleed through the popup:
        // it should be fully covered by the modal, so it shouldn't appear at all
        // once the modal is the last thing rendered.
        assert!(
            !content.contains("bleed-marker"),
            "expected Clear to blank the popup area so the model list underneath doesn't show through"
        );
    }

    #[test]
    fn footer_shows_quit_hint_on_narrow_terminal() {
        let mut providers = IndexMap::new();
        providers.insert(
            "lmstudio".to_string(),
            Provider {
                base_url: "http://localhost:1234".to_string(),
                api_key: "lm-studio".to_string(),
                models: vec!["local-model".to_string()],
                ..Default::default()
            },
        );
        let state = AppState::new(Config { providers }, &Last::default());

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame, &state)).unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
        assert!(content.contains("quit"), "expected [q] quit to be visible even on an 80-column terminal");
    }

    #[test]
    fn render_with_active_search_does_not_panic_and_shows_the_search_popup() {
        let mut providers = IndexMap::new();
        providers.insert(
            "openrouter".to_string(),
            Provider {
                base_url: "https://openrouter.example.com".to_string(),
                api_key: "key".to_string(),
                models: vec!["anthropic/claude".to_string(), "meta/llama".to_string()],
                ..Default::default()
            },
        );
        let mut state = AppState::new(Config { providers }, &Last::default());
        state.switch_focus();
        state.start_search();
        for c in "llama".chars() {
            state.search_input_char(c);
        }

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame, &state)).unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
        assert!(content.contains("Search models"), "expected the search popup to be visible");
        assert!(content.contains("llama_"), "expected the typed query to render in the popup");
        assert!(
            content.contains("meta/llama"),
            "expected the filtered model list to still be visible behind the popup"
        );
    }

    #[test]
    fn render_with_search_on_tiny_terminal_does_not_panic() {
        let mut providers = IndexMap::new();
        providers.insert(
            "a".to_string(),
            Provider {
                base_url: "https://a.example.com".to_string(),
                api_key: "key".to_string(),
                models: vec!["m1".to_string()],
                ..Default::default()
            },
        );
        let mut state = AppState::new(Config { providers }, &Last::default());
        state.switch_focus();
        state.start_search();

        // Small enough that Percentage(40)/(60) splits and the popup's
        // saturating-subtracted dimensions are exercised near their floor.
        let backend = TestBackend::new(10, 8);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame, &state)).unwrap();
    }

    #[test]
    fn footer_renders_status_message_when_present() {
        let mut providers = IndexMap::new();
        providers.insert(
            "lmstudio".to_string(),
            Provider {
                base_url: "http://localhost:1234".to_string(),
                api_key: "lm-studio".to_string(),
                models: vec!["local-model".to_string()],
                ..Default::default()
            },
        );
        let mut state = AppState::new(Config { providers }, &Last::default());
        state.status_message = Some("something happened".to_string());

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame, &state)).unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
        assert!(content.contains("something happened"));
    }

    #[test]
    fn render_shows_configured_context_window_next_to_the_model_name() {
        let mut context_windows = IndexMap::new();
        context_windows.insert("deepseek/deepseek-v4-flash-0731".to_string(), 1_000_000u64);
        let mut providers = IndexMap::new();
        providers.insert(
            "openrouter".to_string(),
            Provider {
                base_url: "https://openrouter.example.com".to_string(),
                api_key: "key".to_string(),
                models: vec!["deepseek/deepseek-v4-flash-0731".to_string(), "other/model".to_string()],
                context_windows,
            },
        );
        let state = AppState::new(Config { providers }, &Last::default());

        let backend = TestBackend::new(100, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame, &state)).unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
        assert!(content.contains("[1M]"), "expected the configured context window to render as a suffix");
    }

    #[test]
    fn format_context_tokens_rounds_power_of_two_windows_to_one_decimal() {
        // Real-world context windows are frequently powers of two, not
        // round decimal multiples — these must round sensibly, not just
        // special-case exact millions/thousands.
        assert_eq!(format_context_tokens(1_048_576), "1M"); // 1.048576 -> rounds to 1.0
        assert_eq!(format_context_tokens(1_310_720), "1.3M"); // 1.310720 -> rounds to 1.3
        assert_eq!(format_context_tokens(1_000_000), "1M");
        assert_eq!(format_context_tokens(163_840), "163.8K"); // 163.84 -> rounds to 163.8
        assert_eq!(format_context_tokens(200_000), "200K");
        assert_eq!(format_context_tokens(8_192), "8.2K");
        assert_eq!(format_context_tokens(500), "500");
        assert_eq!(format_context_tokens(1_999_999), "2M"); // carries into the whole part
    }

    #[test]
    fn focused_panel_border_and_selection_use_the_accent_palette() {
        let mut providers = IndexMap::new();
        providers.insert(
            "openrouter".to_string(),
            Provider {
                base_url: "https://openrouter.example.com".to_string(),
                api_key: "key".to_string(),
                models: vec!["model-a".to_string()],
                ..Default::default()
            },
        );
        let state = AppState::new(Config { providers }, &Last::default());

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame, &state)).unwrap();

        let buffer = terminal.backend().buffer();
        // Providers panel is focused by default: its top-left border corner
        // (row 3, right under the status bar) must use the Cyan accent, not
        // a plain/default style.
        let border_cell = buffer.cell((0, 3)).unwrap();
        assert_eq!(border_cell.style().fg, Some(Color::Cyan));

        // The selected provider row (row 4, first list item) must use the
        // yellow-on-black selection style since its panel has focus.
        let selected_cell = buffer.cell((1, 4)).unwrap();
        assert_eq!(selected_cell.style().bg, Some(Color::Yellow));
    }

    #[test]
    fn status_bar_uses_the_header_accent_and_colors_on_off_state() {
        let mut providers = IndexMap::new();
        providers.insert(
            "a".to_string(),
            Provider {
                base_url: "https://a.example.com".to_string(),
                api_key: "key".to_string(),
                models: vec!["m1".to_string()],
                ..Default::default()
            },
        );
        let last = Last { rtk_enabled: true, ..Default::default() };
        let state = AppState::new(Config { providers }, &last);

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame, &state)).unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
        assert!(content.contains("RTK: ON"));

        // Find the cell holding the 'O' of the RTK "ON" value and confirm it
        // renders in green, distinct from the muted label text next to it.
        // Per-cell symbols, not a joined String: the border glyph is
        // multi-byte UTF-8, so String::find's byte offset wouldn't line up
        // with the column index cell() expects.
        let cols: Vec<&str> = (0..80).map(|x| buffer.cell((x, 1)).unwrap().symbol()).collect();
        let on_col = cols
            .windows(2)
            .position(|w| w == ["O", "N"])
            .expect("RTK ON value should be on the status row") as u16;
        let on_cell = buffer.cell((on_col, 1)).unwrap();
        assert_eq!(on_cell.style().fg, Some(Color::Green));
    }
}
