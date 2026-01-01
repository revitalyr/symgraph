use serde::{Deserialize, Serialize};

/// Basic information about a module/file
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleInfo {
    pub name: String,
    pub path: String,
    pub imports: Vec<String>,
}

/// Generic symbol representation usable for different languages
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Symbol {
    pub name: String,
    pub kind: String,
    pub signature: String,
    pub is_exported: bool,
    pub line: u32,
}

/// Generic relation between symbols
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Relation {
    pub from_name: String,
    pub to_name: String,
    pub kind: String,
}

/// Full analysis result for a module
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleAnalysis {
    pub info: ModuleInfo,
    pub symbols: Vec<Symbol>,
    pub relations: Vec<Relation>,
}

// Convenience re-exports / aliases for backward compatibility
pub use Relation as GenericRelation;
pub use Symbol as GenericSymbol;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_module_info() {
        let mi = ModuleInfo {
            name: "foo".to_string(),
            path: "foo.rs".to_string(),
            imports: vec!["std::io".to_string()],
        };
        let s = serde_json::to_string(&mi).unwrap();
        let got: ModuleInfo = serde_json::from_str(&s).unwrap();
        assert_eq!(mi, got);
    }

    #[test]
    fn symbol_and_relation() {
        let sym = Symbol {
            name: "foo".to_string(),
            kind: "fn".to_string(),
            signature: "fn foo()".to_string(),
            is_exported: true,
            line: 10,
        };
        let rel = Relation {
            from_name: "foo".to_string(),
            to_name: "Bar".to_string(),
            kind: "type_ref".to_string(),
        };
        let ma = ModuleAnalysis {
            info: ModuleInfo {
                name: "m".into(),
                path: "m.rs".into(),
                imports: vec![],
            },
            symbols: vec![sym.clone()],
            relations: vec![rel.clone()],
        };
        let s = serde_json::to_string(&ma).unwrap();
        let got: ModuleAnalysis = serde_json::from_str(&s).unwrap();
        assert_eq!(got.symbols[0], sym);
        assert_eq!(got.relations[0], rel);
    }
}
