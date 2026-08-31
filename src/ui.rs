use crate::app::{AppState, Modal, Panel};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

pub fn render(frame: &mut Frame, state: &AppState) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(area);

    render_status(frame, state, chunks[0]);
    render_panels(frame, state, chunks[1]);
    render_footer(frame, chunks[2]);

    if let Some(modal) = &state.modal {
        render_modal(frame, modal, area);
    }
}

fn render_status(frame: &mut Frame, state: &AppState, area: Rect) {
    let provider = state.current_provider.as_deref().unwrap_or("-");
    let model = state.current_model.as_deref().unwrap_or("-");
    let text = format!(
        "Provider: {provider}   Model: {model}   RTK: {}   Auto-accept: {}",
        if state.rtk_enabled { "ON" } else { "OFF" },
        if state.auto_accept { "ON" } else { "OFF" }
    );
    let block = Block::default().title("Claude Code Swapper").borders(Borders::ALL);
    frame.render_widget(Paragraph::new(text).block(block), area);
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
                .title("Providers")
                .borders(Borders::ALL)
                .border_style(providers_border),
        ),
        panels[0],
        &mut provider_state,
    );

    let models_focused = state.focused_panel == Panel::Models;
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
            ListItem::new(Line::from(m.as_str())).style(style)
        })
        .collect();
    let models_border = panel_border_style(models_focused);
    let mut model_state = ListState::default().with_selected(Some(state.model_cursor));
    frame.render_stateful_widget(
        List::new(model_items).block(
            Block::default()
                .title("Models")
                .borders(Borders::ALL)
                .border_style(models_border),
        ),
        panels[1],
        &mut model_state,
    );
}

fn panel_border_style(focused: bool) -> Style {
    if focused {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    }
}

fn cursor_style(panel_focused: bool) -> Style {
    if panel_focused {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().add_modifier(Modifier::REVERSED)
    }
}

fn render_footer(frame: &mut Frame, area: Rect) {
    let text = "[Tab] switch  [↑/↓] move  [Enter] select  [l] launch  [n] native  [a] add  [x] remove  [s] api key  [r] rtk  [p] auto-accept  [q] quit";
    frame.render_widget(Paragraph::new(text), area);
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
        .title(format!("{title} ({provider})"))
        .borders(Borders::ALL);
    frame.render_widget(Paragraph::new(masked).block(block), popup);
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
    fn render_with_open_modal_does_not_panic() {
        let mut providers = IndexMap::new();
        providers.insert(
            "lmstudio".to_string(),
            Provider {
                base_url: "http://localhost:1234".to_string(),
                api_key: "lm-studio".to_string(),
                models: vec!["local-model".to_string()],
            },
        );
        let mut state = AppState::new(Config { providers }, &Last::default());
        state.open_add_model_modal();

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame, &state)).unwrap();
    }
}
