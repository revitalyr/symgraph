use anyhow::Result;
use serde::{Deserialize, Serialize};
use sled::Db;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub root_path: String,
    pub description: Option<String>,
    pub purpose: Option<String>,
    pub structure: Option<String>,
    pub dependencies: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Module {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub kind: String,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct File {
    pub id: String,
    pub project_id: String,
    pub module_id: Option<String>,
    pub path: String,
    pub lang: String,
    pub category: Option<String>,
    pub purpose: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub id: String,
    pub file_id: String,
    pub usr: Option<String>,
    pub key: Option<String>,
    pub name: String,
    pub kind: String,
    pub is_definition: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Occurrence {
    pub id: String,
    pub symbol_id: String,
    pub file_id: String,
    pub usage_kind: String,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub id: String,
    pub from_sym: Option<String>,
    pub to_sym: Option<String>,
    pub from_module: Option<String>,
    pub to_module: Option<String>,
    pub kind: String,
}

pub struct SymgraphDb {
    pub db: Db,
}

impl SymgraphDb {
    pub fn open(path: &str) -> Result<Self> {
        let db = sled::open(path).map_err(|e| {
            if e.to_string().contains("already exists") || e.to_string().contains("183") {
                anyhow::anyhow!("Failed to open database at '{}': Cannot create file when it already exists. This may indicate:\n\
                1. The database is already open by another process\n\
                2. Insufficient permissions to access the database directory\n\
                3. The database path is being used by another application\n\
                \nTry closing other applications that might be using the database or choose a different path.", path)
            } else if e.to_string().contains("IO") {
                anyhow::anyhow!("Failed to open database at '{}': IO error: {}", path, e)
            } else {
                anyhow::anyhow!("Failed to open database at '{}': {}", path, e)
            }
        })?;
        Ok(Self { db })
    }

    pub fn ensure_project(&mut self, name: &str, root_path: &str) -> Result<String> {
        let project_id = Uuid::new_v4().to_string();
        let created_at = chrono::Utc::now().to_rfc3339();
        
        let project = Project {
            id: project_id.clone(),
            name: name.to_string(),
            root_path: root_path.to_string(),
            description: None,
            purpose: None,
            structure: None,
            dependencies: None,
            created_at,
        };

        let key = format!("project:{}", root_path);
        if let Some(existing) = self.db.get(&key)? {
            let existing_project: Project = serde_json::from_slice(&existing)?;
            Ok(existing_project.id)
        } else {
            let value = serde_json::to_vec(&project)?;
            self.db.insert(&key, value.clone())?;
            self.db.insert(format!("project:{}", project.id), value)?;
            Ok(project_id)
        }
    }

    pub fn update_project_annotation(&mut self, project_id: &str, description: &str, purpose: &str, structure: &str, dependencies: &str) -> Result<()> {
        let key = format!("project:{}", project_id);
        if let Some(data) = self.db.get(&key)? {
            let mut project: Project = serde_json::from_slice(&data)?;
            project.description = Some(description.to_string());
            project.purpose = Some(purpose.to_string());
            project.structure = Some(structure.to_string());
            project.dependencies = Some(dependencies.to_string());
            
            let value = serde_json::to_vec(&project)?;
            self.db.insert(&key, value)?;
        }
        Ok(())
    }

    pub fn ensure_file_with_category(&mut self, project_id: &str, path: &str, lang: &str, category: Option<&str>, purpose: Option<&str>) -> Result<String> {
        let file_id = Uuid::new_v4().to_string();
        
        let file = File {
            id: file_id.clone(),
            project_id: project_id.to_string(),
            module_id: None,
            path: path.to_string(),
            lang: lang.to_string(),
            category: category.map(|s| s.to_string()),
            purpose: purpose.map(|s| s.to_string()),
        };

        let key = format!("file:{}", path);
        if let Some(existing) = self.db.get(&key)? {
            let existing_file: File = serde_json::from_slice(&existing)?;
            Ok(existing_file.id)
        } else {
            let value = serde_json::to_vec(&file)?;
            self.db.insert(&key, value.clone())?;
            self.db.insert(format!("file:{}", file.id), value)?;
            Ok(file_id)
        }
    }

    pub fn ensure_file(&mut self, path: &str, lang: &str) -> Result<String> {
        self.ensure_file_with_category("1", path, lang, None, None)
    }

    pub fn find_symbol_by_usr(&self, usr: &str) -> Result<Option<String>> {
        let key = format!("symbol_by_usr:{}", usr);
        if let Some(symbol_id) = self.db.get(&key)? {
            Ok(Some(String::from_utf8_lossy(&symbol_id).to_string()))
        } else {
            Ok(None)
        }
    }

    pub fn query_edges_by_kind_from(&self, kind: &str, from_usr: &str) -> Result<Vec<String>> {
        let mut result = Vec::new();
        
        if let Some(symbol_id) = self.find_symbol_by_usr(from_usr)? {
            let prefix = format!("edges_from:{}:{}:", symbol_id, kind);
            for item in self.db.scan_prefix(&prefix) {
                let (_, value) = item?;
                let edge: Edge = serde_json::from_slice(&value)?;
                if let Some(to_sym) = edge.to_sym {
                    let symbol_key = format!("symbol:{}", to_sym);
                    if let Some(symbol_data) = self.db.get(&symbol_key)? {
                        let symbol: Symbol = serde_json::from_slice(&symbol_data)?;
                        result.push(symbol.name);
                    }
                }
            }
        }
        
        Ok(result)
    }
}

pub fn insert_symbol(
    db: &mut SymgraphDb,
    file_id: &str,
    usr: Option<&str>,
    key: Option<&str>,
    name: &str,
    kind: &str,
    is_def: bool,
) -> Result<String> {
    let symbol_id = Uuid::new_v4().to_string();
    
    let symbol = Symbol {
        id: symbol_id.clone(),
        file_id: file_id.to_string(),
        usr: usr.map(|s| s.to_string()),
        key: key.map(|s| s.to_string()),
        name: name.to_string(),
        kind: kind.to_string(),
        is_definition: is_def,
    };

    let value = serde_json::to_vec(&symbol)?;
    db.db.insert(format!("symbol:{}", symbol_id), value.clone())?;
    
    if let Some(usr_val) = usr {
        db.db.insert(format!("symbol_by_usr:{}", usr_val), symbol_id.as_bytes())?;
    }
    
    Ok(symbol_id)
}

pub fn insert_occurrence(
    db: &mut SymgraphDb,
    sym_id: &str,
    file_id: &str,
    usage: &str,
    line: u32,
    col: u32,
) -> Result<String> {
    let occ_id = Uuid::new_v4().to_string();
    
    let occurrence = Occurrence {
        id: occ_id.clone(),
        symbol_id: sym_id.to_string(),
        file_id: file_id.to_string(),
        usage_kind: usage.to_string(),
        line,
        column: col,
    };

    let value = serde_json::to_vec(&occurrence)?;
    db.db.insert(format!("occurrence:{}", occ_id), value)?;
    
    Ok(occ_id)
}

pub fn insert_edge(
    db: &mut SymgraphDb,
    from_sym: Option<&str>,
    to_sym: Option<&str>,
    from_module: Option<&str>,
    to_module: Option<&str>,
    kind: &str,
) -> Result<String> {
    let edge_id = Uuid::new_v4().to_string();
    
    let edge = Edge {
        id: edge_id.clone(),
        from_sym: from_sym.map(|s| s.to_string()),
        to_sym: to_sym.map(|s| s.to_string()),
        from_module: from_module.map(|s| s.to_string()),
        to_module: to_module.map(|s| s.to_string()),
        kind: kind.to_string(),
    };

    let value = serde_json::to_vec(&edge)?;
    db.db.insert(format!("edge:{}", edge_id), value.clone())?;
    
    if let Some(from) = from_sym {
        db.db.insert(format!("edges_from:{}:{}:{}", from, kind, edge_id), value)?;
    }
    
    Ok(edge_id)
}

pub fn upsert_module(db: &mut SymgraphDb, name: &str, kind: &str, path: &str) -> Result<String> {
    let key = format!("module:{}", name);
    if let Some(existing) = db.db.get(&key)? {
        let module: Module = serde_json::from_slice(&existing)?;
        Ok(module.id)
    } else {
        let module_id = Uuid::new_v4().to_string();
        let module = Module {
            id: module_id.clone(),
            project_id: "1".to_string(),
            name: name.to_string(),
            kind: kind.to_string(),
            path: if path.is_empty() { None } else { Some(path.to_string()) },
        };
        
        let value = serde_json::to_vec(&module)?;
        db.db.insert(&key, value.clone())?;
        db.db.insert(format!("module:{}", module_id), value)?;
        Ok(module_id)
    }
}

// SCIP-specific methods
impl SymgraphDb {
    /// Store SCIP document information
    pub fn store_scip_document(&mut self, doc_info: &crate::scip::ScipDocumentInfo) -> Result<()> {
        let value = serde_json::to_vec(doc_info)?;
        self.db.insert(format!("scip_document:{}", doc_info.id), value)?;
        self.db.insert(format!("scip_document_by_path:{}", doc_info.relative_path), doc_info.id.as_bytes())?;
        Ok(())
    }

    /// Store SCIP symbol information
    pub fn store_scip_symbol(&mut self, symbol_info: &crate::scip::ScipSymbolInfo) -> Result<()> {
        let value = serde_json::to_vec(symbol_info)?;
        self.db.insert(format!("scip_symbol:{}", symbol_info.id), value)?;
        self.db.insert(format!("scip_symbol_by_name:{}", symbol_info.symbol), symbol_info.id.as_bytes())?;
        Ok(())
    }

    /// Store SCIP occurrence information
    pub fn store_scip_occurrence(&mut self, occ_info: &crate::scip::ScipOccurrenceInfo) -> Result<()> {
        let value = serde_json::to_vec(occ_info)?;
        self.db.insert(format!("scip_occurrence:{}", occ_info.id), value)?;
        Ok(())
    }

    /// Get all SCIP documents for a project
    pub fn get_scip_documents(&self, project_id: &str) -> Result<Vec<crate::scip::ScipDocumentInfo>> {
        let mut documents = Vec::new();
        for item in self.db.scan_prefix("scip_document:") {
            let (_, value) = item?;
            if let Ok(doc) = serde_json::from_slice::<crate::scip::ScipDocumentInfo>(&value) {
                if doc.project_id == project_id {
                    documents.push(doc);
                }
            }
        }
        Ok(documents)
    }

    /// Get SCIP symbols for a file
    pub fn get_scip_symbols_for_file(&self, file_id: &str) -> Result<Vec<crate::scip::ScipSymbolInfo>> {
        let mut symbols = Vec::new();
        for item in self.db.scan_prefix("scip_symbol:") {
            let (_, value) = item?;
            if let Ok(symbol) = serde_json::from_slice::<crate::scip::ScipSymbolInfo>(&value) {
                if symbol.file_id == file_id {
                    symbols.push(symbol);
                }
            }
        }
        Ok(symbols)
    }

    /// Get SCIP occurrences for a symbol
    pub fn get_scip_occurrences_for_symbol(&self, symbol_id: &str) -> Result<Vec<crate::scip::ScipOccurrenceInfo>> {
        let mut occurrences = Vec::new();
        for item in self.db.scan_prefix("scip_occurrence:") {
            let (_, value) = item?;
            if let Ok(occ) = serde_json::from_slice::<crate::scip::ScipOccurrenceInfo>(&value) {
                if occ.symbol_id == symbol_id {
                    occurrences.push(occ);
                }
            }
        }
        Ok(occurrences)
    }

    /// Get database statistics
    pub fn get_stats(&self) -> Result<DatabaseStats> {
        let mut files = 0;
        let mut symbols = 0;
        let mut edges = 0;

        for item in self.db.scan_prefix("file:") {
            let _ = item?;
            files += 1;
        }

        for item in self.db.scan_prefix("symbol:") {
            let _ = item?;
            symbols += 1;
        }

        for item in self.db.scan_prefix("edge:") {
            let _ = item?;
            edges += 1;
        }

        Ok(DatabaseStats { files, symbols, edges })
    }

    /// List all files
    pub fn list_files(&self) -> Result<Vec<FileInfo>> {
        let mut files = Vec::new();
        for item in self.db.scan_prefix("file:") {
            let (_, value) = item?;
            if let Ok(file) = serde_json::from_slice::<File>(&value) {
                files.push(FileInfo {
                    id: file.id,
                    path: file.path,
                    language: file.lang,
                    category: file.category.unwrap_or_default(),
                    purpose: file.purpose.unwrap_or_default(),
                });
            }
        }
        Ok(files)
    }

    /// Search files by path
    pub fn search_files(&self, query: &str) -> Result<Vec<FileInfo>> {
        let all_files = self.list_files()?;
        let query_lower = query.to_lowercase();
        Ok(all_files
            .into_iter()
            .filter(|f| f.path.to_lowercase().contains(&query_lower))
            .collect())
    }

    /// List all symbols
    pub fn list_symbols(&self) -> Result<Vec<SymbolInfo>> {
        let mut symbols = Vec::new();
        for item in self.db.scan_prefix("symbol:") {
            let (_, value) = item?;
            if let Ok(symbol) = serde_json::from_slice::<Symbol>(&value) {
                symbols.push(SymbolInfo {
                    id: symbol.id,
                    name: symbol.name,
                    kind: symbol.kind,
                    file_id: symbol.file_id,
                });
            }
        }
        Ok(symbols)
    }

    /// Search symbols by name
    pub fn search_symbols(&self, query: &str) -> Result<Vec<SymbolInfo>> {
        let all_symbols = self.list_symbols()?;
        let query_lower = query.to_lowercase();
        Ok(all_symbols
            .into_iter()
            .filter(|s| s.name.to_lowercase().contains(&query_lower))
            .collect())
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct DatabaseStats {
    pub files: u64,
    pub symbols: u64,
    pub edges: u64,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct FileInfo {
    pub id: String,
    pub path: String,
    pub language: String,
    pub category: String,
    pub purpose: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct SymbolInfo {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub file_id: String,
}
