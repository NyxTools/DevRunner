use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum ServiceStatus {
    #[default]
    Stopped,
    Running(u32),
    Failed,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ProjectType {
    Node,
    Rust,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Service {
    pub name: String,
    pub path: PathBuf,
    pub project_type: ProjectType,
    pub command: String,
    #[serde(skip)]
    pub status: ServiceStatus,
    #[serde(skip)]
    pub logs: Vec<String>,
}

impl Service {
    pub fn new(name: String, path: PathBuf, project_type: ProjectType, command: String) -> Self {
        Self {
            name,
            path,
            project_type,
            command,
            status: ServiceStatus::Stopped,
            logs: Vec::new(),
        }
    }
}
