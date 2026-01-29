use anyhow::Result;
use std::path::Path;
use cargo_metadata::MetadataCommand;
use symgraph_core::Db;

/// Analyze Rust projects: collect functions and call edges using SCIP indexing.
pub fn scan_rust(manifest_path: &str, lsif: Option<&str>, db_path: &str) -> Result<()> {
    use symgraph_discovery::{ScipConfig, ScipLanguage, check_scip_tool_availability, generate_scip_index};
    use symgraph_core::scip::load_scip_to_database;
    
    // Resolve manifest path and metadata
    let manifest_path = Path::new(manifest_path).canonicalize()?;
    let metadata = MetadataCommand::new()
        .manifest_path(&manifest_path)
        .exec()?;

    // Create database
    let mut db = Db::open(db_path)?;
    let _project_id = db.ensure_project(&metadata.workspace_root.to_string(), &metadata.workspace_root.to_string())?;

    // Generate SCIP index for the project
    let project_dir = manifest_path.parent().unwrap_or(&manifest_path);
    let scip_output = project_dir.join("dump.scip");
    
    println!("Generating SCIP index for Rust project...");
    match check_scip_tool_availability(&ScipLanguage::Rust) {
        Ok(true) => {
            let scip_config = ScipConfig::new(ScipLanguage::Rust, project_dir, &scip_output);
            match generate_scip_index(&scip_config) {
                Ok(_) => {
                    println!("SCIP index generated successfully, loading into database...");
                    
                    // Load SCIP data from file
                    let scip_data = symgraph_core::scip::parse_scip_file(&scip_output)?;
                    
                    // Load SCIP data into database
                    match load_scip_to_database(&mut db, &scip_data, &metadata.workspace_root.to_string()) {
                        Ok(_) => {
                            println!("SCIP data loaded into database successfully");
                            
                            // Clean up temporary SCIP file
                            let _ = std::fs::remove_file(&scip_output);
                        }
                        Err(e) => {
                            eprintln!("Failed to load SCIP data into database: {}", e);
                            return Err(e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Failed to generate SCIP index: {}", e);
                    return Err(e);
                }
            }
        }
        Ok(false) => {
            let instruction = symgraph_discovery::get_installation_instruction(&ScipLanguage::Rust);
            eprintln!("rust-analyzer not found for SCIP generation. Install with: {}", instruction);
            return Err(anyhow::anyhow!("rust-analyzer not available"));
        }
        Err(e) => {
            eprintln!("Error checking rust-analyzer availability: {}", e);
            return Err(e);
        }
    }

    // If LSIF file is provided, parse it and insert into database
    if let Some(lsif_path) = lsif {
        parse_lsif_and_insert(lsif_path, &mut db, &metadata.workspace_root.to_string())?;
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
