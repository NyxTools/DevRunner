use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, Paragraph, Wrap, Sparkline},
    Frame,
};
use tui_big_text::{BigText, PixelSize};
use crate::models::{Service, ServiceStatus};
use sysinfo::System;

pub fn draw(f: &mut Frame, services: &[Service], selected_index: usize, app_title: &str, cpu_history: &[u64], sys: &mut System) {
    // Refresh system info
    sys.refresh_all();
    
    let total_mem = sys.total_memory();
    let used_mem = sys.used_memory();


    // Color Palette matching the image
    let primary_color = Color::Cyan;
    let text_color = Color::White;
    let dimmed_color = Color::DarkGray;
    let highlight_color = Color::Cyan; // Selection highlight
    let graph_color = Color::Magenta; // Purple/Magenta for graph as seen in image

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),  // Header
            Constraint::Min(0),     // Main Content
            Constraint::Length(1),  // Footer
        ])
        .split(f.area());

    // 1. Header (ASCII Art centered + decorative lines if possible)
    let header_area = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(60),
            Constraint::Percentage(20),
        ])
        .split(chunks[0])[1];

    let title_line = BigText::builder()
        .pixel_size(PixelSize::Full)
        .style(Style::default().fg(primary_color))
        .lines(vec![app_title.into()])
        .build();
    
    f.render_widget(title_line, header_area);

    // 2. Main Content (3 Columns)
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25), // Services
            Constraint::Percentage(50), // Logs
            Constraint::Percentage(25), // Resources
        ])
        .split(chunks[1]);

    // --- LEFT COLUMN: SERVICES ---
    let items: Vec<ListItem> = services
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let is_selected = i == selected_index;
            
            // Symbols matching the image roughly
            let (status_symbol, color) = match s.status {
                ServiceStatus::Running(_) => ("[●]", primary_color),
                ServiceStatus::Failed => ("[✖]", Color::Red),
                ServiceStatus::Stopped => ("[ ]", dimmed_color),
                ServiceStatus::Completed => ("[✔]", Color::Green),
            };

            let bg_color = if is_selected { highlight_color } else { Color::Reset };
            let fg_color = if is_selected { Color::Black } else { text_color }; // Black text on Cyan highlight

            let line = Line::from(vec![
                Span::styled(format!("{} ", status_symbol), Style::default().fg(if is_selected { Color::Black } else { color })),
                Span::raw(&s.name),
            ]);

            ListItem::new(line).style(Style::default().bg(bg_color).fg(fg_color))
        })
        .collect();

    let sidebar_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(primary_color))
        .title(" SERVICES ");

    let list = List::new(items).block(sidebar_block);
    f.render_widget(list, main_chunks[0]);

    // --- CENTER COLUMN: LOGS ---
    let selected_service = services.get(selected_index);
    let log_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(primary_color))
        .title(" LOGS ");

    if let Some(service) = selected_service {
        let logs: Vec<Line> = service.logs
            .iter()
            .rev()
            .take(50)
            .rev()
            .map(|s| {
                if s.contains("[INFO]") {
                    Line::styled(s.as_str(), Style::default().fg(Color::Green))
                } else if s.contains("[ERROR]") {
                    Line::styled(s.as_str(), Style::default().fg(Color::Red))
                } else if s.contains("[EXEC]") {
                    Line::styled(s.as_str(), Style::default().fg(Color::Yellow))
                } else {
                    Line::styled(s.as_str(), Style::default().fg(text_color))
                }
            })
            .collect();
            
        let paragraph = Paragraph::new(logs)
            .block(log_block)
            .wrap(Wrap { trim: false });
            
        f.render_widget(paragraph, main_chunks[1]);
    } else {
        f.render_widget(Block::default().borders(Borders::ALL).title(" LOGS "), main_chunks[1]);
    }

    // --- RIGHT COLUMN: RESOURCES ---
    let resources_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(primary_color))
        .title(" RESOURCES ");
    
    let resources_area = resources_block.inner(main_chunks[2]);
    f.render_widget(resources_block, main_chunks[2]);

    let resource_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // CPU Label
            Constraint::Length(10), // Graph
            Constraint::Length(1), // Spacer
            Constraint::Length(4), // Memory details
        ])
        .split(resources_area);

    // CPU Label
    let current_cpu = cpu_history.last().unwrap_or(&0);
    let cpu_label = Line::from(vec![
        Span::raw("CPU "),
        Span::styled(format!("{:>3}%", current_cpu), Style::default().add_modifier(Modifier::BOLD)),
    ]);
    f.render_widget(Paragraph::new(cpu_label), resource_chunks[0]);

    // CPU Graph (Sparkline)
    let sparkline = Sparkline::default()
        .block(Block::default().borders(Borders::NONE))
        .data(cpu_history)
        .style(Style::default().fg(graph_color));
    f.render_widget(sparkline, resource_chunks[1]);

    // Memory Details
    let mem_text = vec![
        Line::from("Memory"),
        Line::from(vec![
            Span::styled(format!("{:.1}MiB", used_mem as f64 / 1024.0 / 1024.0), Style::default().fg(text_color)),
        ]),
        Line::from(vec![
            Span::raw(format!("{}/{}GB", used_mem / 1024 / 1024 / 1024, total_mem / 1024 / 1024 / 1024)),
        ]),
    ];
    let mem_paragraph = Paragraph::new(mem_text);
    f.render_widget(mem_paragraph, resource_chunks[3]);


    // 3. Footer (Simple help line)
    let footer_text = "[Q] Quit | [S/Enter] Start | [J/K] Move | [H] Help";
    let footer = Paragraph::new(footer_text)
        .style(Style::default().fg(text_color).bg(Color::Black));
    f.render_widget(footer, chunks[2]);
}
