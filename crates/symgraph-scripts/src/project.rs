use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use crate::{FileInfo, FileCategory};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectAnnotation {
    pub name: String,
    pub root_path: String,
    pub description: String,
    pub purpose: ProjectPurpose,
    pub structure: ProjectStructure,
    pub dependencies: Vec<Dependency>,
    pub entry_points: Vec<String>,
    pub test_coverage: TestCoverage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProjectPurpose {
    WebApplication,
    WebAPI,
    DesktopApplication,
    Library,
    CLI,
    DataProcessing,
    MachineLearning,
    GameDevelopment,
    MobileApp,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectStructure {
    pub architecture: ArchitecturePattern,
    pub layers: Vec<Layer>,
    pub modules: Vec<ModuleInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ArchitecturePattern {
    MVC,
    MVP,
    MVVM,
    Layered,
    Microservices,
    Monolithic,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layer {
    pub name: String,
    pub purpose: String,
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleInfo {
    pub name: String,
    pub path: String,
    pub purpose: String,
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub name: String,
    pub version: Option<String>,
    pub dep_type: DependencyType,
    pub purpose: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DependencyType {
    Runtime,
    Development,
    Testing,
    Build,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCoverage {
    pub has_unit_tests: bool,
    pub has_integration_tests: bool,
    pub test_frameworks: Vec<String>,
    pub coverage_estimate: f32,
}

pub struct ProjectAnalyzer;

impl ProjectAnalyzer {
    pub fn analyze_project(root_path: &str, files: &[FileInfo]) -> Result<ProjectAnnotation> {
        let name = Self::extract_project_name(root_path, files);
        let purpose = Self::infer_project_purpose(files);
        let structure = Self::analyze_structure(files);
        let dependencies = Self::extract_dependencies(root_path, files)?;
        let entry_points = Self::find_entry_points(files);
        let test_coverage = Self::analyze_test_coverage(files);
        let description = Self::generate_description(&purpose, &structure, files);

        Ok(ProjectAnnotation {
            name,
            root_path: root_path.to_string(),
            description,
            purpose,
            structure,
            dependencies,
            entry_points,
            test_coverage,
        })
    }

    fn extract_project_name(root_path: &str, files: &[FileInfo]) -> String {
        // Try package.json first
        if let Some(package_json) = files.iter().find(|f| f.path.ends_with("package.json")) {
            if let Ok(content) = std::fs::read_to_string(&package_json.path) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(name) = json.get("name").and_then(|n| n.as_str()) {
                        return name.to_string();
                    }
                }
            }
        }

        // Try setup.py
        if let Some(setup_py) = files.iter().find(|f| f.path.ends_with("setup.py")) {
            if let Ok(content) = std::fs::read_to_string(&setup_py.path) {
                if let Some(start) = content.find("name=") {
                    let name_part = &content[start + 5..];
                    if let Some(end) = name_part.find(',') {
                        let name = name_part[..end].trim().trim_matches('"').trim_matches('\'');
                        return name.to_string();
                    }
                }
            }
        }

        // Fallback to directory name
        Path::new(root_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string()
    }

    fn infer_project_purpose(files: &[FileInfo]) -> ProjectPurpose {
        let mut web_indicators = 0;
        let mut api_indicators = 0;
        let mut cli_indicators = 0;
        let mut lib_indicators = 0;
        let mut ml_indicators = 0;

        for file in files {
            let content_lower = file.path.to_lowercase();
            
            // Web application indicators
            if content_lower.contains("template") || content_lower.contains("static") 
                || content_lower.contains("html") || content_lower.contains("css") {
                web_indicators += 1;
            }

            // API indicators
            if content_lower.contains("api") || content_lower.contains("endpoint") 
                || content_lower.contains("route") {
                api_indicators += 1;
            }

            // CLI indicators
            if content_lower.contains("cli") || content_lower.contains("command") 
                || file.imports.iter().any(|i| i.contains("argparse") || i.contains("click")) {
                cli_indicators += 1;
            }

            // Library indicators
            if content_lower.contains("lib") || file.path.ends_with("__init__.py") {
                lib_indicators += 1;
            }

            // ML indicators
            if file.imports.iter().any(|i| 
                i.contains("tensorflow") || i.contains("pytorch") || i.contains("sklearn") 
                || i.contains("numpy") || i.contains("pandas")) {
                ml_indicators += 1;
            }
        }

        if ml_indicators > 0 {
            ProjectPurpose::MachineLearning
        } else if web_indicators > api_indicators && web_indicators > 0 {
            ProjectPurpose::WebApplication
        } else if api_indicators > 0 {
            ProjectPurpose::WebAPI
        } else if cli_indicators > 0 {
            ProjectPurpose::CLI
        } else if lib_indicators > 0 {
            ProjectPurpose::Library
        } else {
            ProjectPurpose::Unknown
        }
    }

    fn analyze_structure(files: &[FileInfo]) -> ProjectStructure {
        let architecture = Self::detect_architecture(files);
        let layers = Self::identify_layers(files);
        let modules = Self::identify_modules(files);

        ProjectStructure {
            architecture,
            layers,
            modules,
        }
    }

    fn detect_architecture(files: &[FileInfo]) -> ArchitecturePattern {
        let has_models = files.iter().any(|f| f.path.contains("model"));
        let has_views = files.iter().any(|f| f.path.contains("view") || f.path.contains("template"));
        let has_controllers = files.iter().any(|f| f.path.contains("controller") || f.path.contains("handler"));

        if has_models && has_views && has_controllers {
            ArchitecturePattern::MVC
        } else if files.iter().any(|f| f.path.contains("service") || f.path.contains("layer")) {
            ArchitecturePattern::Layered
        } else {
            ArchitecturePattern::Unknown
        }
    }

    fn identify_layers(files: &[FileInfo]) -> Vec<Layer> {
        let mut layers = Vec::new();
        let mut layer_files: HashMap<String, Vec<String>> = HashMap::new();

        for file in files {
            let layer_name = if file.path.contains("model") || file.path.contains("entity") {
                "Data Layer"
            } else if file.path.contains("service") || file.path.contains("business") {
                "Business Layer"
            } else if file.path.contains("controller") || file.path.contains("handler") || file.path.contains("api") {
                "Presentation Layer"
            } else if file.path.contains("util") || file.path.contains("helper") {
                "Utility Layer"
            } else {
                continue;
            };

            layer_files.entry(layer_name.to_string())
                .or_insert_with(Vec::new)
                .push(file.path.clone());
        }

        for (name, files) in layer_files {
            let purpose = match name.as_str() {
                "Data Layer" => "Data models and persistence",
                "Business Layer" => "Business logic and services",
                "Presentation Layer" => "User interface and API endpoints",
                "Utility Layer" => "Helper functions and utilities",
                _ => "Unknown purpose",
            };

            layers.push(Layer {
                name,
                purpose: purpose.to_string(),
                files,
            });
        }

        layers
    }

    fn identify_modules(files: &[FileInfo]) -> Vec<ModuleInfo> {
        let mut modules = HashMap::new();

        for file in files {
            let path_parts: Vec<&str> = file.path.split('/').collect();
            if path_parts.len() > 1 {
                let module_name = path_parts[path_parts.len() - 2];
                modules.entry(module_name.to_string())
                    .or_insert_with(Vec::new)
                    .push(file);
            }
        }

        modules.into_iter().map(|(name, files)| {
            let path = files[0].path.rsplit('/').nth(1).unwrap_or("").to_string();
            let purpose = Self::infer_module_purpose(&name, &files);
            let dependencies = Self::extract_module_dependencies(&files);

            ModuleInfo {
                name,
                path,
                purpose,
                dependencies,
            }
        }).collect()
    }

    fn infer_module_purpose(name: &str, files: &[&FileInfo]) -> String {
        match name {
            "auth" | "authentication" => "User authentication and authorization",
            "api" => "API endpoints and routing",
            "models" | "model" => "Data models and schemas",
            "services" | "service" => "Business logic services",
            "utils" | "utilities" => "Utility functions and helpers",
            "tests" | "test" => "Test cases and test utilities",
            "config" | "configuration" => "Configuration management",
            _ => {
                if files.iter().any(|f| matches!(f.category, FileCategory::UnitTest)) {
                    "Testing module"
                } else if files.iter().any(|f| matches!(f.category, FileCategory::CoreLogic)) {
                    "Core functionality module"
                } else {
                    "General purpose module"
                }
            }
        }.to_string()
    }

    fn extract_module_dependencies(files: &[&FileInfo]) -> Vec<String> {
        let mut deps = std::collections::HashSet::new();
        
        for file in files {
            for import in &file.imports {
                if let Some(module) = Self::extract_module_from_import(import) {
                    deps.insert(module);
                }
            }
        }
        
        deps.into_iter().collect()
    }

    fn extract_module_from_import(import: &str) -> Option<String> {
        if import.starts_with("from ") {
            import.split_whitespace().nth(1).map(|s| s.to_string())
        } else if import.starts_with("import ") {
            import.split_whitespace().nth(1).map(|s| s.split('.').next().unwrap().to_string())
        } else {
            None
        }
    }

    fn extract_dependencies(_root_path: &str, files: &[FileInfo]) -> Result<Vec<Dependency>> {
        let mut dependencies = Vec::new();

        // Python dependencies
        if let Some(requirements) = files.iter().find(|f| f.path.ends_with("requirements.txt")) {
            dependencies.extend(Self::parse_requirements_txt(&requirements.path)?);
        }

        // JavaScript dependencies
        if let Some(package_json) = files.iter().find(|f| f.path.ends_with("package.json")) {
            dependencies.extend(Self::parse_package_json(&package_json.path)?);
        }

        Ok(dependencies)
    }

    fn parse_requirements_txt(path: &str) -> Result<Vec<Dependency>> {
        let content = std::fs::read_to_string(path)?;
        let mut deps = Vec::new();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let parts: Vec<&str> = line.split("==").collect();
            let name = parts[0].to_string();
            let version = if parts.len() > 1 { Some(parts[1].to_string()) } else { None };
            let purpose = Self::infer_dependency_purpose(&name);

            deps.push(Dependency {
                name,
                version,
                dep_type: DependencyType::Runtime,
                purpose,
            });
        }

        Ok(deps)
    }

    fn parse_package_json(path: &str) -> Result<Vec<Dependency>> {
        let content = std::fs::read_to_string(path)?;
        let json: serde_json::Value = serde_json::from_str(&content)?;
        let mut deps = Vec::new();

        if let Some(dependencies) = json.get("dependencies").and_then(|d| d.as_object()) {
            for (name, version) in dependencies {
                deps.push(Dependency {
                    name: name.clone(),
                    version: version.as_str().map(|s| s.to_string()),
                    dep_type: DependencyType::Runtime,
                    purpose: Self::infer_dependency_purpose(name),
                });
            }
        }

        if let Some(dev_deps) = json.get("devDependencies").and_then(|d| d.as_object()) {
            for (name, version) in dev_deps {
                deps.push(Dependency {
                    name: name.clone(),
                    version: version.as_str().map(|s| s.to_string()),
                    dep_type: DependencyType::Development,
                    purpose: Self::infer_dependency_purpose(name),
                });
            }
        }

        Ok(deps)
    }

    fn infer_dependency_purpose(name: &str) -> String {
        match name {
            n if n.contains("test") || n.contains("jest") || n.contains("mocha") => "Testing framework",
            n if n.contains("express") || n.contains("flask") || n.contains("django") => "Web framework",
            n if n.contains("react") || n.contains("vue") || n.contains("angular") => "Frontend framework",
            n if n.contains("database") || n.contains("sql") || n.contains("mongo") => "Database connectivity",
            n if n.contains("auth") => "Authentication",
            n if n.contains("log") => "Logging",
            n if n.contains("config") => "Configuration management",
            _ => "General purpose library",
        }.to_string()
    }

    fn find_entry_points(files: &[FileInfo]) -> Vec<String> {
        files.iter()
            .filter(|f| matches!(f.category, FileCategory::EntryPoint))
            .map(|f| f.path.clone())
            .collect()
    }

    fn analyze_test_coverage(files: &[FileInfo]) -> TestCoverage {
        let unit_tests = files.iter().filter(|f| matches!(f.category, FileCategory::UnitTest)).count();
        let integration_tests = files.iter().filter(|f| matches!(f.category, FileCategory::IntegrationTest)).count();
        let total_files = files.len();

        let mut frameworks = std::collections::HashSet::new();
        for file in files {
            for import in &file.imports {
                if import.contains("unittest") || import.contains("pytest") {
                    frameworks.insert("pytest/unittest".to_string());
                } else if import.contains("jest") || import.contains("mocha") {
                    frameworks.insert("jest/mocha".to_string());
                }
            }
        }

        let coverage_estimate = if total_files > 0 {
            ((unit_tests + integration_tests) as f32 / total_files as f32) * 100.0
        } else {
            0.0
        };

        TestCoverage {
            has_unit_tests: unit_tests > 0,
            has_integration_tests: integration_tests > 0,
            test_frameworks: frameworks.into_iter().collect(),
            coverage_estimate,
        }
    }

    fn generate_description(purpose: &ProjectPurpose, structure: &ProjectStructure, files: &[FileInfo]) -> String {
        let purpose_desc = match purpose {
            ProjectPurpose::WebApplication => "a web application",
            ProjectPurpose::WebAPI => "a web API service",
            ProjectPurpose::CLI => "a command-line tool",
            ProjectPurpose::Library => "a software library",
            ProjectPurpose::MachineLearning => "a machine learning project",
            _ => "a software project",
        };

        let arch_desc = match structure.architecture {
            ArchitecturePattern::MVC => " following MVC architecture pattern",
            ArchitecturePattern::Layered => " with layered architecture",
            _ => "",
        };

        let file_count = files.len();
        let lang_counts = Self::count_languages(files);
        let main_lang = lang_counts.iter().max_by_key(|(_, count)| *count).map(|(lang, _)| lang);

        format!(
            "This is {} written primarily in {}{}, containing {} files across {} layers.",
            purpose_desc,
            main_lang.unwrap_or(&"unknown".to_string()),
            arch_desc,
            file_count,
            structure.layers.len()
        )
    }

    fn count_languages(files: &[FileInfo]) -> HashMap<String, usize> {
        let mut counts = HashMap::new();
        for file in files {
            *counts.entry(file.language.clone()).or_insert(0) += 1;
        }
        counts
    }
}