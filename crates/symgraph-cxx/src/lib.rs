pub mod modules;

use clang::{Entity, EntityKind, TranslationUnit};
use serde::Serialize;

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
