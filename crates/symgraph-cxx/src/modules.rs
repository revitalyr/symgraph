
use anyhow::Result;
use regex::Regex;
use std::fs;

pub struct ModuleInfo {
    pub name: String,
    pub path: String,
    pub imports: Vec<String>,
}

pub fn scan_cpp20_module(file_path: &str) -> Result<Option<ModuleInfo>> {
    let text = fs::read_to_string(file_path)?;
    let re_export = Regex::new(r#"(?m)^\s*export\s+module\s+([A-Za-z0-9_:.]+)\s*;"#)?;
    let re_import = Regex::new(r#"(?m)^\s*import\s+([A-Za-z0-9_:.]+)\s*;"#)?;

    if let Some(cap) = re_export.captures(&text) {
        let name = cap.get(1).unwrap().as_str().to_string();
        let imports = re_import.captures_iter(&text)
            .filter_map(|m| m.get(1).map(|s| s.as_str().to_string()))
            .collect();
        Ok(Some(ModuleInfo { name, path: file_path.to_string(), imports }))
    } else {
        Ok(None)
    }
}

/// Внутренняя функция для тестирования без файловой системы
pub fn scan_cpp20_module_from_text(text: &str, path: &str) -> Option<ModuleInfo> {
    let re_export = Regex::new(r#"(?m)^\s*export\s+module\s+([A-Za-z0-9_:.]+)\s*;"#).ok()?;
    let re_import = Regex::new(r#"(?m)^\s*import\s+([A-Za-z0-9_:.]+)\s*;"#).ok()?;

    if let Some(cap) = re_export.captures(text) {
        let name = cap.get(1).unwrap().as_str().to_string();
        let imports = re_import.captures_iter(text)
            .filter_map(|m| m.get(1).map(|s| s.as_str().to_string()))
            .collect();
        Some(ModuleInfo { name, path: path.to_string(), imports })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Демонстрация: парсинг простого C++20 модуля
    #[test]
    fn test_simple_module() {
        let source = r#"
export module foo;

void hello() {}
"#;
        let mi = scan_cpp20_module_from_text(source, "foo.cppm").unwrap();
        assert_eq!(mi.name, "foo");
        assert_eq!(mi.path, "foo.cppm");
        assert!(mi.imports.is_empty());
    }

    /// Демонстрация: модуль с импортами
    #[test]
    fn test_module_with_imports() {
        let source = r#"
export module myapp;

import std;
import mylib;
import utils.io;

void run() {}
"#;
        let mi = scan_cpp20_module_from_text(source, "src/myapp.cppm").unwrap();
        assert_eq!(mi.name, "myapp");
        assert_eq!(mi.imports.len(), 3);
        assert!(mi.imports.contains(&"std".to_string()));
        assert!(mi.imports.contains(&"mylib".to_string()));
        assert!(mi.imports.contains(&"utils.io".to_string()));
    }

    /// Демонстрация: модуль с подмодулями (partitions)
    #[test]
    fn test_module_partitions() {
        let source = r#"
export module graphics:renderer;

import graphics:core;
import graphics:math;

class Renderer {};
"#;
        let mi = scan_cpp20_module_from_text(source, "graphics_renderer.cppm").unwrap();
        assert_eq!(mi.name, "graphics:renderer");
        assert_eq!(mi.imports.len(), 2);
        assert!(mi.imports.contains(&"graphics:core".to_string()));
        assert!(mi.imports.contains(&"graphics:math".to_string()));
    }

    /// Демонстрация: файл без export module (не модуль)
    #[test]
    fn test_not_a_module() {
        let source = r#"
#include <iostream>

int main() {
    std::cout << "Hello" << std::endl;
    return 0;
}
"#;
        let result = scan_cpp20_module_from_text(source, "main.cpp");
        assert!(result.is_none());
    }

    /// Демонстрация: модуль с комментариями
    #[test]
    fn test_module_with_comments() {
        let source = r#"
// This is a C++20 module
/* Multi-line
   comment */
export module mymodule;

// import commented; -- this should not be parsed
import realimport;

void func() {}
"#;
        let mi = scan_cpp20_module_from_text(source, "mymodule.cppm").unwrap();
        assert_eq!(mi.name, "mymodule");
        assert_eq!(mi.imports.len(), 1);
        assert_eq!(mi.imports[0], "realimport");
    }

    /// Демонстрация: модуль с пробелами и табуляцией
    #[test]
    fn test_module_with_whitespace() {
        let source = "  \texport module   spaced_module  ;\n\n  import   dep1;\n\timport dep2;";
        let mi = scan_cpp20_module_from_text(source, "test.cppm").unwrap();
        assert_eq!(mi.name, "spaced_module");
        assert_eq!(mi.imports.len(), 2);
    }

    /// Демонстрация: пустой модуль (только export module)
    #[test]
    fn test_empty_module() {
        let source = "export module empty;";
        let mi = scan_cpp20_module_from_text(source, "empty.cppm").unwrap();
        assert_eq!(mi.name, "empty");
        assert!(mi.imports.is_empty());
    }
}
