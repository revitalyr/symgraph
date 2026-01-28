use anyhow::Result;
use std::path::Path;
use std::fs;
use std::process::Command;
use symgraph_core::{Db, insert_edge, insert_occurrence, insert_symbol};
use syn::{Expr, ExprCall, ItemFn, ItemStruct, ItemEnum, ItemMod, ItemTrait, ItemImpl, Type, visit::Visit};
use walkdir::WalkDir;
use serde_json::Value;
use cargo_metadata::MetadataCommand;
use std::collections::HashMap;

/// Analyze Rust projects: collect functions and call edges using `cargo_metadata` + `syn`.
pub fn scan_rust(manifest_path: &str, lsif: Option<&str>, db_path: &str) -> Result<()> {
    // Resolve manifest path and metadata
    let manifest_path = Path::new(manifest_path).canonicalize()?;
    let metadata = MetadataCommand::new()
        .manifest_path(&manifest_path)
        .exec()?;

    // Create database
    let mut db = Db::open(db_path)?;
    let _project_id = db.ensure_project(&metadata.workspace_root.to_string(), &metadata.workspace_root.to_string())?;

    // Process workspace packages
    for package in &metadata.packages {
        if package.name.starts_with("symgraph") {
            continue; // Skip self
        }

        println!("Processing package: {}", package.name);
        process_rust_package(&package.name, package.manifest_path.as_std_path(), &mut db)?;
    }

    // Process workspace-level extra directories (examples, tests, etc.)
    let workspace_root = metadata.workspace_root.as_std_path();
    for extra_dir in ["examples", "tests", "benches"] {
        let extra_path = workspace_root.join(extra_dir);
        if extra_path.exists() {
            process_workspace_extra_dir(&extra_path, "rust", &mut db)?;
        }
    }

    // If LSIF file is provided, parse it and insert into database
    if let Some(lsif_path) = lsif {
        parse_lsif_and_insert(lsif_path, &mut db, &metadata.workspace_root.to_string())?;
    }

    Ok(())
}

/// Process workspace-level extra directories (examples, tests) that don't belong to a specific package.
pub fn process_workspace_extra_dir(dir_path: &Path, language: &str, db: &mut Db) -> Result<()> {
    use symgraph_rust::{categorize_rust_file, infer_rust_purpose};
    
    #[derive(Default)]
    struct V {
        symbols: Vec<String>,
        calls: Vec<(String, String)>,
        current_fn: Vec<String>,
    }

    impl<'ast> Visit<'ast> for V {
        fn visit_item_fn(&mut self, node: &'ast ItemFn) {
            let name = node.sig.ident.to_string();
            self.symbols.push(name.clone());
            self.current_fn.push(name);
            syn::visit::visit_item_fn(self, node);
            self.current_fn.pop();
        }

        fn visit_expr_call(&mut self, node: &'ast ExprCall) {
            if let Expr::Path(p) = &*node.func {
                if let Some(seg) = p.path.segments.last() {
                    let callee = seg.ident.to_string();
                    if let Some(ref caller) = self.current_fn.last() {
                        self.calls.push((caller.to_string(), callee));
                    }
                }
            }
            syn::visit::visit_expr_call(self, node);
        }
    }

    let mut name_to_usr = HashMap::new();

    for entry in WalkDir::new(dir_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path().extension().map_or(false, |ext| ext == "rs")
        })
    {
        let path_str = entry.path().to_string_lossy();
        let category = categorize_rust_file(&path_str);
        let purpose = infer_rust_purpose(&path_str, &category);
        let category_str = format!("{:?}", category).to_lowercase();
        
        let s = fs::read_to_string(entry.path())?;
        match syn::parse_file(&s) {
            Ok(parsed) => {
                let mut v = V::default();
                v.visit_file(&parsed);

                for sym in v.symbols.iter() {
                    let fid = db.ensure_file_with_category(
                        &"1", &path_str, language, Some(&category_str), Some(&purpose)
                    )?;
                    let usr = format!("r:@workspace@{}", sym);
                    if db.find_symbol_by_usr(&usr)?.is_none() {
                        let _sid = insert_symbol(db, &fid, Some(&usr), None, sym, "function", true)?;
                    }
                    name_to_usr.insert(sym.clone(), usr);
                }

                for (caller, callee) in v.calls.iter() {
                    let caller_usr = name_to_usr.get(caller);
                    let callee_usr = name_to_usr.get(callee);
                    if let (Some(cu), Some(du)) = (caller_usr, callee_usr) {
                        if let (Some(cs), Some(ds)) = (db.find_symbol_by_usr(cu)?, db.find_symbol_by_usr(du)?) {
                            let _eid = insert_edge(db, Some(&cs), Some(&ds), None, None, "call")?;
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("parse failed for {}: {}", path_str, e);
            }
        }
    }
    
    Ok(())
}

/// Process a single Rust package: collect functions and call edges.
pub fn process_rust_package(crate_name: &str, manifest_dir: &Path, db: &mut Db) -> Result<()> {
    use symgraph_rust::{categorize_rust_file, infer_rust_purpose};
    
    #[derive(Default)]
    struct V {
        symbols: Vec<String>,
        calls: Vec<(String, String)>,
        current_fn: Vec<String>,
    }

    impl<'ast> Visit<'ast> for V {
        fn visit_item_fn(&mut self, node: &'ast ItemFn) {
            let name = node.sig.ident.to_string();
            self.symbols.push(name.clone());
            self.current_fn.push(name);
            syn::visit::visit_item_fn(self, node);
            self.current_fn.pop();
        }

        fn visit_item_struct(&mut self, node: &'ast ItemStruct) {
            let name = node.ident.to_string();
            self.symbols.push(name.clone());
            syn::visit::visit_item_struct(self, node);
        }

        fn visit_item_enum(&mut self, node: &'ast ItemEnum) {
            let name = node.variants.iter().map(|v| v.ident.to_string()).collect::<Vec<_>>().join("::");
            self.symbols.push(name);
            syn::visit::visit_item_enum(self, node);
        }

        fn visit_item_mod(&mut self, node: &'ast ItemMod) {
            let name = node.ident.to_string();
            self.symbols.push(name.clone());
            syn::visit::visit_item_mod(self, node);
        }

        fn visit_item_trait(&mut self, node: &'ast ItemTrait) {
            let name = node.ident.to_string();
            self.symbols.push(name.clone());
            syn::visit::visit_item_trait(self, node);
        }

        fn visit_item_impl(&mut self, node: &'ast ItemImpl) {
            // For impl blocks, we don't have a name in the same way
            // but we can track the type being implemented
            if let Type::Path(type_path) = &*node.self_ty {
                if let Some(segment) = type_path.path.segments.last() {
                    let name = format!("impl_{}", segment.ident);
                    self.symbols.push(name);
                }
            }
            syn::visit::visit_item_impl(self, node);
        }

        fn visit_expr_call(&mut self, node: &'ast ExprCall) {
            if let Expr::Path(p) = &*node.func {
                if let Some(seg) = p.path.segments.last() {
                    let callee = seg.ident.to_string();
                    if let Some(ref caller) = self.current_fn.last() {
                        self.calls.push((caller.to_string(), callee));
                    }
                }
            }
            syn::visit::visit_expr_call(self, node);
        }
    }

    let mut name_to_usr = std::collections::HashMap::new();

    for entry in WalkDir::new(manifest_dir.join("src"))
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path().extension().map_or(false, |ext| ext == "rs")
        })
    {
        let path_str = entry.path().to_string_lossy();
        let category = categorize_rust_file(&path_str);
        let purpose = infer_rust_purpose(&path_str, &category);
        let category_str = format!("{:?}", category).to_lowercase();
        
        // Add file once and get its ID
        let fid = db.ensure_file_with_category(
            &"1", &path_str, "rust", Some(&category_str), Some(&purpose)
        )?;

        // Add symbols for this file
        let s = fs::read_to_string(entry.path())?;
        match syn::parse_file(&s) {
            Ok(parsed) => {
                let mut v = V::default();
                v.visit_file(&parsed);

                // Add symbols for this file
                for sym in v.symbols.iter() {
                    let usr = format!("r:@{}@{}", crate_name, sym);
                    if db.find_symbol_by_usr(&usr)?.is_none() {
                        // Determine symbol kind based on name patterns
                        let kind = if sym.starts_with("impl_") {
                            "impl"
                        } else {
                            "function"  // Default for now, could be enhanced
                        };
                        let _sid = insert_symbol(db, &fid, Some(&usr), None, sym, kind, true)?;
                    }
                    name_to_usr.insert(sym.clone(), usr);
                }

                // Add call edges
                for (caller, callee) in v.calls.iter() {
                    let caller_usr = name_to_usr.get(caller);
                    let callee_usr = name_to_usr.get(callee);
                    if let (Some(cu), Some(du)) = (caller_usr, callee_usr) {
                        if let (Some(cs), Some(ds)) = (db.find_symbol_by_usr(cu)?, db.find_symbol_by_usr(du)?) {
                            let _eid = insert_edge(db, Some(&cs), Some(&ds), None, None, "call")?;
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("parse failed for {}: {}", path_str, e);
            }
        }
    }
    
    Ok(())
}

/// Generate LSIF file using rust-analyzer.
pub fn generate_lsif_file(project_dir: &Path, output_path: &Path) -> Result<()> {
    // Allow override via environment variable (for tests/custom paths)
    let ra_bin = std::env::var("SYGRAPH_RUST_ANALYZER_CMD").unwrap_or_else(|_| "rust-analyzer".to_string());

    let output = Command::new(ra_bin)
        .args([
            "lsif",
            project_dir.to_str().unwrap(),
            "--output",
            output_path.to_str().unwrap(),
        ])
        .output()?;

    if !output.status.success() {
        anyhow::bail!(
            "rust-analyzer lsif failed: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    println!("LSIF file generated: {}", output_path.display());
    Ok(())
}

/// Parse minimal LSIF (rust-analyzer) and insert definitions/references into DB.
pub fn parse_lsif_and_insert(lsif_path: &str, db: &mut Db, _crate_name: &str) -> Result<()> {
    let content = fs::read_to_string(lsif_path)?;
    // Try parse as JSON array, otherwise as line-delimited JSON
    let items: Vec<Value> = if let Ok(v) = serde_json::from_str::<Value>(&content) {
        match v {
            Value::Array(a) => a,
            _ => vec![v],
        }
    } else {
        let mut vec = Vec::new();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("//") {
                continue;
            }
            if let Ok(v) = serde_json::from_str::<Value>(line) {
                vec.push(v);
            }
        }
        vec
    };

    // Build maps: vertex_id -> vertex, range_id -> range
    let mut vertices = std::collections::HashMap::new();
    let mut ranges = std::collections::HashMap::new();

    for item in items {
        if let Some(obj) = item.as_object() {
            if let (Some(id), Some(vertex_type)) = (obj.get("id"), obj.get("type")) {
                let id = id.as_u64().unwrap();
                let vertex_type = vertex_type.as_str().unwrap();
                if vertex_type == "vertex" {
                    vertices.insert(id, obj.clone());
                } else if vertex_type == "range" {
                    ranges.insert(id, obj.clone());
                }
            }
        }
    }

    // Build range map: range_id -> (start,line,char,end)
    #[derive(Debug, Clone)]
    struct RangeInfo {
        start_line: usize,
        start_char: usize,
        end_line: usize,
        end_char: usize,
    }

    let mut range_map = std::collections::HashMap::new();
    for (range_id, range_obj) in ranges {
        if let (Some(start), Some(end)) = (range_obj.get("start"), range_obj.get("end")) {
            let start_line = start.get("line").unwrap().as_u64().unwrap();
            let start_char = start.get("character").unwrap().as_u64().unwrap();
            let end_line = end.get("line").unwrap().as_u64().unwrap();
            let end_char = end.get("character").unwrap().as_u64().unwrap();

            range_map.insert(
                range_id.clone(),
                RangeInfo {
                    start_line: start_line as usize,
                    start_char: start_char as usize,
                    end_line: end_line as usize,
                    end_char: end_char as usize,
                },
            );
        }
    }

    // Process vertices to extract symbols and references
    for (_vertex_id, vertex) in &vertices {
        if let (Some(vertex_type), Some(label)) = (vertex.get("type"), vertex.get("label")) {
            let vertex_type = vertex_type.as_str().unwrap();
            let label = label.as_str().unwrap();
            let _label = label; // Prefix with underscore to suppress warning

            match vertex_type {
                "definition" => {
                    // Extract symbol definition
                    if let Some(usr) = vertex.get("usr") {
                        let usr = usr.as_str().unwrap();
                        let name = extract_name_from_usr(usr);
                        let kind = infer_kind_from_usr(usr);

                        // Find containing file with range information
                        let mut file_path = "unknown".to_string();
                        let mut symbol_line = None;
                        let mut symbol_column = None;
                        
                        if let Some(containing) = vertex.get("containment") {
                            if let Some(range_id) = containing.as_u64() {
                                if let Some(range_info) = range_map.get(&range_id) {
                                    // Store symbol location for potential debugging
                                    symbol_line = Some(range_info.start_line + 1);
                                    symbol_column = Some(range_info.start_char + 1);
                                    
                                    // Find document for this range
                                    for (_doc_id, doc_vertex) in vertices.iter() {
                                        if let Some(doc_type) = doc_vertex.get("type") {
                                            if doc_type.as_str().unwrap() == "document" {
                                                if let Some(uri) = doc_vertex.get("uri") {
                                                    file_path = uri.as_str().unwrap().to_string();
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        let fid = db.ensure_file(&file_path, "rust")?;
                        let sid = insert_symbol(db, &fid, Some(usr), None, &name, kind, true)?;

                        // Log symbol location for debugging
                        if let (Some(line), Some(col)) = (symbol_line, symbol_column) {
                            println!("Found symbol {} at {}:{}:{}", name, file_path, line, col);
                        }

                        // Add occurrence for definition
                        if let Some(moniker) = vertex.get("moniker") {
                            if let Some(range_id) = moniker.get("range").and_then(|r| r.as_u64()) {
                                if let Some(range_info) = range_map.get(&range_id) {
                                    // Add occurrence for definition with full range information
                                    let _oid = insert_occurrence(
                                        db,
                                        &sid,
                                        &fid,
                                        "definition",
                                        (range_info.start_line + 1) as u32,
                                        (range_info.start_char + 1) as u32,
                                    )?;
                                    
                                    // Store range information for potential future use
                                    // Could be used for precise symbol highlighting, refactoring, etc.
                                    let _range_span = format!(
                                        "{}:{}-{}:{}",
                                        range_info.start_line + 1,
                                        range_info.start_char + 1,
                                        range_info.end_line + 1,
                                        range_info.end_char + 1
                                    );
                                }
                            }
                        }
                    }
                }
                "reference" => {
                    // Extract symbol reference
                    if let Some(usr) = vertex.get("usr") {
                        let usr = usr.as_str().unwrap();
                        if let Some(symbol_id) = db.find_symbol_by_usr(usr)? {
                            // Find containing file and range information
                            let mut file_path = "unknown".to_string();
                            let mut line = 1u32;
                            let mut column = 1u32;
                            
                            if let Some(moniker) = vertex.get("moniker") {
                                if let Some(range_id) = moniker.get("range").and_then(|r| r.as_u64()) {
                                    if let Some(range_info) = range_map.get(&range_id) {
                                        // Use actual range information
                                        line = (range_info.start_line + 1) as u32;
                                        column = (range_info.start_char + 1) as u32;
                                        
                                        // Find document for this range
                                        for (_doc_id, doc_vertex) in vertices.iter() {
                                            if let Some(doc_type) = doc_vertex.get("type") {
                                                if doc_type.as_str().unwrap() == "document" {
                                                    if let Some(uri) = doc_vertex.get("uri") {
                                                        file_path = uri.as_str().unwrap().to_string();
                                                        break;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            let fid = db.ensure_file(&file_path, "rust")?;
                            let _oid = insert_occurrence(
                                db,
                                &symbol_id,
                                &fid,
                                "reference",
                                line,
                                column,
                            )?;
                        }
                    }
                }
                _ => {}
            }
        }
    }

    Ok(())
}

fn extract_name_from_usr(usr: &str) -> String {
    // Extract the last component from USR
    if let Some(last) = usr.rsplit("::").next() {
        if let Some(name) = last.rsplit("#").next() {
            name.to_string()
        } else {
            last.to_string()
        }
    } else {
        usr.to_string()
    }
}

fn infer_kind_from_usr(usr: &str) -> &str {
    if usr.contains("F@") {
        "function"
    } else if usr.contains("S@") {
        "struct"
    } else if usr.contains("E@") {
        "enum"
    } else if usr.contains("T@") {
        "trait"
    } else if usr.contains("M@") {
        "module"
    } else if usr.contains("C@") {
        "const"
    } else {
        "variable"
    }
}
