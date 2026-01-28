use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledProjectAnnotation {
    pub name: String,
    pub root_path: String,
    pub language: String,
    pub description: String,
    pub purpose: ProjectPurpose,
    pub build_system: BuildSystem,
    pub dependencies: Vec<String>,
    pub entry_points: Vec<String>,
    pub test_coverage: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProjectPurpose {
    Application,
    Library,
    Framework,
    Tool,
    Game,
    SystemSoftware,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BuildSystem {
    CMake,
    Make,
    Cargo,
    MSBuild,
    Ninja,
    Unknown,
}

pub fn analyze_cpp_project(root_path: &str, files: &[(String, String, String)]) -> Result<CompiledProjectAnnotation> {
    let name = extract_project_name_cpp(root_path);
    let purpose = infer_cpp_purpose(files);
    let build_system = detect_cpp_build_system(root_path);
    let entry_points = find_cpp_entry_points(files);
    let test_coverage = calculate_test_coverage(files);
    let dependencies = extract_cpp_dependencies(root_path)?;
    
    Ok(CompiledProjectAnnotation {
        name,
        root_path: root_path.to_string(),
        language: "C++".to_string(),
        description: generate_cpp_description(&purpose, files.len()),
        purpose,
        build_system,
        dependencies,
        entry_points,
        test_coverage,
    })
}

pub fn analyze_rust_project(root_path: &str, files: &[(String, String, String)]) -> Result<CompiledProjectAnnotation> {
    let name = extract_project_name_rust(root_path);
    let purpose = infer_rust_purpose(files);
    let entry_points = find_rust_entry_points(files);
    let test_coverage = calculate_test_coverage(files);
    let dependencies = extract_rust_dependencies(root_path)?;
    
    Ok(CompiledProjectAnnotation {
        name,
        root_path: root_path.to_string(),
        language: "Rust".to_string(),
        description: generate_rust_description(&purpose, files.len()),
        purpose,
        build_system: BuildSystem::Cargo,
        dependencies,
        entry_points,
        test_coverage,
    })
}

fn extract_project_name_cpp(root_path: &str) -> String {
    // Try CMakeLists.txt
    let cmake_path = Path::new(root_path).join("CMakeLists.txt");
    if let Ok(content) = std::fs::read_to_string(&cmake_path).map_err(|e| {
        log::debug!("Failed to read CMakeLists.txt from '{}': {}", cmake_path.display(), e);
        e
    }) {
        if let Some(start) = content.find("project(") {
            let after = &content[start + 8..];
            if let Some(end) = after.find(')') {
                let name = after[..end].trim().split_whitespace().next().unwrap_or("");
                if !name.is_empty() {
                    return name.to_string();
                }
            }
        }
    }
    
    Path::new(root_path).file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("cpp_project")
        .to_string()
}

fn extract_project_name_rust(root_path: &str) -> String {
    let cargo_path = Path::new(root_path).join("Cargo.toml");
    if let Ok(content) = std::fs::read_to_string(&cargo_path).map_err(|e| {
        log::debug!("Failed to read Cargo.toml from '{}': {}", cargo_path.display(), e);
        e
    }) {
        for line in content.lines() {
            if line.trim().starts_with("name") {
                if let Some(eq_pos) = line.find('=') {
                    let name = line[eq_pos + 1..].trim().trim_matches('"');
                    return name.to_string();
                }
            }
        }
    }
    
    Path::new(root_path).file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("rust_project")
        .to_string()
}

fn infer_cpp_purpose(files: &[(String, String, String)]) -> ProjectPurpose {
    let mut has_main = false;
    let mut has_lib = false;
    let mut has_game = false;
    
    for (path, category, _) in files {
        if category == "entrypoint" {
            has_main = true;
        }
        if path.to_lowercase().contains("lib") {
            has_lib = true;
        }
        if path.to_lowercase().contains("game") || path.to_lowercase().contains("engine") {
            has_game = true;
        }
    }
    
    if has_game {
        ProjectPurpose::Game
    } else if has_main && !has_lib {
        ProjectPurpose::Application
    } else if has_lib {
        ProjectPurpose::Library
    } else {
        ProjectPurpose::Unknown
    }
}

fn infer_rust_purpose(files: &[(String, String, String)]) -> ProjectPurpose {
    let mut has_main = false;
    let mut has_lib = false;
    
    for (path, category, _) in files {
        if category == "entrypoint" || path.ends_with("main.rs") {
            has_main = true;
        }
        if path.ends_with("lib.rs") {
            has_lib = true;
        }
    }
    
    if has_main && has_lib {
        ProjectPurpose::Tool
    } else if has_main {
        ProjectPurpose::Application
    } else if has_lib {
        ProjectPurpose::Library
    } else {
        ProjectPurpose::Unknown
    }
}

fn detect_cpp_build_system(root_path: &str) -> BuildSystem {
    let root = Path::new(root_path);
    
    if root.join("CMakeLists.txt").exists() {
        BuildSystem::CMake
    } else if root.join("Makefile").exists() {
        BuildSystem::Make
    } else if root.join("build.ninja").exists() {
        BuildSystem::Ninja
    } else {
        BuildSystem::Unknown
    }
}

fn find_cpp_entry_points(files: &[(String, String, String)]) -> Vec<String> {
    files.iter()
        .filter(|(_, category, _)| category == "entrypoint")
        .map(|(path, _, _)| path.clone())
        .collect()
}

fn find_rust_entry_points(files: &[(String, String, String)]) -> Vec<String> {
    files.iter()
        .filter(|(path, category, _)| {
            category == "entrypoint" || path.ends_with("main.rs") || path.ends_with("lib.rs")
        })
        .map(|(path, _, _)| path.clone())
        .collect()
}

fn calculate_test_coverage(files: &[(String, String, String)]) -> f32 {
    let total = files.len() as f32;
    let tests = files.iter()
        .filter(|(_, category, _)| category.contains("test"))
        .count() as f32;
    
    if total > 0.0 {
        (tests / total) * 100.0
    } else {
        0.0
    }
}

fn extract_cpp_dependencies(_root_path: &str) -> Result<Vec<String>> {
    // Simplified - could parse CMakeLists.txt for find_package calls
    Ok(vec![])
}

fn extract_rust_dependencies(root_path: &str) -> Result<Vec<String>> {
    let cargo_path = Path::new(root_path).join("Cargo.toml");
    let mut deps = Vec::new();
    
    if let Ok(content) = std::fs::read_to_string(&cargo_path).map_err(|e| {
        log::debug!("Failed to read Cargo.toml from '{}': {}", cargo_path.display(), e);
        e
    }) {
        let mut in_deps = false;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed == "[dependencies]" {
                in_deps = true;
                continue;
            }
            if trimmed.starts_with('[') && trimmed != "[dependencies]" {
                in_deps = false;
            }
            if in_deps && !trimmed.is_empty() && !trimmed.starts_with('#') {
                if let Some(eq_pos) = trimmed.find('=') {
                    let dep_name = trimmed[..eq_pos].trim();
                    deps.push(dep_name.to_string());
                }
            }
        }
    }
    
    Ok(deps)
}

fn generate_cpp_description(purpose: &ProjectPurpose, file_count: usize) -> String {
    let purpose_desc = match purpose {
        ProjectPurpose::Application => "a C++ application",
        ProjectPurpose::Library => "a C++ library",
        ProjectPurpose::Game => "a C++ game or engine",
        ProjectPurpose::Tool => "a C++ tool",
        _ => "a C++ project",
    };
    
    format!("This is {} with {} source files.", purpose_desc, file_count)
}

fn generate_rust_description(purpose: &ProjectPurpose, file_count: usize) -> String {
    let purpose_desc = match purpose {
        ProjectPurpose::Application => "a Rust application",
        ProjectPurpose::Library => "a Rust library crate",
        ProjectPurpose::Tool => "a Rust command-line tool",
        _ => "a Rust project",
    };
    
    format!("This is {} with {} source files.", purpose_desc, file_count)
}