use crate::models::{ProjectType, Service};
use anyhow::Result;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

pub fn scan_directory(root: &Path) -> Result<Vec<Service>> {
    let mut services = Vec::new();

    // Shallow scan to avoid deep recursion into node_modules
    for entry in WalkDir::new(root)
        .max_depth(3)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        
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
        let important_scripts = ["dev", "start", "build"];
        
        for script in important_scripts {
             if let Some(_cmd) = scripts.get(script) {
                 services.push(Service::new(
                     format!("{}: {}", package_name, script),
                     dir_path.clone(),
                     ProjectType::Node,
                     format!("npm run {}", script),
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
