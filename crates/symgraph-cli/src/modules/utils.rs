use anyhow::Result;
use std::path::Path;
use std::process::Command;
use symgraph_discovery::generate_scip_index;
use serde_json;
use tempfile;
use symgraph_core::scip::parse_scip_file;

/// Generate compile_commands.json from a build system.
///
/// # Arguments
/// * `project` - Project root directory
/// * `output` - Optional output file path
/// * `build_dir` - Build directory for CMake
/// * `build_system` - Explicit build system type or Auto
/// * `generator` - CMake generator
/// * `configuration` - VS configuration (Debug/Release)
/// * `platform` - VS platform (x64/Win32)
pub fn generate_compdb(
    project: &str,
    output: Option<&str>,
    build_dir: Option<&str>,
    build_system: Option<crate::modules::commands::cli::BuildSystemType>,
    generator: Option<&str>,
    configuration: Option<&str>,
    platform: Option<&str>,
) -> Result<()> {
    let project_path = Path::new(project);
    let output_path = output.unwrap_or("compile_commands.json");

    // Detect build system if not specified
    let build_system = build_system.unwrap_or(crate::modules::commands::cli::BuildSystemType::Auto);

    match build_system {
        crate::modules::commands::cli::BuildSystemType::Auto => {
            // Try to detect build system automatically
            if project_path.join("CMakeLists.txt").exists() {
                return generate_cmake_compdb(project_path, output_path, build_dir, generator);
            } else if project_path.join("Makefile").exists() {
                return generate_make_compdb(project_path, output_path);
            } else if project_path.join("Cargo.toml").exists() {
                return generate_cargo_compdb(project_path, output_path);
            } else if find_file_with_ext(project_path, "sln").is_ok() {
                return generate_vs_compdb(project_path, output_path, configuration, platform);
            } else {
                anyhow::bail!("Could not detect build system in {}", project);
            }
        }
        crate::modules::commands::cli::BuildSystemType::CMake => {
            return generate_cmake_compdb(project_path, output_path, build_dir, generator);
        }
        crate::modules::commands::cli::BuildSystemType::Make => {
            return generate_make_compdb(project_path, output_path);
        }
        crate::modules::commands::cli::BuildSystemType::Solution => {
            return generate_vs_compdb(project_path, output_path, configuration, platform);
        }
        crate::modules::commands::cli::BuildSystemType::Cargo => {
            return generate_cargo_compdb(project_path, output_path);
        }
    }
}

/// Generate compile_commands.json from CMake project
fn generate_cmake_compdb(project_path: &Path, output: &str, build_dir: Option<&str>, generator: Option<&str>) -> Result<()> {
    let build_dir = build_dir.unwrap_or("build");
    let build_dir_path = project_path.join(build_dir);

    // Create build directory if it doesn't exist
    if !build_dir_path.exists() {
        std::fs::create_dir_all(&build_dir_path)
            .map_err(|e| anyhow::anyhow!("Failed to create build directory '{}': {}", build_dir_path.display(), e))?;
    }

    let mut cmake_cmd = Command::new("cmake");
    cmake_cmd.current_dir(&build_dir_path);

    // Configure with generator if specified
    if let Some(gen) = generator {
        cmake_cmd.args(&["-G", gen]);
    }

    cmake_cmd.args(&[
        "..",
        "-DCMAKE_EXPORT_COMPILE_COMMANDS=ON",
        "-DCMAKE_BUILD_TYPE=Debug",
    ]);

    let cmake_output = cmake_cmd.output()?;
    if !cmake_output.status.success() {
        anyhow::bail!("CMake configuration failed: {}", String::from_utf8_lossy(&cmake_output.stderr));
    }

    // Copy compile_commands.json to project root if needed
    let compdb_path = build_dir_path.join("compile_commands.json");
    let output_path = Path::new(output);
    if compdb_path.exists() {
        if compdb_path != output_path {
            std::fs::copy(&compdb_path, output_path)
                .map_err(|e| anyhow::anyhow!("Failed to copy compile_commands.json from '{}' to '{}': {}", 
                    compdb_path.display(), output_path.display(), e))?;
        }
    } else {
        anyhow::bail!("compile_commands.json not found in build directory");
    }

    Ok(())
}

/// Generate compile_commands.json from Makefile project
fn generate_make_compdb(project_path: &Path, output: &str) -> Result<()> {
    // Use bear to generate compile_commands.json from Make
    let bear_output = Command::new("bear")
        .arg("--")
        .arg("make")
        .current_dir(project_path)
        .output()?;

    if !bear_output.status.success() {
        anyhow::bail!("bear make failed: {}", String::from_utf8_lossy(&bear_output.stderr));
    }

    // Move compile_commands.json to desired location
    let compdb_path = project_path.join("compile_commands.json");
    let output_path = Path::new(output);
    if compdb_path.exists() {
        if compdb_path != output_path {
            std::fs::copy(&compdb_path, output_path)
                .map_err(|e| anyhow::anyhow!("Failed to copy compile_commands.json from '{}' to '{}': {}", 
                    compdb_path.display(), output_path.display(), e))?;
        }
    } else {
        anyhow::bail!("compile_commands.json not generated");
    }

    Ok(())
}

/// Generate compile_commands.json from Visual Studio solution
fn generate_vs_compdb(project_path: &Path, output: &str, configuration: Option<&str>, platform: Option<&str>) -> Result<()> {
    let sln_path = find_file_with_ext(project_path, "sln")?;
    
    let mut vs_cmd = Command::new("compdb");
    vs_cmd.arg("-p").arg(&sln_path);

    if let Some(config) = configuration {
        vs_cmd.arg("-c").arg(config);
    }

    if let Some(plat) = platform {
        vs_cmd.arg("-p").arg(plat);
    }

    let vs_output = vs_cmd.output()?;
    if !vs_output.status.success() {
        anyhow::bail!("compdb failed: {}", String::from_utf8_lossy(&vs_output.stderr));
    }

    // Move compile_commands.json to desired location
    let compdb_path = project_path.join("compile_commands.json");
    let output_path = Path::new(output);
    if compdb_path.exists() {
        if compdb_path != output_path {
            std::fs::copy(&compdb_path, output_path)
                .map_err(|e| anyhow::anyhow!("Failed to copy compile_commands.json from '{}' to '{}': {}", 
                    compdb_path.display(), output_path.display(), e))?;
        }
    } else {
        anyhow::bail!("compile_commands.json not generated");
    }

    Ok(())
}

/// Generate compile_commands.json from Cargo project
fn generate_cargo_compdb(project_path: &Path, output: &str) -> Result<()> {
    let cargo_output = Command::new("cargo")
        .args(&["check", "--message-format=json"])
        .current_dir(project_path)
        .output()?;

    if !cargo_output.status.success() {
        anyhow::bail!("cargo check failed: {}", String::from_utf8_lossy(&cargo_output.stderr));
    }

    // Parse cargo output and convert to compile_commands.json
    // This is a simplified version - in practice you'd want to use cargo-llvm-cov or similar
    println!("Warning: Cargo compile_commands.json generation is experimental");
    
    // Create a basic compile_commands.json for now
    let compdb = serde_json::json!([
        {
            "directory": project_path.to_string_lossy(),
            "file": "src/main.rs",
            "arguments": ["rustc", "--edition=2021", "src/main.rs"],
            "output": "target/debug/main"
        }
    ]);

    let output_path = Path::new(output);
    std::fs::write(&output_path, serde_json::to_string_pretty(&compdb)?)
        .map_err(|e| anyhow::anyhow!("Failed to write compile_commands.json to '{}': {}", output_path.display(), e))?;
    Ok(())
}

/// Находит файл с указанным расширением в директории
fn find_file_with_ext(dir: &Path, ext: &str) -> Result<std::path::PathBuf> {
    std::fs::read_dir(dir)
        .map_err(|e| anyhow::anyhow!("Failed to read directory '{}': {}", dir.display(), e))?;
    
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            if let Some(extension) = path.extension() {
                if extension == ext {
                    return Ok(path);
                }
            }
        }
    }
    anyhow::bail!("No .{} file found in {}", ext, dir.display());
}

/// Query call graph for a symbol.
pub fn query_calls(db_path: &str, usr: &str) -> Result<()> {
    let db = symgraph_core::Db::open(db_path)?;

    // Query edges where kind="call" and from_sym matches the USR
    let rows = db.query_edges_by_kind_from("call", usr)?;

    // Print each callee name
    for r in rows {
        println!("{}", r);
    }
    Ok(())
}

/// List all modules in the database.
pub fn list_modules(db_path: &str) -> Result<()> {
    let db = symgraph_core::Db::open(db_path)?;

    // List all modules
    println!("=== Modules ===");
    let mut module_count = 0;
    for item in db.db.scan_prefix("module:") {
        let (_, value): (_, sled::IVec) = item?;
        if let Ok(module) = serde_json::from_slice::<symgraph_core::Module>(&value) {
            println!("{}: {} ({}) - {}", module.id, module.name, module.kind, module.path.unwrap_or_default());
            module_count += 1;
        }
    }

    if module_count == 0 {
        println!("No modules found.");
    }

    // Query module imports
    println!("\n=== Module Dependencies ===");
    let mut import_count = 0;
    for item in db.db.scan_prefix("edge:") {
        let (_, value): (_, sled::IVec) = item?;
        if let Ok(edge) = serde_json::from_slice::<symgraph_core::Edge>(&value) {
            if edge.kind == "module-import" {
                if let (Some(from_module), Some(to_module)) = (&edge.from_module, &edge.to_module) {
                    // Get module names
                    if let (Some(from_data), Some(to_data)) = (
                        db.db.get(format!("module:{}", from_module))?,
                        db.db.get(format!("module:{}", to_module))?
                    ) {
                        if let (Ok(from_mod), Ok(to_mod)) = (
                            serde_json::from_slice::<symgraph_core::Module>(&from_data),
                            serde_json::from_slice::<symgraph_core::Module>(&to_data)
                        ) {
                            println!("  {} -> {}", from_mod.name, to_mod.name);
                            import_count += 1;
                        }
                    }
                }
            }
        }
    }

    if import_count == 0 {
        println!("No module imports found.");
    }

    Ok(())
}

/// Show database statistics.
pub fn show_stats(db_path: &str) -> Result<()> {
    let db = symgraph_core::Db::open(db_path)?;

    let file_count = db.db.scan_prefix("file:").count();
    let symbol_count = db.db.scan_prefix("symbol:").count();
    let occurrence_count = db.db.scan_prefix("occurrence:").count();
    let edge_count = db.db.scan_prefix("edge:").count();
    let module_count = db.db.scan_prefix("module:").count();

    println!("=== Database Statistics ===");
    println!("Files:       {}", file_count);
    println!("Symbols:     {}", symbol_count);
    println!("Occurrences: {}", occurrence_count);
    println!("Edges:       {}", edge_count);
    println!("Modules:     {}", module_count);

    // Symbol breakdown
    println!("\n=== Symbol Types ===");
    let mut symbol_types: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for item in db.db.scan_prefix("symbol:") {
        let (_, value): (_, sled::IVec) = item?;
        if let Ok(symbol) = serde_json::from_slice::<symgraph_core::Symbol>(&value) {
            *symbol_types.entry(symbol.kind).or_insert(0) += 1;
        }
    }
    
    let mut sorted_types: Vec<_> = symbol_types.iter().collect();
    sorted_types.sort_by(|a, b| b.1.cmp(a.1));
    
    for (kind, count) in sorted_types.iter().take(10) {
        println!("  {}: {}", kind, count);
    }

    Ok(())
}

/// Generate project annotation for compiled languages (C++/Rust).
pub fn annotate_compiled_project(root: &str, db_path: &str) -> Result<()> {
    use symgraph_core::annotations::{analyze_cpp_project, analyze_rust_project};
    
    let mut db = symgraph_core::Db::open(db_path)?;
    
    // Get files from database with categories
    let files: Vec<(String, String, String)> = {
        let mut files = Vec::new();
        for item in db.db.scan_prefix("file:") {
            let (_, value): (_, sled::IVec) = item?;
            if let Ok(file) = serde_json::from_slice::<symgraph_core::File>(&value) {
                files.push((
                    file.path,
                    file.category.unwrap_or("unknown".to_string()),
                    file.purpose.unwrap_or("".to_string())
                ));
            }
        }
        files
    };
    
    if files.is_empty() {
        println!("No files found in database. Run scan-cxx or scan-rust first.");
        return Ok(());
    }
    
    // Detect language from file extensions
    let is_cpp = files.iter().any(|(path, _, _): &(String, String, String)| {
        path.ends_with(".cpp") || path.ends_with(".cc") || path.ends_with(".cxx") || path.ends_with(".h")
    });
    let is_rust = files.iter().any(|(path, _, _): &(String, String, String)| path.ends_with(".rs"));
    
    let annotation = if is_cpp {
        analyze_cpp_project(root, &files)?
    } else if is_rust {
        analyze_rust_project(root, &files)?
    } else {
        println!("Unknown project type. Supported: C++, Rust");
        return Ok(());
    };
    
    // Update database
    let project_id = db.ensure_project(&annotation.name, &annotation.root_path)?;
    let purpose_str = format!("{:?}", annotation.purpose);
    let build_system_str = format!("{:?}", annotation.build_system);
    let deps_json = serde_json::to_string(&annotation.dependencies)?;
    
    db.update_project_annotation(
        &project_id,
        &annotation.description,
        &purpose_str,
        &build_system_str,
        &deps_json
    )?;
    
    println!("=== Project Annotation ===");
    println!("Name: {}", annotation.name);
    println!("Language: {}", annotation.language);
    println!("Purpose: {:?}", annotation.purpose);
    println!("Build System: {:?}", annotation.build_system);
    println!("Description: {}", annotation.description);
    println!("Entry Points: {:?}", annotation.entry_points);
    println!("Dependencies: {} external", annotation.dependencies.len());
    println!("Test Coverage: {:.1}%", annotation.test_coverage);
    
    Ok(())
}

/// Analyze script projects (Python, JavaScript, TypeScript) using SCIP.
pub fn scan_scripts(root: &str, db_path: &str) -> Result<()> {
    use anyhow::bail;
    use symgraph_discovery::{ScipConfig, ScipLanguage, detect_language, check_scip_tool_availability};
    use std::path::PathBuf;
    
    let project_path = PathBuf::from(root);
    let detected_language = detect_language(&project_path);
    
    match detected_language {
        ScipLanguage::Python | ScipLanguage::JavaScript | ScipLanguage::TypeScript => {
            println!("Analyzing {} project using SCIP...", detected_language);
            
            // Check if SCIP tool is available
            match check_scip_tool_availability(&detected_language) {
                Ok(_) => {
                    // Generate SCIP index
                    let config = ScipConfig {
                        language: detected_language.clone(),
                        project_path: project_path.clone(),
                        output_path: project_path.join(".scip"),
                        extra_args: vec![],
                        compile_commands: None,
                    };
                    
                    let scip_file_path = generate_scip_index(&config)?;
                    let scip_data = parse_scip_file(&scip_file_path)?;
                    println!("SCIP index generated:");
                    println!("  Documents: {}", scip_data.documents.len());
                    println!("  Symbols: {}", scip_data.symbols.len());
                    println!("  Occurrences: {}", scip_data.occurrences.len());

                    // Load into database
                    let mut db = symgraph_core::Db::open(db_path)?;
                    symgraph_core::scip::load_scip_to_database(&mut db, &scip_data, &format!("{}_project", detected_language))?;
                    
                    println!("SCIP data loaded into database successfully.");
                },
                Err(e) => {
                    println!("Error checking SCIP tool availability: {}", e);
                    bail!("Cannot analyze {} project", detected_language);
                }
            }
        }
        ScipLanguage::Unknown => {
            bail!("Cannot detect language for project annotation. Supported: Python, JavaScript, TypeScript");
        }
        _ => {
            bail!("This is not a script project. Use scan-scip for {} projects", detected_language);
        }
    }
    
    Ok(())
}

/// Generate SCIP index from project.
pub fn scan_scip(root: &str, db_path: &str) -> Result<()> {
    use symgraph_discovery::{ScipConfig, generate_scip_index};
    use std::path::PathBuf;
    
    let project_path = PathBuf::from(root);
    
    // Generate SCIP index
    let config = ScipConfig {
        language: symgraph_discovery::ScipLanguage::Rust, // Default to Rust
        project_path: project_path.clone(),
        output_path: project_path.join(".scip"),
        extra_args: vec![],
        compile_commands: None,
    };
    
    let scip_file_path = generate_scip_index(&config)?;
    let scip_data = parse_scip_file(&scip_file_path)?;
    println!("SCIP index generated:");
    println!("  Documents: {}", scip_data.documents.len());
    println!("  Symbols: {}", scip_data.symbols.len());
    println!("  Occurrences: {}", scip_data.occurrences.len());

    // Load into database
    let mut db = symgraph_core::Db::open(db_path)?;
    symgraph_core::scip::load_scip_to_database(&mut db, &scip_data, "scip_project")?;
    
    println!("SCIP data loaded into database successfully.");
    
    Ok(())
}

/// Start web viewer for database.
pub fn start_web_viewer(db_path: &str) -> Result<()> {
    use std::process::Command;
    use tempfile;
    
    // Create Flask app content (JSON API version)
    let app_content = format!(r#"
from flask import Flask, request, jsonify, send_from_directory
import subprocess
import json
import os
import sys

app = Flask(__name__)

def call_rust_api(endpoint):
    """Call Rust symgraph CLI to get data"""
    try:
        result = subprocess.run([
            'symgraph-cli', 'api', endpoint, '--db', r'{db_path}'
        ], capture_output=True, text=True, timeout=30)
        
        if result.returncode == 0:
            return json.loads(result.stdout)
        else:
            {{"error": result.stderr, "code": result.returncode}}
    except subprocess.TimeoutExpired:
        return {{"error": "Request timeout", "code": 408}}
    except Exception as e:
        return {{"error": str(e), "code": 500}}

@app.route('/')
def index():
    return '''
    <!DOCTYPE html>
    <html>
    <head>
        <title>Symgraph Viewer</title>
        <meta charset="utf-8">
        <style>
            body {{ font-family: Arial, sans-serif; margin: 20px; }}
            .container {{ max-width: 1200px; margin: 0 auto; }}
            .stats {{ display: flex; gap: 20px; margin: 20px 0; }}
            .stat-card {{ border: 1px solid #ddd; padding: 15px; border-radius: 5px; }}
            .error {{ color: red; }}
        </style>
    </head>
    <body>
        <div class="container">
            <h1>Symgraph Viewer</h1>
            <div id="content">Loading...</div>
            <div id="error" class="error"></div>
        </div>
        <script>
            fetch('/api/stats')
                .then(response => response.json())
                .then(data => {{
                    if (data.error) {{
                        document.getElementById('error').textContent = data.error;
                    }} else {{
                        document.getElementById('content').innerHTML = `
                            <div class="stats">
                                <div class="stat-card">
                                    <h3>Files</h3>
                                    <p>${{data.files || 0}}</p>
                                </div>
                                <div class="stat-card">
                                    <h3>Symbols</h3>
                                    <p>${{data.symbols || 0}}</p>
                                </div>
                                <div class="stat-card">
                                    <h3>Edges</h3>
                                    <p>${{data.edges || 0}}</p>
                                </div>
                            </div>
                            <h2>Database Status</h2>
                            <p>Connected to: {db_path}</p>
                        `;
                    }}
                }})
                .catch(error => {{
                    document.getElementById('error').textContent = 'Error: ' + error.message;
                }});
        </script>
    </body>
    </html>
    '''

@app.route('/api/stats')
def get_stats():
    return jsonify(call_rust_api('stats'))

@app.route('/api/files')
def get_files():
    search = request.args.get('search', '')
    endpoint = f'files?search={{search}}' if search else 'files'
    return jsonify(call_rust_api(endpoint))

@app.route('/api/symbols')
def get_symbols():
    search = request.args.get('search', '')
    endpoint = f'symbols?search={{search}}' if search else 'symbols'
    return jsonify(call_rust_api(endpoint))

@app.route('/api/graph')
def get_graph():
    return jsonify(call_rust_api('graph'))

@app.route('/<path:filename>')
def static_files(filename):
    return send_from_directory('.', filename)

@app.route('/')
def index():
    try:
        return send_from_directory('.', 'index.html')
    except:
        # Fallback to basic HTML if index.html not found
        return '''
<!DOCTYPE html>
<html>
<head>
    <title>Symgraph Viewer</title>
    <meta charset="utf-8">
    <style>
        body {{ font-family: Arial, sans-serif; margin: 20px; }}
        .container {{ max-width: 1200px; margin: 0 auto; }}
        .stats {{ display: flex; gap: 20px; margin: 20px 0; }}
        .stat-card {{ border: 1px solid #ddd; padding: 15px; border-radius: 5px; }}
        .error {{ color: red; }}
        .nav {{ margin: 20px 0; }}
        .nav a {{ margin-right: 15px; text-decoration: none; color: #667eea; }}
        .nav a:hover {{ text-decoration: underline; }}
    </style>
</head>
<body>
    <div class="container">
        <h1>Symgraph Viewer</h1>
        <div class="nav">
            <a href="/">Dashboard</a>
            <a href="/graph.html">Dependency Graph</a>
        </div>
        <div id="content">Loading...</div>
        <div id="error" class="error"></div>
    </div>
    <script>
        fetch('/api/stats')
            .then(response => response.json())
            .then(data => {{
                if (data.error) {{
                    document.getElementById('error').textContent = data.error;
                }} else {{
                    document.getElementById('content').innerHTML = `
                        <div class="stats">
                            <div class="stat-card">
                                <h3>Files</h3>
                                <p>${{data.files || 0}}</p>
                            </div>
                            <div class="stat-card">
                                <h3>Symbols</h3>
                                <p>${{data.symbols || 0}}</p>
                            </div>
                            <div class="stat-card">
                                <h3>Edges</h3>
                                <p>${{data.edges || 0}}</p>
                            </div>
                        </div>
                        <p><a href="/graph.html">View Dependency Graph</a></p>
                    `;
                }}
            }})
            .catch(error => {{
                document.getElementById('error').textContent = 'Error loading data: ' + error.message;
            }});
    </script>
</body>
</html>
        '''

if __name__ == '__main__':
    print("Starting Symgraph web viewer on http://localhost:5000")
    app.run(debug=False, port=5000)
"#);

    // Write the Flask app to a temporary file
    let temp_dir = tempfile::TempDir::new()?;
    let app_file = temp_dir.path().join("symgraph_viewer.py");
    std::fs::write(&app_file, app_content)
        .map_err(|e| anyhow::anyhow!("Failed to write Flask app file to '{}': {}", app_file.display(), e))?;
    
    // Copy static files to temp directory
    let static_dir = temp_dir.path().join("static");
    std::fs::create_dir_all(&static_dir)?;
    
    // Get the path to our static files
    let current_dir = std::env::current_dir()?;
    let source_static = current_dir.join("crates/symgraph-cli/src/modules/static");
    
    // Copy all static files
    if source_static.exists() {
        for entry in std::fs::read_dir(source_static)? {
            let entry = entry?;
            let target_path = temp_dir.path().join(entry.file_name());
            std::fs::copy(entry.path(), &target_path)?;
        }
    }
    
    // Start the Flask server
    let mut process = Command::new("python")
        .current_dir(temp_dir.path())
        .arg(&app_file)
        .spawn()?;
    
    println!("Web viewer started at http://localhost:5000");
    println!("Press Ctrl+C to stop");
    
    // Wait for user to stop
    process.wait()?;
    
    // Clean up
    std::fs::remove_dir_all(temp_dir.path())
        .map_err(|e| anyhow::anyhow!("Failed to clean up temporary directory '{}': {}", temp_dir.path().display(), e))?;
    
    Ok(())
}

/// Handle API requests from web viewer.
pub fn handle_api_request(endpoint: &str, db_path: &str, search: Option<&str>) -> Result<()> {
    use symgraph_core::SymgraphDb;
    use serde_json::json;
    
    let db = SymgraphDb::open(db_path)?;
    
    let response = match endpoint {
        "stats" => {
            let stats = db.get_stats()?;
            json!({
                "files": stats.files,
                "symbols": stats.symbols,
                "edges": stats.edges
            })
        }
        "files" => {
            let files = if let Some(search_query) = search {
                db.search_files(search_query)?
            } else {
                db.list_files()?
            };
            json!(files)
        }
        "symbols" => {
            let symbols = if let Some(search_query) = search {
                db.search_symbols(search_query)?
            } else {
                db.list_symbols()?
            };
            json!(symbols)
        }
        "graph" => {
            let graph_data = build_graph_data(&db)?;
            json!(graph_data)
        }
        _ => {
            return Err(anyhow::anyhow!("Unknown API endpoint: {}", endpoint));
        }
    };
    
    println!("{}", serde_json::to_string_pretty(&response)?);
    Ok(())
}

/// Build graph data for Cytoscape visualization
fn build_graph_data(db: &symgraph_core::SymgraphDb) -> Result<serde_json::Value> {
    use serde_json::json;
    
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    
    // Get all files
    let files = db.list_files()?;
    for file in files {
        nodes.push(json!({
            "data": {
                "id": format!("file:{}", file.id),
                "label": file.path.split('/').last().unwrap_or(&file.path),
                "type": "file",
                "file": file.path,
                "language": file.language,
                "category": file.category,
                "purpose": file.purpose
            }
        }));
    }
    
    // Get all symbols
    let symbols = db.list_symbols()?;
    for symbol in symbols {
        let node_type = determine_symbol_type(&symbol.name);
        nodes.push(json!({
            "data": {
                "id": format!("symbol:{}", symbol.id),
                "label": &symbol.name,
                "type": node_type,
                "symbol": &symbol.name,
                "kind": &symbol.kind,
                "file": symbol.file_id
            }
        }));
        
        // Add edge from file to symbol
        edges.push(json!({
            "data": {
                "id": format!("contains:{}:{}", symbol.file_id, symbol.id),
                "source": format!("file:{}", symbol.file_id),
                "target": format!("symbol:{}", symbol.id),
                "type": "defines"
            }
        }));
    }
    
    // Get all edges/relationships
    // Note: This would need to be implemented in SymgraphDb
    // For now, we'll create some example relationships
    
    Ok(json!({
        "nodes": nodes,
        "edges": edges
    }))
}

/// Determine symbol type based on symbol name and kind
fn determine_symbol_type(symbol: &str) -> &str {
    if symbol.contains("::") {
        if symbol.to_lowercase().contains("class") || symbol.to_lowercase().contains("struct") {
            "class"
        } else {
            "function"
        }
    } else if symbol.chars().next().unwrap_or('_').is_uppercase() {
        "class"
    } else {
        "symbol"
    }
}
