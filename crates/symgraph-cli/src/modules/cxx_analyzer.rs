use anyhow::Result;
use std::path::Path;
use clang::{Clang, Index};
use symgraph_core::{Db, insert_edge, insert_occurrence, insert_symbol, upsert_module};
use symgraph_cxx::{categorize_cpp_file, infer_cpp_purpose, scan_tu};
use symgraph_discovery::load_compile_commands;

/// Scan C/C++ source code using compile_commands.json.
pub fn scan_cxx(compdb: &str, db_path: &str) -> Result<()> {
    
    let clang = Clang::new().map_err(|e| anyhow::anyhow!(e))?;
    let index = Index::new(&clang, false, false);

    let mut db = Db::open(db_path)?;
    let compile_commands = load_compile_commands(compdb)?;

    let mut file_count = 0;
    let mut symbol_count = 0;
    let mut relation_count = 0;

    for cc in compile_commands {
        // Skip if file doesn't exist
        if !Path::new(&cc.file).exists() {
            eprintln!("Warning: File not found: {}", cc.file);
            continue;
        }

        // Categorize file
        let category = categorize_cpp_file(&cc.file);
        let purpose = infer_cpp_purpose(&cc.file, &category);
        let category_str = format!("{:?}", category).to_lowercase();
        
        // Create TranslationUnit from compile command
        let tu = match index.parser(&cc.file)
            .arguments(&cc.arguments.as_deref().unwrap_or(&[]))
            .parse()
        {
            Ok(tu) => tu,
            Err(e) => {
                eprintln!("Warning: Failed to parse {}: {}", cc.file, e);
                continue;
            }
        };
        
        // Scan the translation unit for symbols
        let (symbols, occs, edges) = scan_tu(&tu);
        
        file_count += 1;
        
        // Process symbols
        for s in symbols {
            let fid = db.ensure_file_with_category(
                &"1", &s.file, "c++", Some(&category_str), Some(&purpose)
            )?;
            let _sid = insert_symbol(
                &mut db,
                &fid,
                s.usr.as_deref(),
                None,
                &s.name,
                &s.kind,
                s.is_definition,
            )?;
            symbol_count += 1;
        }

        // Process occurrences
        for o in occs {
            let fid = db.ensure_file_with_category(
                &"1", &o.file, "c++", Some(&category_str), Some(&purpose)
            )?;
            
            // Find symbol by USR first
            if let Some(usr) = &o.usr {
                if let Some(sym_id) = db.find_symbol_by_usr(usr)? {
                    let _oid = insert_occurrence(
                        &mut db,
                        &sym_id,
                        &fid,
                        &o.usage_kind,
                        o.line,
                        o.column,
                    )?;
                    relation_count += 1;
                }
            }
        }

        // Process edges
        for (kind, from, to) in &edges {
            if let (Some(from_id), Some(to_id)) = (
                db.find_symbol_by_usr(from)?,
                db.find_symbol_by_usr(to)?
            ) {
                let _eid = insert_edge(
                    &mut db,
                    Some(&from_id),
                    Some(&to_id),
                    None,
                    None,
                    kind,
                )?;
                relation_count += 1;
            }
        }
    }

    println!("\n=== Summary ===");
    println!("Files processed: {}", file_count);
    println!("Symbols extracted: {}", symbol_count);
    println!("Relations found: {}", relation_count);

    Ok(())
}

/// Import C++20 module dependencies.
pub fn import_modules(root: &str, db_path: &str) -> Result<()> {
    use symgraph_cxx::modules::scan_cpp20_module;
    use walkdir::WalkDir;

    let mut db = Db::open(db_path)?;

    for entry in WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let path = e.path();
            path.is_file()
                && (path.extension().map_or(false, |ext| ext == "cppm")
                    || path.extension().map_or(false, |ext| ext == "ixx")
                    || path.extension().map_or(false, |ext| ext == "mxx"))
        })
    {
        let path = entry.path();
        let module_result = scan_cpp20_module(&path.to_string_lossy())?;
        if let Some(module_info) = module_result {
            println!("Scanning module: {}", path.display());
            let module_id = upsert_module(
                &mut db,
                &module_info.name,
                "cpp20-module",
                &path.to_string_lossy(),
            )?;

            // Import module dependencies
            for dep in &module_info.imports {
                    let dep_id = upsert_module(
                        &mut db,
                        dep,
                        "cpp20-module",
                        "", // Path unknown for dependency
                    )?;
                    insert_edge(
                        &mut db,
                        None,
                        None,
                        Some(&module_id),
                        Some(&dep_id),
                        "module-import",
                    )?;
            }
        }
    }

    Ok(())
}

/// Scan C++20 modules directly from source.
pub fn scan_modules(root: &str, db_path: &str) -> Result<()> {
    use symgraph_cxx::modules::analyze_cpp_module;
    use walkdir::WalkDir;

    let mut db = Db::open(db_path)?;

    for entry in WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let path = e.path();
            path.is_file()
                && (path.extension().map_or(false, |ext| ext == "cppm")
                    || path.extension().map_or(false, |ext| ext == "ixx")
                    || path.extension().map_or(false, |ext| ext == "mxx"))
        })
    {
        let path = entry.path();
        let analysis_result = analyze_cpp_module(path.to_str().unwrap())?;
        if let Some(analysis) = analysis_result {
            println!("Analyzing module: {}", path.display());
            let _module_id = upsert_module(
                &mut db,
                &analysis.info.name,
                "cpp20-module",
                &path.to_string_lossy(),
            )?;

            // Add module dependencies - skip for now until we have proper symbol name resolution
            // for rel in &analysis.relations {
            //     if let (Some(from_id), Some(to_id)) = (
            //         db.find_symbol_by_name(&rel.from_name)?,
            //         db.find_symbol_by_name(&rel.to_name)?
            //     ) {
            //         let _eid = insert_edge(
            //             &mut db,
            //             Some(&from_id),
            //             Some(&to_id),
            //             None,
            //             None,
            //             &rel.kind,
            //         )?;
            //     }
            // }
        }
    }

    Ok(())
}

