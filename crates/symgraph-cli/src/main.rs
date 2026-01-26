//! # symgraph-cli
//!
//! Command-line tool for building semantic symbol graphs from C/C++ source code.
//!
//! ## Features
//! - Analyzes C/C++ code using libclang to extract symbols, references, and relationships
//! - Builds call graphs, inheritance hierarchies, and member relationships
//! - Imports C++20 module dependency graphs
//! - Generates compile_commands.json from CMake, Make, and Visual Studio projects
//! - Stores results in SQLite for querying
//!
//! ## Quick Start
//!
//! ### 1. Generate compile_commands.json (if needed)
//! ```bash
//! # Auto-detect build system and generate compile_commands.json
//! symgraph-cli generate-compdb --project /path/to/project
//!
//! # CMake project with Ninja generator
//! symgraph-cli generate-compdb --project . --generator Ninja --build-dir build
//!
//! # Visual Studio solution
//! symgraph-cli generate-compdb --project . --build-system solution --configuration Release
//!
//! # Makefile project (note: `bear -- make` gives better results)
//! symgraph-cli generate-compdb --project . --build-system make
//! ```
//!
//! ### 2. Analyze C/C++ code
//! ```bash
//! # Build symbol graph from compile_commands.json
//! symgraph-cli scan-cxx --compdb build/compile_commands.json --db project.db
//!
//! # The database now contains:
//! # - symbols: functions, classes, variables, etc.
//! # - occurrences: where each symbol is used
//! # - edges: call graph, inheritance, member relationships
//! ```
//!
//! ### 3. Import C++20 modules (optional)
//! ```bash
//! # Scan for .cppm/.ixx/.mxx files and build module dependency graph
//! symgraph-cli import-modules --root src/ --db project.db
//! ```
//!
//! ### 4. Query the symbol graph
//! ```bash
//! # List all functions called by main()
//! symgraph-cli query-calls --db project.db --usr "c:@F@main#"
//!
//! # USR examples:
//! #   c:@F@main#              - global function main()
//! #   c:@S@MyClass#           - class MyClass
//! #   c:@S@MyClass@F@method#  - method of class MyClass
//! #   c:@N@ns@F@func#         - function in namespace ns
//! ```
//!
//! ## Complete Workflow Example
//! ```bash
//! # 1. Clone a project
//! git clone https://github.com/example/project.git
//! cd project
//!
//! # 2. Generate compile_commands.json
//! symgraph-cli generate-compdb --project . --build-dir build
//!
//! # 3. Analyze the code
//! symgraph-cli scan-cxx --compdb build/compile_commands.json --db project.db
//!
//! # 4. Import C++20 modules (if any)
//! symgraph-cli import-modules --root src/ --db project.db
//!
//! # 5. Query: what does main() call?
//! symgraph-cli query-calls --db project.db --usr "c:@F@main#"
//! ```
//!
//! ## Database Schema
//! The SQLite database contains:
//! - `symbols`: id, file_id, usr, name, kind, is_definition
//! - `occurrences`: id, symbol_id, file_id, usage_kind, line, column
//! - `edges`: id, from_sym, to_sym, from_module, to_module, kind
//! - `modules`: id, name, kind, path
//! - `files`: id, path, lang
//!
//! Edge kinds: "call", "inherit", "member", "module-import"

use anyhow::Result;
use clang::{Clang, Index};
use clap::{Parser, Subcommand, ValueEnum};
use std::path::Path;
use std::fs;
use std::collections::HashMap;
use std::process::Command;
use symgraph_core::{insert_edge, insert_occurrence, insert_symbol, upsert_module, Db};
use symgraph_cxx::scan_tu;
use symgraph_discovery::load_compile_commands;

// Rust analysis crates for prototype
use cargo_metadata::MetadataCommand;
use walkdir::WalkDir;
use syn::visit::Visit;
use syn::{Expr, ExprCall};

/// symgraph CLI - Semantic symbol graph builder for C/C++ projects.
///
/// Extracts symbols, references, call graphs, inheritance hierarchies,
/// and C++20 module dependencies from source code.
#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    cmd: Cmd,
}

/// Тип системы сборки для явного указания
#[derive(Debug, Clone, ValueEnum)]
enum BuildSystemType {
    /// Автоматическое определение
    Auto,
    /// CMake проект (CMakeLists.txt)
    Cmake,
    /// Make проект (Makefile)
    Make,
    /// Visual Studio проект (.vcxproj)
    Vcxproj,
    /// Visual Studio решение (.sln)
    Solution,
    /// Cargo / Rust проект (Cargo.toml)
    Cargo,
}

#[derive(Subcommand)]
enum Cmd {
    /// Generate compile_commands.json from a build system.
    ///
    /// Automatically detects the build system (CMake, Make, Visual Studio, Cargo)
    /// and generates a compile_commands.json file for use with clang tools.
    ///
    /// Note for Cargo projects: this tool will attempt to use `cargo compdb` (from `cargo-compdb`). If it's not available,
    /// instructions will be printed explaining how to install it or generate `compile_commands.json` manually.
    ///
    /// # Supported build systems
    /// - CMake: runs cmake with CMAKE_EXPORT_COMPILE_COMMANDS=ON
    /// - Make: parses `make -n` output (or use `bear` for better results)
    /// - Visual Studio: parses .vcxproj/.sln files
    ///
    /// # Examples
    /// ```bash
    /// # Auto-detect build system in current directory
    /// symgraph-cli generate-compdb
    ///
    /// # CMake project with Ninja
    /// symgraph-cli generate-compdb --project ~/myproject --generator Ninja
    ///
    /// # Visual Studio solution, Release x64
    /// symgraph-cli generate-compdb --build-system solution --configuration Release --platform x64
    ///
    /// # Makefile project with custom output
    /// symgraph-cli generate-compdb --build-system make --output my_compdb.json
    /// ```
    GenerateCompdb {
        /// Path to project directory containing build files.
        ///
        /// Should contain CMakeLists.txt, Makefile, .vcxproj, .sln, or Cargo.toml
        #[arg(long, value_name = "DIR", default_value = ".")]
        project: String,

        /// Output path for compile_commands.json.
        ///
        /// If not specified, uses build/compile_commands.json for CMake
        /// or compile_commands.json in project directory for others.
        #[arg(long, short, value_name = "PATH")]
        output: Option<String>,

        /// Build directory for CMake (where to run cmake).
        ///
        /// Only used for CMake projects. Defaults to "build" subdirectory.
        #[arg(long, value_name = "DIR")]
        build_dir: Option<String>,

        /// CMake generator to use (e.g., "Ninja", "Unix Makefiles").
        ///
        /// Only used for CMake projects. Ninja is recommended.
        #[arg(long, value_name = "GEN", default_value = "Ninja")]
        generator: String,

        /// Build system type. Auto-detected if not specified.
        #[arg(long, value_name = "TYPE", value_enum, default_value = "auto")]
        build_system: BuildSystemType,

        /// Build configuration for Visual Studio (Debug, Release).
        #[arg(long, value_name = "CFG", default_value = "Debug")]
        configuration: String,

        /// Platform for Visual Studio (x64, Win32, ARM64).
        #[arg(long, value_name = "PLAT", default_value = "x64")]
        platform: String,
    },

    /// Analyze C/C++ source files using libclang.
    ///
    /// Parses all translation units from compile_commands.json,
    /// extracts symbols (functions, classes, variables), their definitions,
    /// references, and builds relationship graphs (calls, inheritance, members).
    ///
    /// # Examples
    /// ```bash
    /// # Basic usage with CMake project
    /// symgraph-cli scan-cxx --compdb build/compile_commands.json
    ///
    /// # Specify output database
    /// symgraph-cli scan-cxx --compdb build/compile_commands.json --db myproject.db
    ///
    /// # Full workflow: generate compdb then scan
    /// symgraph-cli generate-compdb --project ~/myproject
    /// symgraph-cli scan-cxx --compdb ~/myproject/compile_commands.json --db ~/myproject.db
    /// ```
    ScanCxx {
        /// Path to compile_commands.json generated by CMake/Ninja.
        ///
        /// This file contains compilation commands for each source file,
        /// typically generated with CMAKE_EXPORT_COMPILE_COMMANDS=ON.
        /// Use Ninja or Makefiles generator (not Visual Studio) for CMake.
        #[arg(long, value_name = "PATH")]
        compdb: String,

        /// Output SQLite database path for storing the symbol graph.
        ///
        /// The database will be created if it doesn't exist,
        /// or updated if it already exists.
        #[arg(long, value_name = "PATH", default_value = "symgraph.db")]
        db: String,
    },

    /// Import C++20 module dependency graph from source files.
    ///
    /// Scans directory for module interface files (.cppm, .ixx, .mxx),
    /// extracts module names and import declarations,
    /// and builds the module dependency graph.
    ///
    /// # Examples
    /// ```bash
    /// # Import modules from project source directory
    /// symgraph-cli import-modules --root ~/myproject/src
    ///
    /// # Use custom database
    /// symgraph-cli import-modules --root ./modules --db modules_graph.db
    ///
    /// # Import to same database as symbols
    /// symgraph-cli scan-cxx --compdb build/compile_commands.json --db project.db
    /// symgraph-cli import-modules --root src --db project.db
    /// ```
    ImportModules {
        /// Root directory to scan for C++20 module files.
        ///
        /// Will recursively search for files with extensions:
        /// .cppm, .ixx, .mxx (module interface units).
        #[arg(long, value_name = "DIR")]
        root: String,

        /// Output SQLite database path for storing module graph.
        #[arg(long, value_name = "PATH", default_value = "symgraph.db")]
        db: String,
    },

    /// Analyze C++20 modules without libclang (regex-based parsing).
    ///
    /// Parses C++20 module files directly to extract:
    /// - Exported functions, classes, structs, enums
    /// - Member functions and variables
    /// - Type references and inheritance relationships
    ///
    /// This works without needing compile_commands.json or libclang.
    ///
    /// # Examples
    /// ```bash
    /// # Scan all module files in a directory
    /// symgraph-cli scan-modules --root ~/myproject/modules --db project.db
    ///
    /// # Include .cxx and .cpp files with modules
    /// symgraph-cli scan-modules --root ~/myproject --db project.db
    /// ```
    ScanModules {
        /// Root directory to scan for C++20 module files.
        ///
        /// Scans for: .cppm, .ixx, .mxx, .cxx files
        #[arg(long, value_name = "DIR")]
        root: String,

        /// Output SQLite database path.
        #[arg(long, value_name = "PATH", default_value = "symgraph.db")]
        db: String,
    },

    /// Analyze Rust project without needing compile_commands.json.
    ///
    /// Uses `cargo_metadata` + local parsing (syn) or optional LSIF input to
    /// extract function definitions and call relationships.
    ScanRust {
        /// Path to Cargo.toml or project root.
        #[arg(long, value_name = "PATH", default_value = ".")]
        manifest_path: String,

        /// Optional LSIF JSON file to use (generated by `rust-analyzer lsif`).
        #[arg(long, value_name = "PATH")]
        lsif: Option<String>,

        /// Output SQLite database path.
        #[arg(long, value_name = "PATH", default_value = "symgraph.db")]
        db: String,
    },

    /// Analyze script projects (Python, JavaScript, TypeScript) with categorization.
    ///
    /// Scans directory for script files, categorizes them (entry point, test, core logic, etc.),
    /// extracts symbols and dependencies, and generates project annotations.
    ScanScripts {
        /// Root directory to scan for script files.
        #[arg(long, value_name = "DIR")]
        root: String,

        /// Output SQLite database path.
        #[arg(long, value_name = "PATH", default_value = "symgraph.db")]
        db: String,
    },

    /// Generate project annotation with AI-assisted analysis.
    ///
    /// Analyzes project structure, categorizes files, infers purpose,
    /// and generates comprehensive project documentation.
    AnnotateProject {
        /// Root directory of the project.
        #[arg(long, value_name = "DIR")]
        root: String,

        /// Output SQLite database path.
        #[arg(long, value_name = "PATH", default_value = "symgraph.db")]
        db: String,
    },

    /// Query the call graph: list functions called by a given function.
    ///
    /// Uses the USR (Unified Symbol Resolution) identifier from libclang
    /// to uniquely identify the source function.
    ///
    /// # Examples
    /// ```bash
    /// # Query calls from main function
    /// symgraph-cli query-calls --db project.db --usr "c:@F@main#"
    ///
    /// # Query calls from a class method
    /// symgraph-cli query-calls --db project.db --usr "c:@S@MyClass@F@process#"
    ///
    /// # Query calls from a namespaced function
    /// symgraph-cli query-calls --db project.db --usr "c:@N@utils@F@helper#I#"
    /// ```
    ///
    /// # USR Format
    /// USR (Unified Symbol Resolution) is libclang's unique identifier:
    /// - `c:@F@name#` - free function
    /// - `c:@S@ClassName@F@method#` - class method  
    /// - `c:@N@namespace@F@func#` - namespaced function
    /// - Parameter types may be appended (e.g., `#I#` for int)
    QueryCalls {
        /// Path to the SQLite database with the symbol graph.
        #[arg(long, value_name = "PATH")]
        db: String,

        /// USR (Unified Symbol Resolution) of the caller function.
        ///
        /// USR is a unique identifier for symbols in libclang.
        /// Example: "c:@F@main#" for the main() function,
        ///          "c:@S@MyClass@F@method#" for a class method.
        #[arg(long, value_name = "USR")]
        usr: String,
    },

    /// List all modules and their dependencies from the database.
    ///
    /// Shows imported C++20 modules and their import relationships.
    ListModules {
        /// Path to the SQLite database with the symbol graph.
        #[arg(long, value_name = "PATH")]
        db: String,
    },

    /// Show database statistics (counts of symbols, edges, modules).
    Stats {
        /// Path to the SQLite database.
        #[arg(long, value_name = "PATH")]
        db: String,
    },
}

fn main() -> Result<()> {
    let args = Args::parse();
    match args.cmd {
        Cmd::GenerateCompdb {
            project,
            output,
            build_dir,
            generator,
            build_system,
            configuration,
            platform,
        } => generate_compdb(
            &project,
            output.as_deref(),
            build_dir.as_deref(),
            &generator,
            build_system,
            &configuration,
            &platform,
        )?,
        Cmd::ScanCxx { compdb, db } => scan_cxx(&compdb, &db)?,
        Cmd::ImportModules { root, db } => import_modules(&root, &db)?,
        Cmd::ScanModules { root, db } => scan_modules(&root, &db)?,
        Cmd::ScanRust { manifest_path, lsif, db } => scan_rust(&manifest_path, lsif.as_deref(), &db)?,
        Cmd::ScanScripts { root, db } => scan_scripts(&root, &db)?,
        Cmd::AnnotateProject { root, db } => {
            // Detect if it's a script project or compiled project
            let has_scripts = std::fs::read_dir(&root)?
                .filter_map(|e| e.ok())
                .any(|entry| {
                    if let Some(ext) = entry.path().extension().and_then(|s| s.to_str()) {
                        matches!(ext, "py" | "js" | "ts")
                    } else {
                        false
                    }
                });
            
            if has_scripts {
                annotate_project(&root, &db)?
            } else {
                annotate_compiled_project(&root, &db)?
            }
        },
        Cmd::QueryCalls { db, usr } => query_calls(&db, &usr)?,
        Cmd::ListModules { db } => list_modules(&db)?,
        Cmd::Stats { db } => show_stats(&db)?,
    }
    Ok(())
}

/// Generates compile_commands.json from a project's build system.
///
/// Automatically detects or uses the specified build system to generate
/// a compile_commands.json file suitable for use with libclang and other
/// clang-based tools.
///
/// # Supported build systems
/// - **CMake**: Runs `cmake` with `-DCMAKE_EXPORT_COMPILE_COMMANDS=ON`
/// - **Make**: Parses `make -n` dry-run output
/// - **Visual Studio**: Parses .vcxproj/.sln XML files
///
/// # Arguments
/// * `project` - Path to project directory
/// * `output` - Output path for compile_commands.json (optional)
/// * `build_dir` - Build directory for CMake (optional)
/// * `generator` - CMake generator (e.g., "Ninja")
/// * `build_system` - Explicit build system type or Auto
/// * `configuration` - VS configuration (Debug/Release)
/// * `platform` - VS platform (x64/Win32)
fn generate_compdb(
    project: &str,
    output: Option<&str>,
    build_dir: Option<&str>,
    generator: &str,
    build_system: BuildSystemType,
    configuration: &str,
    platform: &str,
) -> Result<()> {
    use symgraph_discovery::{
        detect_build_system, generate_from_cmake, generate_from_makefile, generate_from_solution,
        generate_from_vcxproj, generate_from_cargo, BuildSystem,
    };

    let project_path = Path::new(project);

    // Определяем систему сборки
    let detected_system = match build_system {
        BuildSystemType::Auto => detect_build_system(project_path),
        BuildSystemType::Cmake => BuildSystem::CMake,
        BuildSystemType::Make => BuildSystem::Make,
        BuildSystemType::Vcxproj => BuildSystem::VcxProj,
        BuildSystemType::Solution => BuildSystem::Solution,
        BuildSystemType::Cargo => BuildSystem::Cargo,
    };

    println!("Detected build system: {:?}", detected_system);

    // Определяем путь вывода
    let default_output = project_path.join("compile_commands.json");
    let output_path = output.map(Path::new).unwrap_or(&default_output);

    // Генерируем compile_commands.json в зависимости от системы сборки
    let result_path = match detected_system {
        BuildSystem::CMake => {
            let default_build = project_path.join("build");
            let build = build_dir.map(Path::new).unwrap_or(&default_build);
            println!("Running CMake with generator '{}'...", generator);
            generate_from_cmake(project_path, build, Some(generator), &[])?
        }
        BuildSystem::Make => {
            println!("Parsing Makefile...");
            println!("Note: For better results, consider using 'bear -- make' instead.");
            generate_from_makefile(project_path, output_path, &[])?
        }
        BuildSystem::VcxProj => {
            // Находим .vcxproj файл
            let vcxproj = find_file_with_ext(project_path, "vcxproj")?;
            println!("Parsing Visual Studio project: {}", vcxproj.display());
            generate_from_vcxproj(&vcxproj, output_path, configuration, platform)?
        }
        BuildSystem::Solution => {
            // Находим .sln файл
            let sln = find_file_with_ext(project_path, "sln")?;
            println!("Parsing Visual Studio solution: {}", sln.display());
            generate_from_solution(&sln, output_path, configuration, platform)?
        }
        BuildSystem::Cargo => {
            println!("Detected Cargo project in {}. Using `rust-analyzer lsif` to generate LSIF output.", project);
            println!("To generate manually: `rust-analyzer lsif . > compile_commands.json`. Attempting to run `rust-analyzer lsif` now...");
            // Pass build_dir (if the user supplied it) through to generator (currently unused by LSIF flow)
            let build_dir_path = build_dir.map(|s| Path::new(s));
            generate_from_cargo(project_path, output_path, build_dir_path)?
        }
        BuildSystem::Unknown => {
            anyhow::bail!(
                "Could not detect build system in '{}'. \n\
                 Supported: CMakeLists.txt, Makefile, .vcxproj, .sln, Cargo.toml\n\
                 Use --build-system to specify explicitly.",
                project
            );
        }
    };

    println!("Generated: {}", result_path.display());
    Ok(())
}

/// Находит файл с указанным расширением в директории
fn find_file_with_ext(dir: &Path, ext: &str) -> Result<std::path::PathBuf> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map(|e| e == ext).unwrap_or(false) {
            return Ok(path);
        }
    }
    anyhow::bail!("No .{} file found in {}", ext, dir.display())
}

/// Analyzes C/C++ source files using libclang and stores results in SQLite.
///
/// This function performs the following steps:
/// 1. Initializes libclang and creates an index for parsing
/// 2. Loads compilation commands from compile_commands.json
/// 3. For each translation unit:
///    - Parses the source file with appropriate compiler flags
///    - Extracts symbols (functions, classes, variables, etc.)
///    - Extracts occurrences (where symbols are used)
///    - Extracts edges (call, inheritance, member relationships)
/// 4. Stores all extracted data in the SQLite database
///
/// # Arguments
/// * `compdb` - Path to compile_commands.json
/// * `db_path` - Path to output SQLite database
///
/// # Errors
/// Returns an error if libclang initialization fails, database cannot be opened,
/// or if there are issues reading the compile commands.
fn scan_cxx(compdb: &str, db_path: &str) -> Result<()> {
    use symgraph_cxx::{categorize_cpp_file, infer_cpp_purpose};
    
    let clang = Clang::new().map_err(|e| anyhow::anyhow!("{}", e))?;
    let index = Index::new(&clang, false, true);
    let cmds = load_compile_commands(compdb)?;
    let mut db = Db::open(db_path)?;
    let project_id = db.ensure_project("cpp_project", ".")?;

    for cc in cmds {
        let args = if let Some(a) = cc.arguments {
            a
        } else if let Some(cmd) = cc.command {
            shell_words::split(&cmd)?
        } else {
            Vec::new()
        };

        let clean_args: Vec<String> = args
            .iter()
            .skip_while(|a| {
                a.ends_with("cl")
                    || a.ends_with("clang")
                    || a.ends_with("clang++")
                    || a.ends_with("gcc")
                    || a.ends_with("g++")
            })
            .cloned()
            .collect();

        let clean_args_refs: Vec<&str> = clean_args.iter().map(|s| s.as_str()).collect();

        let tu = match index.parser(&cc.file).arguments(&clean_args_refs).parse() {
            Ok(tu) => tu,
            Err(e) => {
                eprintln!("parse failed for {}: {:?}", cc.file, e);
                continue;
            }
        };

        // Categorize file
        let category = categorize_cpp_file(&cc.file);
        let purpose = infer_cpp_purpose(&cc.file, &category);
        let category_str = format!("{:?}", category).to_lowercase();
        
        let (symbols, occs, edges) = scan_tu(&tu);

        for s in symbols {
            let fid = db.ensure_file_with_category(
                project_id, &s.file, "c++", Some(&category_str), Some(&purpose)
            )?;
            let _sid = insert_symbol(
                &mut db.conn,
                fid,
                s.usr.as_deref(),
                None,
                &s.name,
                &s.kind,
                s.is_definition,
            )?;
        }

        for o in occs {
            if let Some(usr) = o.usr.as_deref() {
                if let Some(sid) = db.find_symbol_by_usr(usr)? {
                    let fid = db.ensure_file_with_category(
                        project_id, &o.file, "c++", Some(&category_str), Some(&purpose)
                    )?;
                    let _oid =
                        insert_occurrence(&mut db.conn, sid, fid, &o.usage_kind, o.line, o.column)?;
                }
            }
        }

        for (kind, from, to) in edges {
            let from_id = db.find_symbol_by_usr(&from)?;
            let to_id = db.find_symbol_by_usr(&to)?;
            if let (Some(fs), Some(ts)) = (from_id, to_id) {
                let _eid = insert_edge(&mut db.conn, Some(fs), Some(ts), None, None, &kind)?;
            }
        }
    }
    Ok(())
}

/// Imports C++20 module dependency graph from source files.
///
/// Scans a directory tree for C++20 module interface files and extracts:
/// - Module names (from `export module <name>;` declarations)
/// - Import dependencies (from `import <name>;` declarations)
///
/// This creates a module-level dependency graph stored in the database,
/// useful for understanding project structure and build ordering.
///
/// # Arguments
/// * `root` - Root directory to scan for module files
/// * `db_path` - Path to output SQLite database
///
/// # Supported file extensions
/// - `.cppm` - Standard C++20 module interface
/// - `.ixx` - MSVC module interface
/// - `.mxx` - Alternative module interface extension
fn import_modules(root: &str, db_path: &str) -> Result<()> {
    use symgraph_cxx::modules::scan_cpp20_module;
    use walkdir::WalkDir;

    let mut db = Db::open(db_path)?;

    // Recursively walk the directory tree
    for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path().display().to_string();

        // Check for module interface file extensions
        if p.ends_with(".cppm") || p.ends_with(".ixx") || p.ends_with(".mxx") {
            // Parse the module file to extract name and imports
            if let Some(mi) = scan_cpp20_module(&p)? {
                // Insert or get the module record
                let mid = upsert_module(&mut db.conn, &mi.name, "cpp20-module", &mi.path)?;

                // Also register as a file for cross-referencing
                let _fid = db.ensure_file(&mi.path, "c++")?;

                // Create edges for each import dependency
                for imp in mi.imports {
                    // Create placeholder for imported module (may not exist yet)
                    let to = upsert_module(&mut db.conn, &imp, "cpp20-module", "")?;
                    // Record the import relationship
                    let _eid = insert_edge(
                        &mut db.conn,
                        None,
                        None,
                        Some(mid),
                        Some(to),
                        "module-import",
                    )?;
                }
            }
        }
    }
    Ok(())
}

/// Scans C++20 modules and extracts symbols without libclang.
///
/// Uses regex-based parsing to extract:
/// - Exported functions, classes, structs, enums
/// - Member functions and variables
/// - Type references and inheritance relationships
///
/// This is useful when libclang cannot parse the files (e.g., C++20 modules with
/// CMake-generated compile_commands.json containing @modmap response files).
fn scan_modules(root: &str, db_path: &str) -> Result<()> {
    use symgraph_cxx::modules::analyze_cpp_module;
    use walkdir::WalkDir;

    let mut db = Db::open(db_path)?;
    let mut file_count = 0;
    let mut symbol_count = 0;
    let mut relation_count = 0;

    // Recursively walk the directory tree
    for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path();
        let path_str = p.display().to_string();

        // Check for module file extensions
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !["cppm", "ixx", "mxx", "cxx"].contains(&ext) {
            continue;
        }

        // Analyze the module file
        match analyze_cpp_module(&path_str) {
            Ok(Some(analysis)) => {
                file_count += 1;

                // Register the module
                let mid = upsert_module(
                    &mut db.conn,
                    &analysis.info.name,
                    "cpp20-module",
                    &analysis.info.path,
                )?;
                let fid = db.ensure_file(&analysis.info.path, "c++")?;

                // Import module dependencies
                for imp in &analysis.info.imports {
                    let to = upsert_module(&mut db.conn, imp, "cpp20-module", "")?;
                    let _eid = insert_edge(
                        &mut db.conn,
                        None,
                        None,
                        Some(mid),
                        Some(to),
                        "module-import",
                    )?;
                    relation_count += 1;
                }

                // Insert symbols
                for sym in &analysis.symbols {
                    // Create a pseudo-USR for the symbol
                    let usr = format!("module:{}:{}", analysis.info.name, sym.name);
                    let _sid = insert_symbol(
                        &mut db.conn,
                        fid,
                        Some(&usr),
                        None,
                        &sym.name,
                        &sym.kind,
                        sym.is_exported,
                    )?;
                    symbol_count += 1;
                }

                // Insert relations
                for rel in &analysis.relations {
                    // For now, just count them - full edge insertion would need symbol lookup
                    relation_count += 1;

                    // Try to find symbols and create edges
                    let from_usr = format!("module:{}:{}", analysis.info.name, rel.from_name);
                    let to_usr = format!("module:{}:{}", analysis.info.name, rel.to_name);

                    if let (Some(from_id), Some(to_id)) = (
                        db.find_symbol_by_usr(&from_usr)?,
                        db.find_symbol_by_usr(&to_usr)?,
                    ) {
                        let _eid = insert_edge(
                            &mut db.conn,
                            Some(from_id),
                            Some(to_id),
                            None,
                            None,
                            &rel.kind,
                        )?;
                    }
                }

                println!(
                    "  {} - {} symbols, {} relations",
                    analysis.info.name,
                    analysis.symbols.len(),
                    analysis.relations.len()
                );
            }
            Ok(None) => {
                // Not a module file, skip
            }
            Err(e) => {
                eprintln!("Error parsing {}: {}", path_str, e);
            }
        }
    }

    println!("\n=== Summary ===");
    println!("Files processed: {}", file_count);
    println!("Symbols extracted: {}", symbol_count);
    println!("Relations found: {}", relation_count);

    Ok(())
}

/// Generate LSIF file using rust-analyzer.
fn generate_lsif_file(project_dir: &Path, output_path: &Path) -> Result<()> {
    // Allow override via environment variable (for tests/custom paths)
    let ra_bin = std::env::var("SYGRAPH_RUST_ANALYZER_CMD").unwrap_or_else(|_| "rust-analyzer".to_string());

    // Run: `rust-analyzer lsif .` and write its stdout to the output path
    let mut cmd = Command::new(&ra_bin);
    cmd.arg("lsif").arg(".").current_dir(project_dir);

    let output = cmd.output()
        .map_err(|e| anyhow::anyhow!(
            "Failed to run `rust-analyzer` (is it installed and on PATH?): {}\n\
             Install it via rustup or from release and try again.",
            e
        ))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!(
            "`rust-analyzer lsif` failed: {}\n\
             Suggestions:\n\
             1) Ensure `rust-analyzer` is installed and on PATH (install via rustup or from release).\n\
             2) Or generate manually: `rust-analyzer lsif . > {}` and re-run the command.\n\
             Original output: {}",
            stderr,
            output_path.display(),
            stderr
        );
    }

    let stdout = String::from_utf8(output.stdout)
        .map_err(|_| anyhow::anyhow!("Failed to read stdout from rust-analyzer"))?;
    
    // Create parent directory if needed
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    
    fs::write(output_path, stdout)
        .map_err(|e| anyhow::anyhow!("Failed to write {}: {}", output_path.display(), e))?;
    
    Ok(())
}

/// Analyze Rust projects: collect functions and call edges using `cargo_metadata` + `syn`.
fn scan_rust(manifest_path: &str, lsif: Option<&str>, db_path: &str) -> Result<()> {
    // Resolve manifest path and metadata
    let mut cmd = MetadataCommand::new();
    // If user passed a directory, try manifest in that dir
    let mpath = Path::new(manifest_path);
    let workspace_root = if mpath.is_dir() {
        mpath
    } else if mpath.is_file() {
        mpath.parent().unwrap_or(Path::new("."))
    } else {
        Path::new(".")
    };
    
    let cargo_toml_path = workspace_root.join("Cargo.toml");
    cmd.manifest_path(&cargo_toml_path);
    let metadata = cmd.exec()?;
    
    // Handle both single package and workspace cases
    let packages = if let Some(root) = metadata.root_package() {
        vec![root.clone()]
    } else if !metadata.workspace_members.is_empty() {
        // Use cargo_metadata workspace members - this is the reliable way
        println!("Found workspace with {} members from cargo_metadata", metadata.workspace_members.len());
        metadata.workspace_members
            .iter()
            .filter_map(|id| metadata.packages.iter().find(|p| &p.id == id).cloned())
            .collect()
    } else {
        anyhow::bail!("Could not find root package or workspace members. Make sure you're running this from a Rust project directory with a Cargo.toml file.");
    };
    
    // Open database once for all packages
    let mut db = Db::open(db_path)?;
    
    // Parse LSIF once for the entire workspace (if provided)
    if let Some(lsif_path) = lsif {
        // Resolve LSIF path relative to manifest_path directory
        let lsif_resolved = if Path::new(lsif_path).is_absolute() {
            Path::new(lsif_path).to_path_buf()
        } else {
            workspace_root.join(lsif_path)
        };
        
        if lsif_resolved.exists() {
            println!("Parsing LSIF file: {}", lsif_resolved.display());
            // Parse LSIF without specific crate_name - it will contain info for all packages
            if let Err(e) = parse_lsif_and_insert(lsif_resolved.to_str().unwrap(), &mut db, "workspace") {
                eprintln!("Warning: Failed to parse LSIF {}: {}", lsif_resolved.display(), e);
                eprintln!("Continuing with source code analysis only...");
            }
        } else {
            // Try to generate LSIF file automatically
            println!("LSIF file not found: {}", lsif_resolved.display());
            println!("Attempting to generate it using rust-analyzer...");
            
            let project_dir = if mpath.is_dir() {
                mpath
            } else {
                mpath.parent().unwrap_or(Path::new("."))
            };
            
            match generate_lsif_file(project_dir, &lsif_resolved) {
                Ok(_) => {
                    println!("Successfully generated LSIF file: {}", lsif_resolved.display());
                    // Now parse the generated file
                    if let Err(e) = parse_lsif_and_insert(lsif_resolved.to_str().unwrap(), &mut db, "workspace") {
                        eprintln!("Warning: Failed to parse generated LSIF {}: {}", lsif_resolved.display(), e);
                        eprintln!("Continuing with source code analysis only...");
                    }
                }
                Err(e) => {
                    eprintln!("Warning: Failed to generate LSIF file: {}", e);
                    eprintln!("Continuing with source code analysis only...");
                }
            }
        }
    }
    
    // Process each package
    for package in packages {
        let crate_name = package.name.clone();
        let manifest_dir = Path::new(&package.manifest_path)
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| Path::new(".").to_path_buf());
        
        println!("Processing package: {} (from {})", crate_name, manifest_dir.display());
        
        // Process this package (rest of the function logic)
        process_rust_package(&crate_name, &manifest_dir, &mut db)?;
    }
    
    // Also scan workspace-level examples and tests directories if they exist
    // Check if we're in a workspace by looking at the workspace members
    if metadata.workspace_members.len() > 1 {
        let workspace_root_path = metadata.workspace_root.as_std_path();
        let workspace_examples = workspace_root_path.join("examples");
        let workspace_tests = workspace_root_path.join("tests");
        
        // Scan workspace-level examples
        if workspace_examples.exists() && workspace_examples.is_dir() {
            println!("Scanning workspace-level examples directory...");
            process_workspace_extra_dir(workspace_examples.as_path(), "rust", &mut db)?;
        }
        
        // Scan workspace-level tests
        if workspace_tests.exists() && workspace_tests.is_dir() {
            println!("Scanning workspace-level tests directory...");
            process_workspace_extra_dir(workspace_tests.as_path(), "rust", &mut db)?;
        }
    }
    
    Ok(())
}

/// Process workspace-level extra directories (examples, tests) that don't belong to a specific package.
fn process_workspace_extra_dir(dir_path: &Path, language: &str, db: &mut Db) -> Result<()> {
    use symgraph_rust::{categorize_rust_file, infer_rust_purpose};
    
    #[derive(Default)]
    struct V {
        symbols: Vec<String>,
        calls: Vec<(String, String)>,
        current_fn: Vec<String>,
    }

    impl<'ast> Visit<'ast> for V {
        fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
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
                    if let Some(caller) = self.current_fn.last() {
                        self.calls.push((caller.clone(), callee));
                    }
                }
            }
            syn::visit::visit_expr_call(self, node);
        }
    }

    let mut name_to_usr: HashMap<String, String> = HashMap::new();
    let project_id = db.ensure_project("workspace_extras", dir_path.parent().unwrap_or(dir_path).to_str().unwrap_or("."))?;

    for entry in WalkDir::new(dir_path).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path();
        if p.is_file() && p.extension().and_then(|e| e.to_str()) == Some("rs") {
            let path_str = p.display().to_string();
            
            // Categorize Rust file
            let category = categorize_rust_file(&path_str);
            let purpose = infer_rust_purpose(&path_str, &category);
            let category_str = format!("{:?}", category).to_lowercase();
            
            let s = fs::read_to_string(p)?;
            match syn::parse_file(&s) {
                Ok(parsed) => {
                    let mut v = V::default();
                    v.visit_file(&parsed);

                    for sym in v.symbols.iter() {
                        let fid = db.ensure_file_with_category(
                            project_id, &path_str, language, Some(&category_str), Some(&purpose)
                        )?;
                        let usr = format!("r:@workspace@{}", sym);
                        if db.find_symbol_by_usr(&usr)?.is_none() {
                            let _sid = insert_symbol(&mut db.conn, fid, Some(&usr), None, sym, "function", true)?;
                        }
                        name_to_usr.insert(sym.clone(), usr);
                    }

                    for (caller, callee) in v.calls.iter() {
                        let caller_usr = name_to_usr.get(caller);
                        let callee_usr = name_to_usr.get(callee);
                        if let (Some(cu), Some(du)) = (caller_usr, callee_usr) {
                            if let (Some(cs), Some(ds)) = (db.find_symbol_by_usr(cu)?, db.find_symbol_by_usr(du)?) {
                                let _eid = insert_edge(&mut db.conn, Some(cs), Some(ds), None, None, "call")?;
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("parse failed for {}: {}", path_str, e);
                }
            }
        }
    }

    Ok(())
}

/// Process a single Rust package: collect functions and call edges.
fn process_rust_package(crate_name: &str, manifest_dir: &Path, db: &mut Db) -> Result<()> {
    use symgraph_rust::{categorize_rust_file, infer_rust_purpose};
    
    #[derive(Default)]
    struct V {
        symbols: Vec<String>,
        calls: Vec<(String, String)>,
        current_fn: Vec<String>,
    }

    impl<'ast> Visit<'ast> for V {
        fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
            let name = node.sig.ident.to_string();
            self.symbols.push(name.clone());
            self.current_fn.push(name);
            syn::visit::visit_item_fn(self, node);
            self.current_fn.pop();
        }

        fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
            let name = node.ident.to_string();
            self.symbols.push(name.clone());
            syn::visit::visit_item_struct(self, node);
        }

        fn visit_item_enum(&mut self, node: &'ast syn::ItemEnum) {
            let name = node.ident.to_string();
            self.symbols.push(name.clone());
            syn::visit::visit_item_enum(self, node);
        }

        fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
            let name = node.ident.to_string();
            self.symbols.push(name.clone());
            syn::visit::visit_item_mod(self, node);
        }

        fn visit_item_trait(&mut self, node: &'ast syn::ItemTrait) {
            let name = node.ident.to_string();
            self.symbols.push(name.clone());
            syn::visit::visit_item_trait(self, node);
        }

        fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
            // For impl blocks, we don't have a name in the same way
            // but we can track the type being implemented
            if let syn::Type::Path(type_path) = &*node.self_ty {
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
                    if let Some(caller) = self.current_fn.last() {
                        self.calls.push((caller.clone(), callee));
                    }
                }
            }
            syn::visit::visit_expr_call(self, node);
        }
    }

    let mut name_to_usr: HashMap<String, String> = HashMap::new();
    let project_id = db.ensure_project(crate_name, manifest_dir.to_str().unwrap_or("."))?;

    // Define directories to scan: src, examples, tests
    let scan_dirs = vec![
        ("src", "source"),
        ("examples", "examples"),
        ("tests", "tests")
    ];

    for (dir_name, _dir_purpose) in scan_dirs {
        let scan_root = manifest_dir.join(dir_name);
        if scan_root.exists() && scan_root.is_dir() {
            println!("  Scanning {} directory...", dir_name);
            for entry in WalkDir::new(&scan_root).into_iter().filter_map(|e| e.ok()) {
                let p = entry.path();
                if p.is_file() && p.extension().and_then(|e| e.to_str()) == Some("rs") {
                    let path_str = p.display().to_string();
                    
                    // Categorize Rust file
                    let category = categorize_rust_file(&path_str);
                    let purpose = infer_rust_purpose(&path_str, &category);
                    let category_str = format!("{:?}", category).to_lowercase();
                    
                    let s = fs::read_to_string(p)?;
                    match syn::parse_file(&s) {
                        Ok(parsed) => {
                            let mut v = V::default();
                            v.visit_file(&parsed);

                            // Add file once and get its ID
                            let fid = db.ensure_file_with_category(
                                project_id, &path_str, "rust", Some(&category_str), Some(&purpose)
                            )?;

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
                                    let _sid = insert_symbol(&mut db.conn, fid, Some(&usr), None, sym, kind, true)?;
                                }
                                name_to_usr.insert(sym.clone(), usr);
                            }

                            // Add call edges
                            for (caller, callee) in v.calls.iter() {
                                let caller_usr = name_to_usr.get(caller);
                                let callee_usr = name_to_usr.get(callee);
                                if let (Some(cu), Some(du)) = (caller_usr, callee_usr) {
                                    if let (Some(cs), Some(ds)) = (db.find_symbol_by_usr(cu)?, db.find_symbol_by_usr(du)?) {
                                        let _eid = insert_edge(&mut db.conn, Some(cs), Some(ds), None, None, "call")?;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("parse failed for {}: {}", path_str, e);
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

/// Parse minimal LSIF (rust-analyzer) and insert definitions/references into DB.
fn parse_lsif_and_insert(lsif_path: &str, db: &mut Db, crate_name: &str) -> Result<()> {
    use serde_json::Value;

    let content = fs::read_to_string(lsif_path)?;
    // Try parse as JSON array, otherwise as line-delimited JSON
    let mut items: Vec<Value> = if let Ok(v) = serde_json::from_str::<Value>(&content) {
        match v {
            Value::Array(a) => a,
            _ => vec![v],
        }
    } else {
        let mut vec = Vec::new();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let v: Value = serde_json::from_str(line)?;
            vec.push(v);
        }
        vec
    };

    // Maps
    let mut vertex_map: HashMap<i64, Value> = HashMap::new();
    let mut edges: Vec<Value> = Vec::new();

    for it in items.drain(..) {
        if let Some(id) = it.get("id").and_then(|v| v.as_i64()) {
            if it.get("type").and_then(|t| t.as_str()) == Some("vertex") {
                vertex_map.insert(id, it);
            } else {
                edges.push(it);
            }
        }
    }

    // Build document map: doc_id -> uri
    let mut doc_uri: HashMap<i64, String> = HashMap::new();
    for (id, v) in &vertex_map {
        if let Some(label) = v.get("label").and_then(|l| l.as_str()) {
            if label == "document" || label == "textDocument" {
                if let Some(uri) = v.get("data").and_then(|d| d.get("uri")).and_then(|u| u.as_str()) {
                    doc_uri.insert(*id, uri.to_string());
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
    let mut ranges: HashMap<i64, RangeInfo> = HashMap::new();

    for (id, v) in &vertex_map {
        if let Some(label) = v.get("label").and_then(|l| l.as_str()) {
            if label == "range" {
                if let Some(data) = v.get("data") {
                    let start = data.get("start");
                    let end = data.get("end");
                    if let (Some(s), Some(e)) = (start, end) {
                        let sl = s.get("line").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
                        let sc = s.get("character").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
                        let el = e.get("line").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
                        let ec = e.get("character").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
                        ranges.insert(*id, RangeInfo { start_line: sl, start_char: sc, end_line: el, end_char: ec });
                    }
                }
            }
        }
    }

    // Map range -> document by processing 'contains' edges
    let mut range_doc: HashMap<i64, String> = HashMap::new();
    for e in &edges {
        if e.get("label").and_then(|l| l.as_str()) == Some("contains") {
            // handle single inV or array inVs
            let outv_opt = e.get("outV").and_then(|v| v.as_i64());
            if let Some(outv) = outv_opt {
                if let Some(invs) = e.get("inVs").and_then(|v| v.as_array()) {
                    for inv_val in invs {
                        if let Some(inv) = inv_val.as_i64() {
                            if let Some(uri) = doc_uri.get(&outv) {
                                range_doc.insert(inv, uri.clone());
                            }
                        }
                    }
                } else if let Some(inv) = e.get("inV").and_then(|v| v.as_i64()) {
                    if let Some(uri) = doc_uri.get(&outv) {
                        range_doc.insert(inv, uri.clone());
                    }
                }
            }
        }
    }

    // Process 'item' edges for definitions and references
    let mut def_ranges_for_result: HashMap<i64, Vec<i64>> = HashMap::new();
    let mut ref_ranges_for_result: HashMap<i64, Vec<i64>> = HashMap::new();

    for e in &edges {
        if e.get("label").and_then(|l| l.as_str()) == Some("item") {
            let prop = e.get("data").and_then(|d| d.get("property")).and_then(|p| p.as_str()).unwrap_or("");
            if let Some(outv) = e.get("outV").and_then(|v| v.as_i64()) {
                // support both single inV and array inVs
                if let Some(invs) = e.get("inVs").and_then(|v| v.as_array()) {
                    for inv_val in invs {
                        if let Some(inv) = inv_val.as_i64() {
                            if prop == "definitions" {
                                def_ranges_for_result.entry(outv).or_default().push(inv);
                            } else if prop == "references" {
                                ref_ranges_for_result.entry(outv).or_default().push(inv);
                            }
                        }
                    }
                } else if let Some(inv) = e.get("inV").and_then(|v| v.as_i64()) {
                    if prop == "definitions" {
                        def_ranges_for_result.entry(outv).or_default().push(inv);
                    } else if prop == "references" {
                        ref_ranges_for_result.entry(outv).or_default().push(inv);
                    }
                }
            }
        }
    }

    // For each resultSet (outv), get symbol name from definition range, then insert symbol and references
    for (result_id, def_ranges) in def_ranges_for_result {
        if def_ranges.is_empty() { continue; }
        // pick first def range
        let rid = def_ranges[0];
        if let (Some(range), Some(uri)) = (ranges.get(&rid), range_doc.get(&rid)) {
            // Convert URI to path
            let mut path = uri.clone();
            if path.starts_with("file://") {
                path = path.trim_start_matches("file://").to_string();
                // On Windows remove leading slash if present (file:///C:/...)
                if cfg!(windows) && path.starts_with('/') && path.chars().nth(2) == Some(':') {
                    path = path.trim_start_matches('/').to_string();
                }
            }
            // Read file and extract text span
            if let Ok(text) = fs::read_to_string(&path) {
                let lines: Vec<&str> = text.lines().collect();
                if range.start_line < lines.len() {
                    if range.start_line == range.end_line {
                        let line = lines[range.start_line];
                        let start = range.start_char.min(line.len());
                        let end = range.end_char.min(line.len());
                        let snippet = &line[start..end];
                        let name = extract_ident(snippet);
                        if !name.is_empty() {
                            let fid = db.ensure_file(&path, "rust")?;
                            let usr = format!("r:lsif:{}:{}", crate_name, name);
                            let sid = insert_symbol(&mut db.conn, fid, Some(&usr), None, &name, "function", true)?;

                            // Insert references
                            if let Some(ref_ranges) = ref_ranges_for_result.get(&result_id) {
                                for rr in ref_ranges {
                                    if let (Some(rrange), Some(ruri)) = (ranges.get(rr), range_doc.get(rr)) {
                                        let mut rpath = ruri.clone();
                                        if rpath.starts_with("file://") {
                                            rpath = rpath.trim_start_matches("file://").to_string();
                                            if cfg!(windows) && rpath.starts_with('/') && rpath.chars().nth(2) == Some(':') {
                                                rpath = rpath.trim_start_matches('/').to_string();
                                            }
                                        }
                                        let rfid = db.ensure_file(&rpath, "rust")?;
                                        // Use 1-based line/col
                                        let _oid = insert_occurrence(&mut db.conn, sid, rfid, "reference", (rrange.start_line as u32) + 1, (rrange.start_char as u32) + 1)?;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

fn extract_ident(s: &str) -> String {
    let mut start = None;
    let mut end = None;
    for (i, c) in s.char_indices() {
        if start.is_none() {
            if c.is_ascii_alphabetic() || c == '_' {
                start = Some(i);
            }
        } else {
            if !(c.is_ascii_alphanumeric() || c == '_') {
                end = Some(i);
                break;
            }
        }
    }
    if start.is_some() && end.is_none() { end = Some(s.len()); }
    if let Some(st) = start {
        if let Some(en) = end {
            return s[st..en].to_string();
        }
    }
    String::new()
}


/// Queries the call graph to find functions called by a given function,
///
/// Looks up a function by its USR (Unified Symbol Resolution) identifier
/// and prints the names of all functions it directly calls.
///
/// # Arguments
/// * `db_path` - Path to SQLite database with symbol graph
/// * `usr` - USR of the caller function (e.g., "c:@F@main#")
///
/// # Output
/// Prints one function name per line to stdout.
///
/// # Example USR formats
/// - `c:@F@main#` - global function main()
/// - `c:@S@MyClass@F@method#` - method of class MyClass
/// - `c:@N@ns@F@func#` - function in namespace ns
fn query_calls(db_path: &str, usr: &str) -> Result<()> {
    let db = Db::open(db_path)?;

    // Query edges where kind="call" and from_sym matches the USR
    let rows = db.query_edges_by_kind_from("call", usr)?;

    // Print each callee name
    for r in rows {
        println!("{r}");
    }
    Ok(())
}

/// Lists all modules and their import dependencies.
fn list_modules(db_path: &str) -> Result<()> {
    let db = Db::open(db_path)?;

    // Query all modules
    let mut stmt = db
        .conn
        .prepare("SELECT id, name, kind, path FROM modules ORDER BY name")?;
    let modules: Vec<(i64, String, String, String)> = stmt
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?
        .filter_map(|r| r.ok())
        .collect();

    if modules.is_empty() {
        println!("No modules found in database.");
        return Ok(());
    }

    println!("=== Modules ===");
    for (id, name, kind, path) in &modules {
        println!("{}: {} ({}) - {}", id, name, kind, path);
    }

    // Query module imports
    println!("\n=== Module Dependencies ===");
    let mut stmt = db.conn.prepare(
        "SELECT m1.name, m2.name FROM edges e
         JOIN modules m1 ON e.from_module = m1.id
         JOIN modules m2 ON e.to_module = m2.id
         WHERE e.kind = 'module-import'",
    )?;
    let imports: Vec<(String, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .filter_map(|r| r.ok())
        .collect();

    if imports.is_empty() {
        println!("No module imports found.");
    } else {
        for (from, to) in imports {
            println!("  {} -> {}", from, to);
        }
    }

    Ok(())
}

/// Shows database statistics.
fn show_stats(db_path: &str) -> Result<()> {
    let db = Db::open(db_path)?;

    let file_count: i64 = db
        .conn
        .query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0))?;
    let symbol_count: i64 = db
        .conn
        .query_row("SELECT COUNT(*) FROM symbols", [], |r| r.get(0))?;
    let occurrence_count: i64 = db
        .conn
        .query_row("SELECT COUNT(*) FROM occurrences", [], |r| r.get(0))?;
    let edge_count: i64 = db
        .conn
        .query_row("SELECT COUNT(*) FROM edges", [], |r| r.get(0))?;
    let module_count: i64 = db
        .conn
        .query_row("SELECT COUNT(*) FROM modules", [], |r| r.get(0))?;

    println!("=== Database Statistics ===");
    println!("Files:       {}", file_count);
    println!("Symbols:     {}", symbol_count);
    println!("Occurrences: {}", occurrence_count);
    println!("Edges:       {}", edge_count);
    println!("Modules:     {}", module_count);

    // Edge breakdown
    println!("\n=== Edge Types ===");
    let mut stmt = db
        .conn
        .prepare("SELECT kind, COUNT(*) FROM edges GROUP BY kind")?;
    let edge_types: Vec<(String, i64)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .filter_map(|r| r.ok())
        .collect();

    for (kind, count) in edge_types {
        println!("  {}: {}", kind, count);
    }

    // Symbol breakdown
    println!("\n=== Symbol Types ===");
    let mut stmt = db.conn.prepare(
        "SELECT kind, COUNT(*) FROM symbols GROUP BY kind ORDER BY COUNT(*) DESC LIMIT 10",
    )?;
    let symbol_types: Vec<(String, i64)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .filter_map(|r| r.ok())
        .collect();

    for (kind, count) in symbol_types {
        println!("  {}: {}", kind, count);
    }

    Ok(())
}

/// Analyze script projects (Python, JavaScript, TypeScript) with categorization.
fn scan_scripts(root: &str, db_path: &str) -> Result<()> {
    use symgraph_scripts::{ScriptAnalyzer, FileCategory};
    
    let mut analyzer = ScriptAnalyzer::new()?;
    let files = analyzer.analyze_project(root)?;
    
    let mut db = Db::open(db_path)?;
    let project_id = db.ensure_project("script_project", root)?;
    
    println!("Analyzing {} script files...", files.len());
    
    for file in &files {
        let category_str = match file.category {
            FileCategory::EntryPoint => "entry_point",
            FileCategory::UnitTest => "unit_test",
            FileCategory::IntegrationTest => "integration_test",
            FileCategory::CoreLogic => "core_logic",
            FileCategory::Utility => "utility",
            FileCategory::Configuration => "configuration",
            FileCategory::Documentation => "documentation",
            FileCategory::BuildScript => "build_script",
            FileCategory::Unknown => "unknown",
        };
        
        let file_id = db.ensure_file_with_category(
            project_id,
            &file.path,
            &file.language,
            Some(category_str),
            Some(&file.purpose)
        )?;
        
        // Insert functions as symbols
        for func in &file.functions {
            let usr = format!("{}:{}:{}", file.language, file.path, func);
            insert_symbol(&mut db.conn, file_id, Some(&usr), None, func, "function", true)?;
        }
        
        // Insert classes as symbols
        for class in &file.classes {
            let usr = format!("{}:{}:{}", file.language, file.path, class);
            insert_symbol(&mut db.conn, file_id, Some(&usr), None, class, "class", true)?;
        }
        
        println!("  {} [{}] - {} functions, {} classes", 
                file.path, category_str, file.functions.len(), file.classes.len());
    }
    
    println!("\nScript analysis complete. {} files processed.", files.len());
    Ok(())
}

/// Generate project annotation for compiled languages (C++/Rust).
fn annotate_compiled_project(root: &str, db_path: &str) -> Result<()> {
    use symgraph_core::annotations::{analyze_cpp_project, analyze_rust_project};
    
    let mut db = Db::open(db_path)?;
    
    // Get files from database with categories
    let files: Vec<(String, String, String)> = {
        let mut stmt = db.conn.prepare(
            "SELECT path, COALESCE(category, 'unknown'), COALESCE(purpose, '') FROM files"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?
        .filter_map(|r| r.ok())
        .collect();
        rows
    };
    
    if files.is_empty() {
        println!("No files found in database. Run scan-cxx or scan-rust first.");
        return Ok(());
    }
    
    // Detect language from file extensions
    let is_cpp = files.iter().any(|(path, _, _)| {
        path.ends_with(".cpp") || path.ends_with(".cc") || path.ends_with(".cxx") || path.ends_with(".h")
    });
    let is_rust = files.iter().any(|(path, _, _)| path.ends_with(".rs"));
    
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
        project_id,
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
/// Generate project annotation for script languages (Python, JS, TS).
fn annotate_project(root: &str, db_path: &str) -> Result<()> {
    use symgraph_scripts::{ScriptAnalyzer, project::ProjectAnalyzer};
    
    let mut analyzer = ScriptAnalyzer::new()?;
    let files = analyzer.analyze_project(root)?;
    
    let annotation = ProjectAnalyzer::analyze_project(root, &files)?;
    
    let mut db = Db::open(db_path)?;
    let project_id = db.ensure_project(&annotation.name, &annotation.root_path)?;
    
    let structure_json = serde_json::to_string(&annotation.structure)?;
    let dependencies_json = serde_json::to_string(&annotation.dependencies)?;
    let purpose_str = format!("{:?}", annotation.purpose);
    
    db.update_project_annotation(
        project_id,
        &annotation.description,
        &purpose_str,
        &structure_json,
        &dependencies_json
    )?;
    
    println!("=== Project Annotation ===");
    println!("Name: {}", annotation.name);
    println!("Purpose: {:?}", annotation.purpose);
    println!("Description: {}", annotation.description);
    println!("Architecture: {:?}", annotation.structure.architecture);
    println!("Entry Points: {:?}", annotation.entry_points);
    println!("Dependencies: {} external", annotation.dependencies.len());
    println!("Test Coverage: {:.1}%", annotation.test_coverage.coverage_estimate);
    
    if !annotation.structure.layers.is_empty() {
        println!("\n=== Architecture Layers ===");
        for layer in &annotation.structure.layers {
            println!("  {}: {} ({} files)", layer.name, layer.purpose, layer.files.len());
        }
    }
    
    if !annotation.structure.modules.is_empty() {
        println!("\n=== Modules ===");
        for module in &annotation.structure.modules {
            println!("  {}: {}", module.name, module.purpose);
        }
    }
    
    Ok(())
}