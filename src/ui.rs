use crate::app::{AppState, Modal, Panel};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
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

    let provider_items: Vec<ListItem> = state
        .providers
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let style = if i == state.provider_cursor {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(p.as_str())).style(style)
        })
        .collect();
    let providers_border = if state.focused_panel == Panel::Providers {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    frame.render_widget(
        List::new(provider_items).block(
            Block::default()
                .title("Providers")
                .borders(Borders::ALL)
                .border_style(providers_border),
        ),
        panels[0],
    );

    let model_items: Vec<ListItem> = state
        .models_for_focused_provider
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let style = if i == state.model_cursor {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(m.as_str())).style(style)
        })
        .collect();
    let models_border = if state.focused_panel == Panel::Models {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    frame.render_widget(
        List::new(model_items).block(
            Block::default()
                .title("Models")
                .borders(Borders::ALL)
                .border_style(models_border),
        ),
        panels[1],
    );
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
