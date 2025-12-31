
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
use clap::{Parser, Subcommand, ValueEnum};
use clang::{Clang, Index};
use symgraph_discovery::load_compile_commands;
use symgraph_cxx::scan_tu;
use symgraph_core::{Db, insert_symbol, insert_occurrence, insert_edge, upsert_module};
use std::path::Path;

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
}

#[derive(Subcommand)]
enum Cmd {
    /// Generate compile_commands.json from a build system.
    /// 
    /// Automatically detects the build system (CMake, Make, Visual Studio)
    /// and generates a compile_commands.json file for use with clang tools.
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
        /// Should contain CMakeLists.txt, Makefile, .vcxproj, or .sln
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
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    match args.cmd {
        Cmd::GenerateCompdb { 
            project, output, build_dir, generator, 
            build_system, configuration, platform 
        } => {
            generate_compdb(&project, output.as_deref(), build_dir.as_deref(), 
                           &generator, build_system, &configuration, &platform)?
        }
        Cmd::ScanCxx { compdb, db } => scan_cxx(&compdb, &db)?,
        Cmd::ImportModules { root, db } => import_modules(&root, &db)?,
        Cmd::QueryCalls { db, usr } => query_calls(&db, &usr)?,
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
        detect_build_system, BuildSystem,
        generate_from_cmake, generate_from_makefile,
        generate_from_vcxproj, generate_from_solution,
    };
    
    let project_path = Path::new(project);
    
    // Определяем систему сборки
    let detected_system = match build_system {
        BuildSystemType::Auto => detect_build_system(project_path),
        BuildSystemType::Cmake => BuildSystem::CMake,
        BuildSystemType::Make => BuildSystem::Make,
        BuildSystemType::Vcxproj => BuildSystem::VcxProj,
        BuildSystemType::Solution => BuildSystem::Solution,
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
        BuildSystem::Unknown => {
            anyhow::bail!(
                "Could not detect build system in '{}'. \n\
                 Supported: CMakeLists.txt, Makefile, .vcxproj, .sln\n\
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
    // Initialize libclang. This loads the libclang shared library.
    let clang = Clang::new().map_err(|e| anyhow::anyhow!("{}", e))?;
    
    // Create an index for parsing. Parameters:
    // - exclude_declarations_from_pch: false (include all declarations)
    // - display_diagnostics: true (show parsing errors/warnings)
    let index = Index::new(&clang, false, true);
    
    // Load compilation commands from JSON file generated by CMake/Ninja
    let cmds = load_compile_commands(compdb)?;

    // Open or create the SQLite database with schema
    let mut db = Db::open(db_path)?;

    // Process each compilation command (one per source file)
    for cc in cmds {
        // Extract compiler arguments from either 'arguments' array or 'command' string
        let args = if let Some(a) = cc.arguments { 
            a 
        } else if let Some(cmd) = cc.command { 
            // Parse command string into arguments (handles quotes, escapes)
            shell_words::split(&cmd)? 
        } else { 
            Vec::new() 
        };
        
        // Remove compiler executable from arguments (clang, gcc, cl, etc.)
        // libclang only needs the flags, not the compiler path
        let clean_args: Vec<String> = args.iter()
            .skip_while(|a| a.ends_with("cl") || a.ends_with("clang") || 
                           a.ends_with("clang++") || a.ends_with("gcc") || 
                           a.ends_with("g++"))
            .cloned().collect();
        
        // Convert to &str references for libclang API
        let clean_args_refs: Vec<&str> = clean_args.iter().map(|s| s.as_str()).collect();
        
        // Parse the translation unit (source file + headers)
        let tu = match index.parser(&cc.file).arguments(&clean_args_refs).parse() { 
            Ok(tu) => tu, 
            Err(e) => { 
                // Log parse failure but continue with other files
                eprintln!("parse failed for {}: {:?}", cc.file, e); 
                continue; 
            } 
        };
        
        // Extract symbols, occurrences, and relationship edges from the AST
        let (symbols, occs, edges) = scan_tu(&tu);

        // Store symbols (function declarations, class definitions, etc.)
        for s in symbols {
            // Ensure the file exists in DB, get its ID
            let fid = db.ensure_file(&s.file, "c++")?;
            // Insert symbol with USR (Unified Symbol Resolution) for unique identification
            let _sid = insert_symbol(&mut db.conn, fid, s.usr.as_deref(), None, 
                                     &s.name, &s.kind, s.is_definition)?;
        }
        
        // Store occurrences (references to symbols)
        for o in occs {
            if let Some(usr) = o.usr.as_deref() {
                // Find the symbol being referenced by its USR
                if let Some(sid) = db.find_symbol_by_usr(usr)? {
                    let fid = db.ensure_file(&o.file, "c++")?;
                    // Record where and how the symbol is used (call, reference, type_ref, etc.)
                    let _oid = insert_occurrence(&mut db.conn, sid, fid, 
                                                 &o.usage_kind, o.line, o.column)?;
                }
            }
        }
        
        // Store relationship edges (call graph, inheritance, member relationships)
        for (kind, from, to) in edges {
            // Look up both endpoints by USR
            let from_id = db.find_symbol_by_usr(&from)?;
            let to_id = db.find_symbol_by_usr(&to)?;
            // Only insert edge if both symbols are found
            if let (Some(fs), Some(ts)) = (from_id, to_id) {
                // kind can be: "call", "inherit", "member"
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
    use walkdir::WalkDir;
    use symgraph_cxx::modules::scan_cpp20_module;
    
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
                    let _eid = insert_edge(&mut db.conn, None, None, Some(mid), Some(to), 
                                          "module-import")?;
                }
            }
        }
    }
    Ok(())
}

/// Queries the call graph to find functions called by a given function.
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
