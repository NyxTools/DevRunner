use crate::models::{ProjectType, Service};
use anyhow::Result;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

pub fn scan_directory(root: &Path) -> Result<Vec<Service>> {
    let mut services = Vec::new();

    // Shallow scan or limited depth to avoid scanning node_modules too deeply if we were recursing
    // But for monorepos, we might need to go deep.
    // Let's use WalkDir with a filter.

    for entry in WalkDir::new(root)
        .max_depth(3) // Adjust depth as needed for monorepos
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        
        // Ignore node_modules and target directories
        if path.components().any(|c| c.as_os_str() == "node_modules" || c.as_os_str() == "target") {
            continue;
        }

        if path.file_name() == Some("package.json".as_ref()) {
            if let Ok(found_services) = parse_package_json(path) {
                services.extend(found_services);
            }
        } else if path.file_name() == Some("Cargo.toml".as_ref()) {
             if let Ok(found_services) = parse_cargo_toml(path) {
                services.extend(found_services);
             }
        }
    }

    Ok(services)
}

fn parse_package_json(path: &Path) -> Result<Vec<Service>> {
    let content = fs::read_to_string(path)?;
    let json: serde_json::Value = serde_json::from_str(&content)?;

    let mut services = Vec::new();
    let dir_path = path.parent().unwrap_or(path).to_path_buf();
    let package_name = json["name"].as_str().unwrap_or("unknown-js").to_string();

    if let Some(scripts) = json["scripts"].as_object() {
        // Simple heuristic: if it has "start" or "dev", it's likely a service.
        // If it has "build", we might want to offer that too.
        // For now, let's grab "dev", "start", and "build" if they exist.
        
        let important_scripts = ["dev", "start", "build"];
        
        for script in important_scripts {
             if let Some(_cmd) = scripts.get(script) {
                 services.push(Service::new(
                     format!("{}: {}", package_name, script),
                     dir_path.clone(),
                     ProjectType::Node,
                     format!("npm run {}", script), // TODO: Detect yarn/pnpm?
                 ));
             }
        }
    }

    Ok(services)
}

fn parse_cargo_toml(path: &Path) -> Result<Vec<Service>> {
    let content = fs::read_to_string(path)?;
    let toml_val: toml::Value = toml::from_str(&content)?;

    let mut services = Vec::new();
    let dir_path = path.parent().unwrap_or(path).to_path_buf();
    
    // Check if it's a binary package
    if let Some(package) = toml_val.get("package") {
        let name = package.get("name").and_then(|v| v.as_str()).unwrap_or("unknown-rust");
        
        // If it has [[bin]] or is just a package with main.rs (convention), we assume it can run.
        // For simplicity, let's just add a "run" command for every Cargo.toml that looks like a package.
        // We might want to check for "workspace" members vs actual crates.
        
        if toml_val.get("workspace").is_none() {
             services.push(Service::new(
                 format!("{}: run", name),
                 dir_path.clone(),
                 ProjectType::Rust,
                 "cargo run".to_string(),
             ));
        }
    }

    Ok(services)
}
