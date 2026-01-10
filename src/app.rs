use crate::events::Event;
use crate::models::{Service, ServiceStatus};
use crate::process::ProcessManager;
use crate::ui;
use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event as CEvent, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::sync::Arc;
use tokio::sync::mpsc;
use std::time::Duration;
use sysinfo::System;

pub struct App {
    pub services: Vec<Service>,
    pub selected_index: usize,
    pub title: String,
    pub cpu_history: Vec<u64>,
}

impl App {
    pub fn new(services: Vec<Service>) -> Self {
        Self {
            services,
            selected_index: 0,
            title: "DevRunner".to_string(),
            cpu_history: vec![0; 40],
        }
    }

    pub fn next(&mut self) {
        if !self.services.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.services.len();
        }
    }

    pub fn previous(&mut self) {
        if !self.services.is_empty() {
            if self.selected_index > 0 {
                self.selected_index -= 1;
            } else {
                self.selected_index = self.services.len() - 1;
            }
        }
    }
    
    pub fn on_tick(&mut self, sys: &mut System) {
        sys.refresh_cpu_usage();
        sys.refresh_memory();
        let usage = sys.global_cpu_usage() as u64;
        
        self.cpu_history.push(usage);
        if self.cpu_history.len() > 40 {
            self.cpu_history.remove(0);
        }
    }
}

pub async fn run_app(services: Vec<Service>) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let (tx, mut rx) = mpsc::unbounded_channel();
    let process_manager = Arc::new(ProcessManager::new(tx.clone()));

    let mut sys = System::new_all();
    sys.refresh_all();

    let tick_rate = Duration::from_millis(500);
    let tx_tick = tx.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tick_rate).await;
            if tx_tick.send(Event::Tick).is_err() {
                break;
            }
        }
    });

    let tx_input = tx.clone();
    std::thread::spawn(move || {
        loop {
            if event::poll(Duration::from_millis(250)).expect("Poll failed") {
                match event::read().expect("Read failed") {
                    CEvent::Key(key) => {
                        if tx_input.send(Event::Key(key)).is_err() { break; }
                    }
                    CEvent::Mouse(mouse) => {
                         if tx_input.send(Event::Mouse(mouse)).is_err() { break; }
                    }
                    _ => {}
                }
            }
        }
    });

    let mut app = App::new(services);

    loop {
        terminal.draw(|f| ui::draw(f, &app.services, app.selected_index, &app.title, &app.cpu_history, &mut sys))?;

        if let Some(event) = rx.recv().await {
            match event {
                Event::Tick => {
                    app.on_tick(&mut sys);
                }
                Event::Key(key) => {
                    match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Down | KeyCode::Char('j') => app.next(),
                        KeyCode::Up | KeyCode::Char('k') => app.previous(),
                        KeyCode::Enter | KeyCode::Char('s') => {
                             if let Some(service) = app.services.get_mut(app.selected_index) {
                                 match service.status {
                                     ServiceStatus::Stopped | ServiceStatus::Failed | ServiceStatus::Completed => {
                                         let service_clone = service.clone();
                                         let pm = process_manager.clone();
                                         tokio::spawn(async move {
                                             let _ = pm.spawn_service(service_clone).await;
                                         });
                                     },
                                     ServiceStatus::Running(_) => {}
                                 }
                             }
                        }
                        _ => {}
                    }
                }
                Event::ServiceLog(name, line) => {
                    if let Some(service) = app.services.iter_mut().find(|s| s.name == name) {
                        service.logs.push(line);
                    }
                }
                Event::ServiceStatus(name, status) => {
                     if let Some(service) = app.services.iter_mut().find(|s| s.name == name) {
                        service.status = status;
                    }
                }
                Event::Quit => break,
                _ => {}
            }
        }
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
