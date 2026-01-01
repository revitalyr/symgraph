use anyhow::Result;
use regex::Regex;
use std::fs;
use std::path::Path;

use symgraph_models::{
    GenericRelation as Relation, GenericSymbol as Symbol, ModuleAnalysis, ModuleInfo,
};

/// Try to detect whether the file represents a Rust module and return basic info
pub fn scan_rust_module(file_path: &str) -> Result<Option<ModuleInfo>> {
    let text = fs::read_to_string(file_path)?;
    Ok(scan_rust_module_from_text(&text, file_path))
}

/// Internal (testable) scanner from text
pub fn scan_rust_module_from_text(text: &str, path: &str) -> Option<ModuleInfo> {
    // If the file contains `pub mod NAME` or `mod NAME` or any pub item, consider it a module
    let re_mod = Regex::new(r"(?m)^\s*(?:pub\s+)?mod\s+([A-Za-z0-9_]+)\b").ok()?;
    let re_use = Regex::new(r"(?m)^\s*(?:pub\s+)?use\s+([A-Za-z0-9_:]+)").ok()?;
    let re_pub_item = Regex::new(r"(?m)^\s*pub\s+(?:fn|struct|enum|type|const|static)\b").ok()?;

    let module_name = if let Some(cap) = re_mod.captures(text) {
        cap.get(1).unwrap().as_str().to_string()
    } else {
        // fallback to file stem if there is any public item or use
        if re_pub_item.is_match(text) || re_use.is_match(text) {
            Path::new(path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string()
        } else {
            return None;
        }
    };

    let imports = re_use
        .captures_iter(text)
        .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
        .collect();

    Some(ModuleInfo {
        name: module_name,
        path: path.to_string(),
        imports,
    })
}

/// Analyze a Rust source file and extract exported symbols/relations
pub fn analyze_rust_module(file_path: &str) -> Result<Option<ModuleAnalysis>> {
    let text = fs::read_to_string(file_path)?;
    analyze_rust_module_from_text(&text, file_path)
}

/// Text-based analyzer (useful for tests)
pub fn analyze_rust_module_from_text(text: &str, path: &str) -> Result<Option<ModuleAnalysis>> {
    // Determine module name using scan
    let module_info = scan_rust_module_from_text(text, path);
    let module_name = if let Some(mi) = &module_info {
        mi.name.clone()
    } else {
        return Ok(None);
    };

    // Clean text
    let clean = remove_comments_and_strings(text);

    let mut symbols: Vec<Symbol> = Vec::new();
    let mut relations: Vec<Relation> = Vec::new();

    // Track current impl block (for methods)
    let mut current_impl: Option<String> = None;

    // Match `pub fn` anywhere on the line (handles `impl S { pub fn ... }` inline)
    let re_pub_fn = Regex::new(r"pub\s+fn\s+([A-Za-z0-9_]+)\s*\(").unwrap();
    let re_pub_struct = Regex::new(r"^\s*pub\s+struct\s+([A-Za-z0-9_]+)").unwrap();
    let re_pub_enum = Regex::new(r"^\s*pub\s+enum\s+([A-Za-z0-9_]+)").unwrap();
    let re_pub_type = Regex::new(r"^\s*pub\s+type\s+([A-Za-z0-9_]+)\s*=\s*(.+);?").unwrap();
    let re_pub_const =
        Regex::new(r"^\s*pub\s+(?:const|static)\s+([A-Za-z0-9_]+)\s*:\s*([^=;]+)").unwrap();
    let re_impl = Regex::new(r"^\s*impl\s+(?:<[^>]*>\s*)?([A-Za-z0-9_:<>::]+)\s*\{").unwrap();
    let re_impl_end = Regex::new(r"^\s*}\s*$").unwrap();
    let _re_fn_in_impl = Regex::new(r"^\s*pub\s+fn\s+([A-Za-z0-9_]+)\s*\(").unwrap();

    for (i, line) in clean.lines().enumerate() {
        let ln = (i + 1) as u32;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Impl start (match even when `{` and content are on the same line)
        if let Some(cap) = re_impl.captures(trimmed) {
            let typ = cap.get(1).unwrap().as_str().to_string();
            current_impl = Some(typ);
            // do not `continue` — allow matching `pub fn` on the same line
        }
        if let Some(cap) = re_pub_fn.captures(trimmed) {
            let name = cap.get(1).unwrap().as_str().to_string();
            let full_name = if let Some(ref typ) = current_impl {
                format!("{}::{}", typ, name)
            } else {
                name.clone()
            };
            symbols.push(Symbol {
                name: full_name.clone(),
                kind: "function".to_string(),
                signature: trimmed.to_string(),
                is_exported: true,
                line: ln,
            });
        }

        if let Some(cap) = re_pub_struct.captures(trimmed) {
            let name = cap.get(1).unwrap().as_str().to_string();
            symbols.push(Symbol {
                name: name.clone(),
                kind: "struct".to_string(),
                signature: trimmed.to_string(),
                is_exported: true,
                line: ln,
            });
        }

        if let Some(cap) = re_pub_enum.captures(trimmed) {
            let name = cap.get(1).unwrap().as_str().to_string();
            symbols.push(Symbol {
                name: name.clone(),
                kind: "enum".to_string(),
                signature: trimmed.to_string(),
                is_exported: true,
                line: ln,
            });
        }

        if let Some(cap) = re_pub_type.captures(trimmed) {
            let name = cap.get(1).unwrap().as_str().to_string();
            let orig = cap
                .get(2)
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();
            symbols.push(Symbol {
                name: name.clone(),
                kind: "type".to_string(),
                signature: trimmed.to_string(),
                is_exported: true,
                line: ln,
            });
            relations.push(Relation {
                from_name: name,
                to_name: orig.trim().to_string(),
                kind: "type_ref".to_string(),
            });
        }

        if let Some(cap) = re_pub_const.captures(trimmed) {
            let name = cap.get(1).unwrap().as_str().to_string();
            let typ = cap.get(2).unwrap().as_str().trim().to_string();
            symbols.push(Symbol {
                name: name.clone(),
                kind: "constant".to_string(),
                signature: trimmed.to_string(),
                is_exported: true,
                line: ln,
            });
            relations.push(Relation {
                from_name: name,
                to_name: typ,
                kind: "type_ref".to_string(),
            });
        }

        // Impl end (either standalone `}` or `}` somewhere on the line)
        if current_impl.is_some() && (re_impl_end.is_match(trimmed) || trimmed.contains('}')) {
            current_impl = None;
            continue;
        }
    }

    // Top-level imports
    let re_use = Regex::new(r"(?m)^\s*(?:pub\s+)?use\s+([A-Za-z0-9_:]+)").unwrap();
    let imports: Vec<String> = re_use
        .captures_iter(text)
        .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
        .collect();

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

fn remove_comments_and_strings(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '/' => {
                if chars.peek() == Some(&'/') {
                    // line comment
                    while let Some(cc) = chars.next() {
                        if cc == '\n' {
                            result.push('\n');
                            break;
                        }
                    }
                } else if chars.peek() == Some(&'*') {
                    // block comment
                    chars.next();
                    while let Some(cc) = chars.next() {
                        if cc == '*' && chars.peek() == Some(&'/') {
                            chars.next();
                            result.push(' ');
                            break;
                        }
                    }
                } else {
                    result.push('/');
                }
            }
            '"' => {
                // string
                result.push('"');
                while let Some(cc) = chars.next() {
                    result.push(cc);
                    if cc == '\\' {
                        if let Some(esc) = chars.next() {
                            result.push(esc);
                        }
                    } else if cc == '"' {
                        break;
                    }
                }
            }
            '\'' => {
                result.push('\'');
                while let Some(cc) = chars.next() {
                    result.push(cc);
                    if cc == '\\' {
                        if let Some(esc) = chars.next() {
                            result.push(esc);
                        }
                    } else if cc == '\'' {
                        break;
                    }
                }
            }
            _ => result.push(c),
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_module_pub_items() {
        let s = "pub fn foo() {}\npub struct Bar;";
        let mi = scan_rust_module_from_text(s, "lib.rs").unwrap();
        assert_eq!(mi.name, "lib");
        assert!(mi.imports.is_empty());
    }

    #[test]
    fn test_analyze_simple() {
        let s = "pub fn hello() {}\npub struct Foo { pub x: i32 }\npub type MyInt = i32;\npub const C: i32 = 1;";
        let res = analyze_rust_module_from_text(s, "m.rs").unwrap().unwrap();
        assert!(res
            .symbols
            .iter()
            .any(|s| s.name == "hello" && s.kind == "function"));
        assert!(res
            .symbols
            .iter()
            .any(|s| s.name == "Foo" && s.kind == "struct"));
        assert!(res
            .symbols
            .iter()
            .any(|s| s.name == "MyInt" && s.kind == "type"));
        assert!(res
            .symbols
            .iter()
            .any(|s| s.name == "C" && s.kind == "constant"));
    }

    #[test]
    fn test_impl_methods() {
        let s = "pub struct S;\nimpl S { pub fn do_it(&self) {} }";
        let res = analyze_rust_module_from_text(s, "s.rs").unwrap().unwrap();
        assert!(res.symbols.iter().any(|s| s.name == "S::do_it"));
    }
}
