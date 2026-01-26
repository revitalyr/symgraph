pub mod modules;

use clang::{Entity, EntityKind, TranslationUnit};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub enum FileCategory {
    EntryPoint,
    UnitTest,
    IntegrationTest,
    CoreLogic,
    Utility,
    Header,
    Implementation,
    Configuration,
    Unknown,
}

pub fn categorize_cpp_file(path: &str) -> FileCategory {
    let path_lower = path.to_lowercase();
    let filename = Path::new(path).file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    
    // Entry points
    if filename == "main.cpp" || filename == "main.c" || filename == "winmain.cpp" {
        return FileCategory::EntryPoint;
    }
    
    // Tests
    if path_lower.contains("test") || path_lower.contains("spec") 
        || filename.starts_with("test_") || filename.ends_with("_test.cpp")
        || filename.contains("gtest") || filename.contains("catch") {
        return FileCategory::UnitTest;
    }
    
    // Headers vs Implementation
    if filename.ends_with(".h") || filename.ends_with(".hpp") 
        || filename.ends_with(".hxx") || filename.ends_with(".hh") {
        return FileCategory::Header;
    }
    
    if filename.ends_with(".cpp") || filename.ends_with(".cc") 
        || filename.ends_with(".cxx") || filename.ends_with(".c") {
        return FileCategory::Implementation;
    }
    
    // Configuration
    if filename.ends_with(".cmake") || filename == "cmakelists.txt" 
        || filename.ends_with(".config") {
        return FileCategory::Configuration;
    }
    
    // Utilities
    if path_lower.contains("util") || path_lower.contains("helper") 
        || path_lower.contains("common") {
        return FileCategory::Utility;
    }
    
    FileCategory::Unknown
}

pub fn infer_cpp_purpose(path: &str, category: &FileCategory) -> String {
    let path_lower = path.to_lowercase();
    
    match category {
        FileCategory::EntryPoint => "Application entry point".to_string(),
        FileCategory::UnitTest => "Unit tests".to_string(),
        FileCategory::Header => "Header declarations".to_string(),
        FileCategory::Configuration => "Build configuration".to_string(),
        FileCategory::Utility => "Utility functions".to_string(),
        FileCategory::Implementation => {
            // Для Implementation проверяем эвристики по пути
            if path_lower.contains("network") || path_lower.contains("socket") {
                "Network operations".to_string()
            } else if path_lower.contains("database") || path_lower.contains("db") {
                "Database operations".to_string()
            } else if path_lower.contains("ui") || path_lower.contains("gui") {
                "User interface".to_string()
            } else {
                "Implementation code".to_string()
            }
        },
        _ => {
            if path_lower.contains("network") || path_lower.contains("socket") {
                "Network operations".to_string()
            } else if path_lower.contains("database") || path_lower.contains("db") {
                "Database operations".to_string()
            } else if path_lower.contains("ui") || path_lower.contains("gui") {
                "User interface".to_string()
            } else {
                "Core application logic".to_string()
            }
        }
    }
}

fn is_declaration_kind(kind: EntityKind) -> bool {
    matches!(
        kind,
        EntityKind::StructDecl
            | EntityKind::ClassDecl
            | EntityKind::EnumDecl
            | EntityKind::FieldDecl
            | EntityKind::FunctionDecl
            | EntityKind::VarDecl
            | EntityKind::ParmDecl
            | EntityKind::TypedefDecl
            | EntityKind::Method
            | EntityKind::Namespace
            | EntityKind::Constructor
            | EntityKind::Destructor
            | EntityKind::ClassTemplate
            | EntityKind::FunctionTemplate
            | EntityKind::UnionDecl
    )
}

fn is_expression_or_reference_kind(kind: EntityKind) -> bool {
    matches!(
        kind,
        EntityKind::CallExpr
            | EntityKind::DeclRefExpr
            | EntityKind::MemberRefExpr
            | EntityKind::TypeRef
            | EntityKind::TemplateRef
            | EntityKind::NamespaceRef
            | EntityKind::MemberRef
            | EntityKind::UnexposedExpr
    )
}

fn usr_to_string(entity: &Entity) -> Option<String> {
    entity.get_usr().map(|u| u.0.clone())
}

#[derive(Debug, Serialize)]
pub struct Symbol {
    pub usr: Option<String>,
    pub name: String,
    pub kind: String,
    pub is_definition: bool,
    pub file: String,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Serialize)]
pub struct Occurrence {
    pub usr: Option<String>,
    pub usage_kind: String,
    pub file: String,
    pub line: u32,
    pub column: u32,
}

pub fn scan_tu(
    tu: &TranslationUnit,
) -> (Vec<Symbol>, Vec<Occurrence>, Vec<(String, String, String)>) {
    let mut symbols = Vec::new();
    let mut occs = Vec::new();
    let mut edges = Vec::new();

    let root = tu.get_entity();
    root.visit_children(|entity, _parent| {
        let kind = entity.get_kind();

        if is_declaration_kind(kind) {
            let usr = usr_to_string(&entity);
            if let Some(loc) = entity.get_location() {
                let file_loc = loc.get_file_location();
                let file = file_loc
                    .file
                    .map(|f| f.get_path().display().to_string())
                    .unwrap_or_default();
                let line = file_loc.line;
                let col = file_loc.column;
                symbols.push(Symbol {
                    usr: usr.clone(),
                    name: entity.get_display_name().unwrap_or_default(),
                    kind: format!("{:?}", kind),
                    is_definition: entity.is_definition(),
                    file,
                    line,
                    column: col,
                });
            }
            if matches!(kind, EntityKind::FieldDecl | EntityKind::Method) {
                if let Some(owner) = entity.get_semantic_parent() {
                    let from = usr_to_string(&owner);
                    let to = usr_to_string(&entity);
                    if let (Some(f), Some(t)) = (from, to) {
                        edges.push(("member".to_string(), f, t));
                    }
                }
            }
            if kind == EntityKind::BaseSpecifier {
                if let Some(derived) = entity.get_semantic_parent().and_then(|p| usr_to_string(&p))
                {
                    if let Some(base) = entity.get_reference().and_then(|r| usr_to_string(&r)) {
                        edges.push(("inherit".to_string(), base, derived));
                    }
                }
            }
        }

        if is_expression_or_reference_kind(kind) {
            if let Some(target) = entity.get_reference() {
                let usr = usr_to_string(&target);
                if let Some(loc) = entity.get_location() {
                    let file_loc = loc.get_file_location();
                    let file = file_loc
                        .file
                        .map(|f| f.get_path().display().to_string())
                        .unwrap_or_default();
                    let line = file_loc.line;
                    let col = file_loc.column;
                    occs.push(Occurrence {
                        usr: usr.clone(),
                        usage_kind: classify_usage(&entity),
                        file,
                        line,
                        column: col,
                    });
                    if kind == EntityKind::CallExpr {
                        if let Some(caller) =
                            entity.get_semantic_parent().and_then(|p| usr_to_string(&p))
                        {
                            if let Some(callee) = usr.clone() {
                                edges.push(("call".to_string(), caller, callee));
                            }
                        }
                    }
                }
            }
        }

        clang::EntityVisitResult::Continue
    });

    (symbols, occs, edges)
}

fn classify_usage(entity: &Entity) -> String {
    match entity.get_kind() {
        EntityKind::CallExpr => "call",
        EntityKind::DeclRefExpr => "reference",
        EntityKind::MemberRefExpr => "member_ref",
        EntityKind::TypeRef => "type_ref",
        _ => "expr",
    }
    .to_string()
}
#[cfg(test)]
mod categorization_tests {
    use super::*;

    #[test]
    fn test_cpp_categorization() {
        assert_eq!(categorize_cpp_file("main.cpp"), FileCategory::EntryPoint);
        assert_eq!(categorize_cpp_file("utils.h"), FileCategory::Header);
        assert_eq!(categorize_cpp_file("utils.cpp"), FileCategory::Implementation);
        assert_eq!(categorize_cpp_file("test_utils.cpp"), FileCategory::UnitTest);
        assert_eq!(categorize_cpp_file("CMakeLists.txt"), FileCategory::Configuration);
        assert_eq!(categorize_cpp_file("src/network/client.cpp"), FileCategory::Implementation);
    }

    #[test]
    fn test_cpp_purpose_inference() {
        assert_eq!(infer_cpp_purpose("main.cpp", &FileCategory::EntryPoint), "Application entry point");
        assert_eq!(infer_cpp_purpose("utils.h", &FileCategory::Header), "Header declarations");
        assert_eq!(infer_cpp_purpose("test.cpp", &FileCategory::UnitTest), "Unit tests");
        // Для Implementation категории проверяем эвристики по пути
        assert_eq!(infer_cpp_purpose("src/network/net.cpp", &FileCategory::Implementation), "Network operations");
        assert_eq!(infer_cpp_purpose("src/database/db.cpp", &FileCategory::Implementation), "Database operations");
        assert_eq!(infer_cpp_purpose("src/ui/window.cpp", &FileCategory::Implementation), "User interface");
        assert_eq!(infer_cpp_purpose("src/core/app.cpp", &FileCategory::Implementation), "Implementation code");
    }
}