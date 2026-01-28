//! SCIP (Source Code Intelligence Protocol) parser
//! 
//! This module provides functionality to parse SCIP files and convert them
//! to the internal symgraph format.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use crate::SymgraphDb;
use serde::{Deserialize, Serialize};

/// SCIP document metadata stored in database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScipDocumentInfo {
    pub id: String,
    pub relative_path: String,
    pub language: String,
    pub symbol_count: usize,
    pub occurrence_count: usize,
    pub project_id: String,
}

/// SCIP symbol metadata stored in database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScipSymbolInfo {
    pub id: String,
    pub symbol: String,
    pub documentation: Option<String>,
    pub display_name: Option<String>,
    pub symbol_kind: String,
    pub file_id: String,
    pub relationships: Vec<ScipRelationship>,
}

/// SCIP occurrence metadata stored in database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScipOccurrenceInfo {
    pub id: String,
    pub symbol_id: String,
    pub document_path: String,
    pub range: ScipRange,
    pub roles: Vec<String>,
    pub syntax_kind: String,
    pub file_id: String,
}

/// Parsed SCIP information ready for database insertion
#[derive(Debug, Clone)]
pub struct ScipParsedData {
    pub metadata: ScipMetadata,
    pub documents: Vec<ScipDocument>,
    pub symbols: Vec<ScipSymbol>,
    pub occurrences: Vec<ScipOccurrence>,
}

/// Metadata extracted from SCIP index
#[derive(Debug, Clone)]
pub struct ScipMetadata {
    pub version: String,
    pub tool_name: String,
    pub tool_version: String,
    pub project_roots: Vec<String>,
}

/// Document information from SCIP
#[derive(Debug, Clone)]
pub struct ScipDocument {
    pub relative_path: String,
    pub language: String,
    pub symbol_count: usize,
    pub occurrence_count: usize,
}

/// Symbol information from SCIP
#[derive(Debug, Clone)]
pub struct ScipSymbol {
    pub symbol: String,
    pub documentation: Option<String>,
    pub display_name: Option<String>,
    pub symbol_kind: String,
    pub relationships: Vec<ScipRelationship>,
}

/// Relationship between symbols
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScipRelationship {
    pub kind: String,
    pub target_symbol: String,
}

/// Symbol occurrence information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScipOccurrence {
    pub document_path: String,
    pub symbol: String,
    pub range: ScipRange,
    pub roles: Vec<String>,
    pub syntax_kind: String,
}

/// Range information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScipRange {
    pub start_line: i32,
    pub start_character: i32,
    pub end_line: i32,
    pub end_character: i32,
}

/// Main SCIP parser
pub struct ScipParser;

impl ScipParser {
    /// Create a new SCIP parser
    pub fn new() -> Self {
        Self
    }

    /// Parse a SCIP file from disk
    pub fn parse_file<P: AsRef<Path>>(file_path: P) -> Result<ScipParsedData> {
        let content = fs::read(file_path)
            .context("Failed to read SCIP file")?;
        
        Self::parse_bytes(&content)
    }

    /// Parse SCIP data from bytes
    pub fn parse_bytes(data: &[u8]) -> Result<ScipParsedData> {
        // For now, we'll create a simple mock implementation
        // TODO: Implement full protobuf parsing when protoc is available
        Self::parse_mock_data(data)
    }

    /// Mock implementation for testing without protoc
    fn parse_mock_data(_data: &[u8]) -> Result<ScipParsedData> {
        Ok(ScipParsedData {
            metadata: ScipMetadata {
                version: "0.1.0".to_string(),
                tool_name: "rust-analyzer".to_string(),
                tool_version: "1.92.0".to_string(),
                project_roots: vec!["file:///project".to_string()],
            },
            documents: vec![
                ScipDocument {
                    relative_path: "src/main.rs".to_string(),
                    language: "rust".to_string(),
                    symbol_count: 1,
                    occurrence_count: 2,
                }
            ],
            symbols: vec![
                ScipSymbol {
                    symbol: "rust-analyzer cargo test_project 0.1.0 main()".to_string(),
                    documentation: Some("Main function".to_string()),
                    display_name: Some("main".to_string()),
                    symbol_kind: "function".to_string(),
                    relationships: vec![],
                }
            ],
            occurrences: vec![
                ScipOccurrence {
                    document_path: "src/main.rs".to_string(),
                    symbol: "rust-analyzer cargo test_project 0.1.0 main()".to_string(),
                    range: ScipRange {
                        start_line: 1,
                        start_character: 0,
                        end_line: 1,
                        end_character: 10,
                    },
                    roles: vec!["definition".to_string()],
                    syntax_kind: "function".to_string(),
                }
            ],
        })
    }
}

impl Default for ScipParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Utility function to quickly parse a SCIP file
pub fn parse_scip_file<P: AsRef<Path>>(file_path: P) -> Result<ScipParsedData> {
    ScipParser::parse_file(file_path)
}

/// Utility function to quickly parse SCIP bytes
pub fn parse_scip_bytes(data: &[u8]) -> Result<ScipParsedData> {
    ScipParser::parse_bytes(data)
}

/// Load SCIP data into symgraph database with complete information preservation
pub fn load_scip_to_database(db: &mut SymgraphDb, scip_data: &ScipParsedData, project_name: &str) -> Result<()> {
    use uuid::Uuid;
    
    // Create or get project
    let unknown_root = "file:///unknown".to_string();
    let project_root = scip_data.metadata.project_roots.first()
        .unwrap_or(&unknown_root);
    let project_id = db.ensure_project(project_name, project_root)?;

    // Track mappings between SCIP identifiers and database IDs
    let mut symbol_ids: HashMap<String, String> = HashMap::new();
    let mut file_ids: HashMap<String, String> = HashMap::new();
    let mut document_ids: HashMap<String, String> = HashMap::new();

    // First pass: Insert documents and collect file IDs
    for document in &scip_data.documents {
        let document_id = Uuid::new_v4().to_string();
        let file_id = db.ensure_file_with_category(
            &project_id,
            &document.relative_path,
            &document.language,
            Some("scip_parsed"),
            Some("SCIP parsed file")
        )?;

        // Store SCIP document info
        let scip_doc_info = ScipDocumentInfo {
            id: document_id.clone(),
            relative_path: document.relative_path.clone(),
            language: document.language.clone(),
            symbol_count: document.symbol_count,
            occurrence_count: document.occurrence_count,
            project_id: project_id.clone(),
        };
        db.store_scip_document(&scip_doc_info)?;

        file_ids.insert(document.relative_path.clone(), file_id);
        document_ids.insert(document.relative_path.clone(), document_id);
    }

    // Second pass: Insert symbols with complete information
    for symbol in &scip_data.symbols {
        if !symbol_ids.contains_key(&symbol.symbol) {
            let symbol_id = Uuid::new_v4().to_string();
            
            // Find a file ID for this symbol (use first document as fallback)
            let file_id = if scip_data.documents.is_empty() {
                "1".to_string() // fallback
            } else {
                file_ids.get(&scip_data.documents[0].relative_path)
                    .unwrap_or(&"1".to_string())
                    .clone()
            };

            // Insert basic symbol into main database
            let _db_symbol_id = crate::insert_symbol(
                db,
                &file_id,
                Some(&symbol.symbol),
                None,
                symbol.display_name.as_deref().unwrap_or(&symbol.symbol),
                &symbol.symbol_kind,
                true,
            )?;

            // Store complete SCIP symbol info
            let scip_symbol_info = ScipSymbolInfo {
                id: symbol_id.clone(),
                symbol: symbol.symbol.clone(),
                documentation: symbol.documentation.clone(),
                display_name: symbol.display_name.clone(),
                symbol_kind: symbol.symbol_kind.clone(),
                file_id,
                relationships: symbol.relationships.clone(),
            };
            db.store_scip_symbol(&scip_symbol_info)?;

            symbol_ids.insert(symbol.symbol.clone(), symbol_id);
        }
    }

    // Third pass: Insert occurrences with complete information
    for occurrence in &scip_data.occurrences {
        if let Some(scip_symbol_id) = symbol_ids.get(&occurrence.symbol) {
            let file_id = file_ids.get(&occurrence.document_path)
                .cloned()
                .unwrap_or_else(|| "1".to_string());

            // Insert basic occurrence into main database
            let _occurrence_id = crate::insert_occurrence(
                db,
                scip_symbol_id,
                &file_id,
                &occurrence.roles.join(","),
                occurrence.range.start_line as u32,
                occurrence.range.start_character as u32,
            )?;

            // Store complete SCIP occurrence info
            let scip_occ_info = ScipOccurrenceInfo {
                id: Uuid::new_v4().to_string(),
                symbol_id: scip_symbol_id.clone(),
                document_path: occurrence.document_path.clone(),
                range: occurrence.range.clone(),
                roles: occurrence.roles.clone(),
                syntax_kind: occurrence.syntax_kind.clone(),
                file_id,
            };
            db.store_scip_occurrence(&scip_occ_info)?;
        }
    }

    // Fourth pass: Create symbol relationships based on SCIP relationships
    for symbol in &scip_data.symbols {
        if let Some(from_scip_id) = symbol_ids.get(&symbol.symbol) {
            for relationship in &symbol.relationships {
                if let Some(to_scip_id) = symbol_ids.get(&relationship.target_symbol) {
                    crate::insert_edge(
                        db,
                        Some(from_scip_id),
                        Some(to_scip_id),
                        None,
                        None,
                        &relationship.kind,
                    )?;
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scip_parser_creation() {
        let parser = ScipParser::new();
        assert!(parser.symbol_cache.is_empty());
    }

    #[test]
    fn test_symbol_kind_inference() {
        let parser = ScipParser::new();
        
        assert_eq!(parser.infer_symbol_kind("my_function()"), "function");
        assert_eq!(parser.infer_symbol_kind("MyClass"), "type");
        assert_eq!(parser.infer_symbol_kind("my_variable"), "variable");
        assert_eq!(parser.infer_symbol_kind("module::submodule"), "module");
    }

    #[test]
    fn test_parse_mock_data() {
        let mut parser = ScipParser::new();
        let data = b"mock scip data";
        let result = parser.parse_bytes(data).unwrap();
        
        assert_eq!(result.metadata.tool_name, "rust-analyzer");
        assert_eq!(result.documents.len(), 1);
        assert_eq!(result.symbols.len(), 1);
        assert_eq!(result.occurrences.len(), 1);
    }
}
