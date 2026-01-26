use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tree_sitter::{Parser};
use walkdir::WalkDir;

pub mod project;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileCategory {
    EntryPoint,
    UnitTest,
    IntegrationTest,
    CoreLogic,
    Utility,
    Configuration,
    Documentation,
    BuildScript,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub path: String,
    pub language: String,
    pub category: FileCategory,
    pub purpose: String,
    pub imports: Vec<String>,
    pub exports: Vec<String>,
    pub functions: Vec<String>,
    pub classes: Vec<String>,
}

pub struct ScriptAnalyzer {
    parsers: HashMap<String, Parser>,
}

impl ScriptAnalyzer {
    pub fn new() -> Result<Self> {
        let mut parsers = HashMap::new();
        
        let mut py_parser = Parser::new();
        py_parser.set_language(tree_sitter_python::language())?;
        parsers.insert("python".to_string(), py_parser);
        
        let mut js_parser = Parser::new();
        js_parser.set_language(tree_sitter_javascript::language())?;
        parsers.insert("javascript".to_string(), js_parser);
        
        let mut ts_parser = Parser::new();
        ts_parser.set_language(tree_sitter_typescript::language_typescript())?;
        parsers.insert("typescript".to_string(), ts_parser);
        
        Ok(Self { parsers })
    }

    pub fn analyze_project(&mut self, root_path: &str) -> Result<Vec<FileInfo>> {
        let mut files = Vec::new();
        
        for entry in WalkDir::new(root_path) {
            let entry = entry?;
            if entry.file_type().is_file() {
                if let Some(info) = self.analyze_file(entry.path())? {
                    files.push(info);
                }
            }
        }
        
        Ok(files)
    }

    fn analyze_file(&mut self, path: &Path) -> Result<Option<FileInfo>> {
        let ext = path.extension()
            .and_then(|s| s.to_str())
            .unwrap_or("");
            
        let language = match ext {
            "py" => "python",
            "js" | "mjs" => "javascript", 
            "ts" => "typescript",
            _ => return Ok(None),
        };

        let content = std::fs::read_to_string(path)?;
        let parser = self.parsers.get_mut(language).unwrap();
        
        let tree = parser.parse(&content, None).unwrap();
        let root = tree.root_node();
        
        let category = self.categorize_file(path, &content, language);
        let purpose = self.infer_purpose(path, &content, &category);
        
        let mut info = FileInfo {
            path: path.to_string_lossy().to_string(),
            language: language.to_string(),
            category,
            purpose,
            imports: Vec::new(),
            exports: Vec::new(),
            functions: Vec::new(),
            classes: Vec::new(),
        };
        
        self.extract_symbols(&mut info, &root, &content, language)?;
        
        Ok(Some(info))
    }

    fn categorize_file(&self, path: &Path, content: &str, language: &str) -> FileCategory {
        let path_str = path.to_string_lossy().to_lowercase();
        let filename = path.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        
        // Entry points
        if filename == "main.py" || filename == "app.py" || filename == "index.js" 
            || filename == "server.js" || filename == "__main__.py" {
            return FileCategory::EntryPoint;
        }
        
        // Tests
        if path_str.contains("test") || path_str.contains("spec") 
            || filename.starts_with("test_") || filename.ends_with("_test.py")
            || filename.ends_with(".test.js") || filename.ends_with(".spec.js") {
            if path_str.contains("integration") || path_str.contains("e2e") {
                return FileCategory::IntegrationTest;
            }
            return FileCategory::UnitTest;
        }
        
        // Configuration
        if filename.ends_with(".config.js") || filename == "setup.py" 
            || filename == "package.json" || filename == "requirements.txt"
            || filename == "pyproject.toml" {
            return FileCategory::Configuration;
        }
        
        // Build scripts
        if filename == "setup.py" || filename == "build.py" 
            || filename == "webpack.config.js" || filename == "gulpfile.js" {
            return FileCategory::BuildScript;
        }
        
        // Documentation
        if filename.ends_with(".md") || path_str.contains("doc") {
            return FileCategory::Documentation;
        }
        
        // Utilities (common patterns)
        if path_str.contains("util") || path_str.contains("helper") 
            || path_str.contains("common") || filename.starts_with("utils") {
            return FileCategory::Utility;
        }
        
        // Core logic heuristics
        if self.has_main_logic_patterns(content, language) {
            return FileCategory::CoreLogic;
        }
        
        FileCategory::Unknown
    }

    fn has_main_logic_patterns(&self, content: &str, language: &str) -> bool {
        match language {
            "python" => {
                content.contains("class ") && content.contains("def ") 
                    && !content.contains("import unittest")
                    && !content.contains("import pytest")
            },
            "javascript" | "typescript" => {
                (content.contains("class ") || content.contains("function "))
                    && !content.contains("describe(") 
                    && !content.contains("it(")
                    && !content.contains("test(")
            },
            _ => false,
        }
    }

    fn infer_purpose(&self, _path: &Path, content: &str, category: &FileCategory) -> String {
        match category {
            FileCategory::EntryPoint => "Application entry point".to_string(),
            FileCategory::UnitTest => "Unit tests".to_string(),
            FileCategory::IntegrationTest => "Integration tests".to_string(),
            FileCategory::Configuration => "Configuration file".to_string(),
            FileCategory::BuildScript => "Build automation script".to_string(),
            FileCategory::Documentation => "Documentation".to_string(),
            FileCategory::Utility => "Utility functions and helpers".to_string(),
            FileCategory::CoreLogic => {
                // Try to infer from content
                if content.contains("database") || content.contains("db") {
                    "Database operations".to_string()
                } else if content.contains("api") || content.contains("endpoint") {
                    "API endpoints".to_string()
                } else if content.contains("model") || content.contains("schema") {
                    "Data models".to_string()
                } else {
                    "Core application logic".to_string()
                }
            },
            FileCategory::Unknown => "Unknown purpose".to_string(),
        }
    }

    fn extract_symbols(&self, info: &mut FileInfo, root: &tree_sitter::Node, content: &str, language: &str) -> Result<()> {
        match language {
            "python" => self.extract_python_symbols(info, root, content),
            "javascript" => self.extract_javascript_symbols(info, root, content),
            "typescript" => self.extract_typescript_symbols(info, root, content),
            _ => Ok(()),
        }
    }

    fn extract_python_symbols(&self, info: &mut FileInfo, root: &tree_sitter::Node, content: &str) -> Result<()> {
        let mut cursor = root.walk();
        
        for child in root.children(&mut cursor) {
            match child.kind() {
                "import_statement" | "import_from_statement" => {
                    if let Ok(import_text) = child.utf8_text(content.as_bytes()) {
                        info.imports.push(import_text.to_string());
                    }
                },
                "function_definition" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        if let Ok(name) = name_node.utf8_text(content.as_bytes()) {
                            info.functions.push(name.to_string());
                        }
                    }
                },
                "class_definition" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        if let Ok(name) = name_node.utf8_text(content.as_bytes()) {
                            info.classes.push(name.to_string());
                        }
                    }
                },
                _ => {}
            }
        }
        
        Ok(())
    }

    fn extract_javascript_symbols(&self, info: &mut FileInfo, root: &tree_sitter::Node, content: &str) -> Result<()> {
        let mut cursor = root.walk();
        
        for child in root.children(&mut cursor) {
            match child.kind() {
                "import_statement" => {
                    if let Ok(import_text) = child.utf8_text(content.as_bytes()) {
                        info.imports.push(import_text.to_string());
                    }
                },
                "export_statement" => {
                    if let Ok(export_text) = child.utf8_text(content.as_bytes()) {
                        info.exports.push(export_text.to_string());
                    }
                },
                "function_declaration" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        if let Ok(name) = name_node.utf8_text(content.as_bytes()) {
                            info.functions.push(name.to_string());
                        }
                    }
                },
                "class_declaration" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        if let Ok(name) = name_node.utf8_text(content.as_bytes()) {
                            info.classes.push(name.to_string());
                        }
                    }
                },
                _ => {}
            }
        }
        
        Ok(())
    }

    fn extract_typescript_symbols(&self, info: &mut FileInfo, root: &tree_sitter::Node, content: &str) -> Result<()> {
        // TypeScript parsing similar to JavaScript but with additional type info
        self.extract_javascript_symbols(info, root, content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_categorize_entry_point() {
        let analyzer = ScriptAnalyzer::new().unwrap();
        let path = Path::new("main.py");
        let content = "def main():\n    print('Hello')";
        
        let category = analyzer.categorize_file(path, content, "python");
        assert!(matches!(category, FileCategory::EntryPoint));
    }

    #[test]
    fn test_categorize_test_file() {
        let analyzer = ScriptAnalyzer::new().unwrap();
        let path = Path::new("test_utils.py");
        let content = "import unittest\n\nclass TestUtils(unittest.TestCase):\n    pass";
        
        let category = analyzer.categorize_file(path, content, "python");
        assert!(matches!(category, FileCategory::UnitTest));
    }

    #[test]
    fn test_categorize_core_logic() {
        let analyzer = ScriptAnalyzer::new().unwrap();
        let path = Path::new("models.py");
        let content = "class User:\n    def __init__(self):\n        pass\n\ndef process_data():\n    pass";
        
        let category = analyzer.categorize_file(path, content, "python");
        assert!(matches!(category, FileCategory::CoreLogic));
    }
}