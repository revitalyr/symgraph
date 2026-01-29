use anyhow::{Result, Context};
use std::path::Path;
use cargo_metadata::MetadataCommand;
use symgraph_core::Db;
use walkdir::WalkDir;

/// Analyze Rust projects: collect functions and call edges using SCIP indexing.
pub fn scan_rust(manifest_path: &str, lsif: Option<&str>, db_path: &str) -> Result<()> {
    use symgraph_discovery::{ScipLanguage, check_scip_tool_availability};
    use symgraph_core::scip::{load_scip_to_database, parse_scip_file};
    use std::path::PathBuf;
    
    // Resolve manifest path and metadata
    let manifest_path = Path::new(manifest_path).canonicalize()?;
    let metadata = MetadataCommand::new()
        .manifest_path(&manifest_path)
        .exec()?;
    
    let project_dir = manifest_path.parent().unwrap();
    let mut db = Db::open(db_path)?;
    
    // If LSIF file is provided, parse it and insert into database
    if let Some(lsif_path) = lsif {
        parse_lsif_and_insert(lsif_path, &mut db, &metadata.workspace_root.to_string())?;
        return Ok(());
    }
    
    // Generate SCIP for all Rust files in the project
    println!("Generating SCIP index for Rust project...");
    match check_scip_tool_availability(&ScipLanguage::Rust) {
        Ok(true) => {
            // Find all Rust files
            let rust_files: Vec<PathBuf> = WalkDir::new(project_dir.join("src"))
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.path().extension().map_or(false, |ext| ext == "rs")
                })
                .map(|e| e.path().to_path_buf())
                .collect();
            
            println!("Found {} Rust files to index", rust_files.len());
            
            let mut total_symbols = 0;
            let mut total_documents = 0;
            
            for rust_file in &rust_files {
                let scip_output = project_dir.join(format!("dump_{}.scip", 
                    rust_file.file_stem().unwrap().to_str().unwrap()));
                
                // Generate SCIP for individual file
                let mut cmd = std::process::Command::new("rust-analyzer");
                cmd.arg("scip")
                    .arg(rust_file.strip_prefix(project_dir).unwrap())
                    .arg("--output")
                    .arg(&scip_output)
                    .current_dir(project_dir);
                
                let output = cmd.output()
                    .with_context(|| "Failed to execute rust-analyzer. Install with: rustup component add rust-analyzer")?;
                
                if !output.status.success() {
                    eprintln!("Failed to generate SCIP for {}: {}", 
                        rust_file.display(), 
                        String::from_utf8_lossy(&output.stderr));
                    continue;
                }
                
                if !scip_output.exists() {
                    eprintln!("SCIP file was not generated for: {}", scip_output.display());
                    continue;
                }
                
                // Load SCIP data from file
                match parse_scip_file(&scip_output) {
                    Ok(scip_data) => {
                        total_documents += scip_data.documents.len();
                        total_symbols += scip_data.symbols.len();
                        
                        // Load SCIP data into database
                        match load_scip_to_database(&mut db, &scip_data, &metadata.workspace_root.to_string()) {
                            Ok(_) => {
                                // Success
                            }
                            Err(e) => {
                                eprintln!("Failed to load SCIP data into database: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to parse SCIP file: {}", e);
                    }
                }
                
                // Clean up temporary SCIP file
                let _ = std::fs::remove_file(&scip_output);
            }
            
            println!("SCIP indexing completed: {} documents, {} symbols total", 
                total_documents, total_symbols);
        }
        Ok(false) => {
            eprintln!("SCIP tool not available for Rust");
            return Err(anyhow::anyhow!("SCIP tool not available"));
        }
        Err(e) => {
            eprintln!("SCIP tool not available: {}", e);
            return Err(e);
        }
    }

    Ok(())
}

/// Parse LSIF file and insert into database (legacy support)
fn parse_lsif_and_insert(lsif_path: &str, _db: &mut Db, _project_name: &str) -> Result<()> {
    // This function can be implemented for legacy LSIF support
    // For now, we'll just log that LSIF parsing is not implemented
    println!("LSIF parsing not yet implemented: {}", lsif_path);
    Ok(())
}

/// Generate LSIF file using rust-analyzer (legacy support).
pub fn generate_lsif_file(project_dir: &Path, output_path: &Path) -> Result<()> {
    use std::process::Command;
    
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
