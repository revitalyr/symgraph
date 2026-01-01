use anyhow::Result;
use regex::Regex;
use std::fs;

// Shared models
use symgraph_models::{
    ModuleAnalysis, ModuleInfo, Relation as GenericRelation, Symbol as GenericSymbol,
};

// Backwards-compatible aliases for existing code
pub type CppSymbol = GenericSymbol;
pub type CppRelation = GenericRelation;
pub fn scan_cpp20_module(file_path: &str) -> Result<Option<ModuleInfo>> {
    let text = fs::read_to_string(file_path)?;
    let re_export = Regex::new(r#"(?m)^\s*export\s+module\s+([A-Za-z0-9_:.]+)\s*;"#)?;
    let re_import = Regex::new(r#"(?m)^\s*import\s+([A-Za-z0-9_:.]+)\s*;"#)?;

    if let Some(cap) = re_export.captures(&text) {
        let name = cap.get(1).unwrap().as_str().to_string();
        let imports = re_import
            .captures_iter(&text)
            .filter_map(|m| m.get(1).map(|s| s.as_str().to_string()))
            .collect();
        Ok(Some(ModuleInfo {
            name,
            path: file_path.to_string(),
            imports,
        }))
    } else {
        Ok(None)
    }
}

/// Internal function for testing without file system
pub fn scan_cpp20_module_from_text(text: &str, path: &str) -> Option<ModuleInfo> {
    let re_export = Regex::new(r#"(?m)^\s*export\s+module\s+([A-Za-z0-9_:.]+)\s*;"#).ok()?;
    let re_import = Regex::new(r#"(?m)^\s*import\s+([A-Za-z0-9_:.]+)\s*;"#).ok()?;

    if let Some(cap) = re_export.captures(text) {
        let name = cap.get(1).unwrap().as_str().to_string();
        let imports = re_import
            .captures_iter(text)
            .filter_map(|m| m.get(1).map(|s| s.as_str().to_string()))
            .collect();
        Some(ModuleInfo {
            name,
            path: path.to_string(),
            imports,
        })
    } else {
        None
    }
}

/// Analyze a C++ module file and extract symbols
pub fn analyze_cpp_module(file_path: &str) -> Result<Option<ModuleAnalysis>> {
    let text = fs::read_to_string(file_path)?;
    analyze_cpp_module_from_text(&text, file_path)
}

/// Analyze C++ source text and extract symbols (for testing and direct use)
pub fn analyze_cpp_module_from_text(text: &str, path: &str) -> Result<Option<ModuleAnalysis>> {
    // First check if it's a module
    let re_export_module = Regex::new(r#"(?m)^\s*export\s+module\s+([A-Za-z0-9_:.]+)\s*;"#)?;
    let re_import = Regex::new(r#"(?m)^\s*import\s+([A-Za-z0-9_:.]+)\s*;"#)?;

    let module_name = if let Some(cap) = re_export_module.captures(text) {
        cap.get(1).unwrap().as_str().to_string()
    } else {
        return Ok(None);
    };

    let imports: Vec<String> = re_import
        .captures_iter(text)
        .filter_map(|m| m.get(1).map(|s| s.as_str().to_string()))
        .collect();

    // Remove comments and strings for cleaner parsing
    let clean_text = remove_comments_and_strings(text);

    let mut symbols = Vec::new();
    let mut relations = Vec::new();

    // Track current context for member detection
    let mut current_class: Option<String> = None;

    // Parse line by line with context
    for (line_num, line) in clean_text.lines().enumerate() {
        let line_num = (line_num + 1) as u32;
        let trimmed = line.trim();

        // Skip empty lines
        if trimmed.is_empty() {
            continue;
        }

        // Detect class/struct end
        if trimmed.starts_with("};") || trimmed == "}" {
            current_class = None;
            continue;
        }

        // Detect exported functions (free functions)
        if let Some(func) = parse_exported_function(trimmed) {
            symbols.push(CppSymbol {
                name: func.0.clone(),
                kind: "function".to_string(),
                signature: func.1,
                is_exported: true,
                line: line_num,
            });

            // Extract calls from function body would need more context
            // For now, we detect type references in parameters
            for type_ref in &func.2 {
                relations.push(CppRelation {
                    from_name: func.0.clone(),
                    to_name: type_ref.clone(),
                    kind: "type_ref".to_string(),
                });
            }
        }

        // Detect exported classes/structs
        if let Some((class_name, base_classes)) = parse_exported_class(trimmed) {
            symbols.push(CppSymbol {
                name: class_name.clone(),
                kind: if trimmed.contains("struct") {
                    "struct"
                } else {
                    "class"
                }
                .to_string(),
                signature: trimmed.to_string(),
                is_exported: true,
                line: line_num,
            });

            // Inheritance relations
            for base in base_classes {
                relations.push(CppRelation {
                    from_name: class_name.clone(),
                    to_name: base,
                    kind: "inherit".to_string(),
                });
            }

            current_class = Some(class_name);
        }

        // Detect member functions inside a class
        if let Some(ref class_name) = current_class {
            if let Some((method_name, signature, type_refs)) = parse_member_function(trimmed) {
                symbols.push(CppSymbol {
                    name: format!("{}::{}", class_name, method_name),
                    kind: "method".to_string(),
                    signature,
                    is_exported: true,
                    line: line_num,
                });

                relations.push(CppRelation {
                    from_name: method_name.clone(),
                    to_name: class_name.clone(),
                    kind: "member".to_string(),
                });

                for type_ref in type_refs {
                    relations.push(CppRelation {
                        from_name: format!("{}::{}", class_name, method_name),
                        to_name: type_ref,
                        kind: "type_ref".to_string(),
                    });
                }
            }

            // Detect member variables
            if let Some((var_name, var_type)) = parse_member_variable(trimmed) {
                let full_name = format!("{}::{}", class_name, var_name);
                symbols.push(CppSymbol {
                    name: full_name.clone(),
                    kind: "field".to_string(),
                    signature: trimmed.to_string(),
                    is_exported: true,
                    line: line_num,
                });

                relations.push(CppRelation {
                    from_name: var_name,
                    to_name: class_name.clone(),
                    kind: "member".to_string(),
                });

                relations.push(CppRelation {
                    from_name: full_name,
                    to_name: var_type,
                    kind: "type_ref".to_string(),
                });
            }
        }

        // Detect exported enums
        if let Some(enum_name) = parse_exported_enum(trimmed) {
            symbols.push(CppSymbol {
                name: enum_name,
                kind: "enum".to_string(),
                signature: trimmed.to_string(),
                is_exported: true,
                line: line_num,
            });
        }

        // Detect exported typedefs/using
        if let Some((alias_name, original_type)) = parse_exported_typedef(trimmed) {
            symbols.push(CppSymbol {
                name: alias_name.clone(),
                kind: "typedef".to_string(),
                signature: trimmed.to_string(),
                is_exported: true,
                line: line_num,
            });

            relations.push(CppRelation {
                from_name: alias_name,
                to_name: original_type,
                kind: "type_ref".to_string(),
            });
        }

        // Detect exported variables/constants
        if let Some((var_name, var_type)) = parse_exported_variable(trimmed) {
            symbols.push(CppSymbol {
                name: var_name.clone(),
                kind: "variable".to_string(),
                signature: trimmed.to_string(),
                is_exported: true,
                line: line_num,
            });

            relations.push(CppRelation {
                from_name: var_name,
                to_name: var_type,
                kind: "type_ref".to_string(),
            });
        }
    }

    Ok(Some(ModuleAnalysis {
        info: ModuleInfo {
            name: module_name,
            path: path.to_string(),
            imports,
        },
        symbols,
        relations,
    }))
}

/// Remove C/C++ comments and string literals for cleaner parsing
fn remove_comments_and_strings(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '/' => {
                if chars.peek() == Some(&'/') {
                    // Line comment - skip to end of line
                    while let Some(c) = chars.next() {
                        if c == '\n' {
                            result.push('\n');
                            break;
                        }
                    }
                } else if chars.peek() == Some(&'*') {
                    // Block comment - skip to */
                    chars.next(); // consume *
                    while let Some(c) = chars.next() {
                        if c == '*' && chars.peek() == Some(&'/') {
                            chars.next(); // consume /
                            result.push(' '); // replace with space to preserve tokens
                            break;
                        }
                    }
                } else {
                    result.push(c);
                }
            }
            '"' => {
                // String literal - skip to closing quote
                result.push('"');
                while let Some(c) = chars.next() {
                    result.push(c);
                    if c == '\\' {
                        if let Some(escaped) = chars.next() {
                            result.push(escaped);
                        }
                    } else if c == '"' {
                        break;
                    }
                }
            }
            '\'' => {
                // Char literal
                result.push('\'');
                while let Some(c) = chars.next() {
                    result.push(c);
                    if c == '\\' {
                        if let Some(escaped) = chars.next() {
                            result.push(escaped);
                        }
                    } else if c == '\'' {
                        break;
                    }
                }
            }
            _ => result.push(c),
        }
    }

    result
}

/// Parse exported function: export ReturnType function_name(params) { or ;
fn parse_exported_function(line: &str) -> Option<(String, String, Vec<String>)> {
    // Match: export [inline] [constexpr] ReturnType name(...)
    let re = Regex::new(
        r#"^\s*export\s+(?:inline\s+)?(?:constexpr\s+)?(?:static\s+)?([A-Za-z_][A-Za-z0-9_:<>,\s\*&]*?)\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(([^)]*)\)"#
    ).ok()?;

    if let Some(caps) = re.captures(line) {
        let return_type = caps.get(1)?.as_str().trim().to_string();
        let name = caps.get(2)?.as_str().to_string();
        let params = caps.get(3)?.as_str();

        // Extract type references from parameters
        let type_refs = extract_types_from_params(params);

        let signature = format!("{} {}({})", return_type, name, params);

        // Skip if it looks like a class/struct definition
        if return_type == "class" || return_type == "struct" || return_type == "enum" {
            return None;
        }

        Some((name, signature, type_refs))
    } else {
        None
    }
}

/// Parse exported class/struct: export class/struct Name : public Base {
fn parse_exported_class(line: &str) -> Option<(String, Vec<String>)> {
    let re = Regex::new(
        r#"^\s*export\s+(?:class|struct)\s+([A-Za-z_][A-Za-z0-9_]*)\s*(?::\s*(.+?))?\s*\{"#,
    )
    .ok()?;

    if let Some(caps) = re.captures(line) {
        let name = caps.get(1)?.as_str().to_string();
        let base_classes = if let Some(bases) = caps.get(2) {
            // Parse base classes: public Base1, protected Base2
            let re_base =
                Regex::new(r#"(?:public|protected|private)?\s*([A-Za-z_][A-Za-z0-9_:<>]*)"#)
                    .ok()?;
            re_base
                .captures_iter(bases.as_str())
                .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
                .collect()
        } else {
            Vec::new()
        };

        Some((name, base_classes))
    } else {
        None
    }
}

/// Parse member function inside a class
fn parse_member_function(line: &str) -> Option<(String, String, Vec<String>)> {
    // Match: [virtual] [static] ReturnType name(params) [const] [override] [= 0] ;
    let re = Regex::new(
        r#"^\s*(?:virtual\s+)?(?:static\s+)?(?:inline\s+)?(?:constexpr\s+)?([A-Za-z_][A-Za-z0-9_:<>,\s\*&]*?)\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(([^)]*)\)"#
    ).ok()?;

    if let Some(caps) = re.captures(line) {
        let return_type = caps.get(1)?.as_str().trim();
        let name = caps.get(2)?.as_str().to_string();
        let params = caps.get(3)?.as_str();

        // Skip constructors/destructors (no return type, name matches class)
        if return_type.is_empty() || return_type == "explicit" || name.starts_with('~') {
            return None;
        }

        let type_refs = extract_types_from_params(params);
        let signature = format!("{} {}({})", return_type, name, params);

        Some((name, signature, type_refs))
    } else {
        None
    }
}

/// Parse member variable: Type name;
fn parse_member_variable(line: &str) -> Option<(String, String)> {
    // Skip function declarations and access specifiers
    if line.contains('(')
        || line.contains("public:")
        || line.contains("private:")
        || line.contains("protected:")
        || line.starts_with("//")
        || line.starts_with("/*")
    {
        return None;
    }

    let re = Regex::new(
        r#"^\s*(?:mutable\s+)?(?:static\s+)?(?:const\s+)?([A-Za-z_][A-Za-z0-9_:<>,\s\*&]*?)\s+([A-Za-z_][A-Za-z0-9_]*)\s*(?:=.*)?;"#
    ).ok()?;

    if let Some(caps) = re.captures(line) {
        let var_type = caps.get(1)?.as_str().trim().to_string();
        let name = caps.get(2)?.as_str().to_string();

        // Skip if type looks like a keyword
        if [
            "return", "if", "else", "while", "for", "switch", "case", "break", "continue",
        ]
        .contains(&var_type.as_str())
        {
            return None;
        }

        Some((name, var_type))
    } else {
        None
    }
}

/// Parse exported enum
fn parse_exported_enum(line: &str) -> Option<String> {
    let re = Regex::new(r#"^\s*export\s+enum\s+(?:class\s+)?([A-Za-z_][A-Za-z0-9_]*)"#).ok()?;
    re.captures(line)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
}

/// Parse exported typedef/using
fn parse_exported_typedef(line: &str) -> Option<(String, String)> {
    // using Alias = Type;
    let re_using =
        Regex::new(r#"^\s*export\s+using\s+([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.+?)\s*;"#).ok()?;
    if let Some(caps) = re_using.captures(line) {
        let alias = caps.get(1)?.as_str().to_string();
        let original = caps.get(2)?.as_str().trim().to_string();
        return Some((alias, original));
    }

    // typedef Type Alias;
    let re_typedef =
        Regex::new(r#"^\s*export\s+typedef\s+(.+?)\s+([A-Za-z_][A-Za-z0-9_]*)\s*;"#).ok()?;
    if let Some(caps) = re_typedef.captures(line) {
        let original = caps.get(1)?.as_str().trim().to_string();
        let alias = caps.get(2)?.as_str().to_string();
        return Some((alias, original));
    }

    None
}

/// Parse exported variable
fn parse_exported_variable(line: &str) -> Option<(String, String)> {
    // export [inline] [constexpr] Type name = value;
    let re = Regex::new(
        r#"^\s*export\s+(?:inline\s+)?(?:constexpr\s+)?(?:const\s+)?([A-Za-z_][A-Za-z0-9_:<>,\s\*&]*?)\s+([A-Za-z_][A-Za-z0-9_]*)\s*(?:=.*)?;"#
    ).ok()?;

    if let Some(caps) = re.captures(line) {
        let var_type = caps.get(1)?.as_str().trim();
        let name = caps.get(2)?.as_str().to_string();

        // Skip if it looks like a function
        if line.contains('(') {
            return None;
        }

        // Skip class/struct/enum definitions
        if ["class", "struct", "enum", "namespace"].contains(&var_type) {
            return None;
        }

        Some((name, var_type.to_string()))
    } else {
        None
    }
}

/// Extract type names from parameter list
fn extract_types_from_params(params: &str) -> Vec<String> {
    let mut types = Vec::new();

    // Simple regex to extract type names (before the parameter name)
    let re = Regex::new(r#"([A-Za-z_][A-Za-z0-9_]*(?:::[A-Za-z_][A-Za-z0-9_]*)*)"#).ok();
    if let Some(re) = re {
        for cap in re.captures_iter(params) {
            if let Some(m) = cap.get(1) {
                let t = m.as_str();
                // Skip common keywords and primitive types
                if ![
                    "const", "volatile", "mutable", "int", "char", "bool", "void", "float",
                    "double", "long", "short", "unsigned", "signed", "auto", "decltype",
                    "typename", "class", "struct",
                ]
                .contains(&t)
                {
                    types.push(t.to_string());
                }
            }
        }
    }

    types
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
