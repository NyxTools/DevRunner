use crate::models::{Service, ServiceStatus};
use crate::events::Event;
use anyhow::Result;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc::UnboundedSender;
use chrono::Local;

pub struct ProcessManager {
    event_tx: UnboundedSender<Event>,
}

impl ProcessManager {
    pub fn new(event_tx: UnboundedSender<Event>) -> Self {
        Self { event_tx }
    }

    pub async fn spawn_service(&self, service: Service) -> Result<()> {
        let event_tx = self.event_tx.clone();
        let service_name = service.name.clone();
        let command_str = service.command.clone();
        let path = service.path.clone();

        event_tx.send(Event::ServiceStatus(service_name.clone(), ServiceStatus::Running(0)))?;

        tokio::spawn(async move {
            let child = if cfg!(target_os = "windows") {
                Command::new("cmd")
                    .args(&["/C", &command_str])
                    .current_dir(&path)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
            } else {
                Command::new("sh")
                    .args(&["-c", &command_str])
                    .current_dir(&path)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
            };

            let mut child = match child {
                Ok(c) => c,
                Err(e) => {
                    let timestamp = Local::now().format("%H:%M:%S").to_string();
                    let _ = event_tx.send(Event::ServiceLog(
                        service_name.clone(),
                        format!("{} [ERROR] Failed to start: {}", timestamp, e)
                    ));
                    let _ = event_tx.send(Event::ServiceStatus(service_name.clone(), ServiceStatus::Failed));
                    return;
                }
            };
            
            if let Some(id) = child.id() {
                let timestamp = Local::now().format("%H:%M:%S").to_string();
                let _ = event_tx.send(Event::ServiceStatus(service_name.clone(), ServiceStatus::Running(id)));
                let _ = event_tx.send(Event::ServiceLog(
                    service_name.clone(),
                    format!("{} [INFO] {} started successfully.", timestamp, service_name)
                ));
            }

            let stdout = child.stdout.take().expect("Failed to capture stdout");
            let stderr = child.stderr.take().expect("Failed to capture stderr");

            let mut stdout_reader = BufReader::new(stdout).lines();
            let mut stderr_reader = BufReader::new(stderr).lines();
            
            let name_clone = service_name.clone();
            let tx_clone = event_tx.clone();
            
            let stdout_task = tokio::spawn(async move {
                while let Ok(Some(line)) = stdout_reader.next_line().await {
                    let timestamp = Local::now().format("%H:%M:%S").to_string();
                    let _ = tx_clone.send(Event::ServiceLog(
                        name_clone.clone(),
                        format!("{} [EXEC] {}", timestamp, line)
                    ));
                }
            });
            
            let name_clone = service_name.clone();
            let tx_clone = event_tx.clone();
            
            let stderr_task = tokio::spawn(async move {
                while let Ok(Some(line)) = stderr_reader.next_line().await {
                    let timestamp = Local::now().format("%H:%M:%S").to_string();
                    let _ = tx_clone.send(Event::ServiceLog(
                        name_clone.clone(),
                        format!("{} [ERROR] {}", timestamp, line)
                    ));
                }
            });
            
            let status = child.wait().await;
            
            let _ = stdout_task.await;
            let _ = stderr_task.await;

            let timestamp = Local::now().format("%H:%M:%S").to_string();
            match status {
                Ok(s) => {
                    if s.success() {
                        let _ = event_tx.send(Event::ServiceLog(
                            service_name.clone(),
                            format!("{} [INFO] Process completed successfully.", timestamp)
                        ));
                        let _ = event_tx.send(Event::ServiceStatus(service_name, ServiceStatus::Completed));
                    } else {
                        let _ = event_tx.send(Event::ServiceLog(
                            service_name.clone(),
                            format!("{} [ERROR] Process failed with exit code: {:?}", timestamp, s.code())
                        ));
                        let _ = event_tx.send(Event::ServiceStatus(service_name, ServiceStatus::Failed));
                    }
                }
                Err(e) => {
                    let _ = event_tx.send(Event::ServiceLog(
                        service_name.clone(),
                        format!("{} [ERROR] Process error: {}", timestamp, e)
                    ));
                    let _ = event_tx.send(Event::ServiceStatus(service_name, ServiceStatus::Failed));
                }
            }
        });

        Ok(())
    }
}
