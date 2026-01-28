//! # symgraph-cli
//!
//! Command-line tool for building semantic symbol graphs from C/C++ source code.
//!
//! ## Features
//! - Analyzes C/C++ code using libclang to extract symbols, references, and relationships
//! - Builds call graphs, inheritance hierarchies, and member relationships
//! - Imports C++20 module dependency graphs
//! - Generates compile_commands.json from CMake, Make, and Visual Studio projects
//! - Stores results in NoSQL database for querying
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
//! The NoSQL database contains:
//! - `symbols`: id, file_id, usr, name, kind, is_definition
//! - `occurrences`: id, symbol_id, file_id, usage_kind, line, column
//! - `edges`: id, from_sym, to_sym, from_module, to_module, kind
//! - `modules`: id, name, kind, path
//! - `files`: id, path, lang
//!
//! Edge kinds: "call", "inherit", "member", "module-import"

use anyhow::Result;
use clap::Parser;
use std::path::Path;

// Import modules
mod modules;

use modules::commands::{Args, Command};
use modules::cxx_analyzer::{scan_cxx, import_modules, scan_modules};
use modules::rust_analyzer::{scan_rust, generate_lsif_file};
use modules::utils::*;

fn main() -> Result<()> {
    let args = Args::parse();
    match args.cmd {
        Command::GenerateCompdb {
            project,
            output,
            build_dir,
            build_system,
            generator,
            configuration,
            platform,
        } => {
            generate_compdb(
                &project,
                output.as_deref(),
                build_dir.as_deref(),
                build_system,
                generator.as_deref(),
                configuration.as_deref(),
                platform.as_deref(),
            )?;
        }
        
        Command::ScanCxx { compdb, db } => {
            scan_cxx(&compdb, &db)?;
        }
        
        Command::ImportModules { root, db } => {
            import_modules(&root, &db)?;
        }
        
        Command::ScanModules { root, db } => {
            scan_modules(&root, &db)?;
        }
        
        Command::GenerateLsif { project, output } => {
            generate_lsif_file(Path::new(&project), Path::new(&output))?;
        }
        
        Command::ScanRust {
            manifest,
            lsif,
            db,
        } => {
            scan_rust(&manifest, lsif.as_deref(), &db)?;
        }
        
        Command::QueryCalls { db, usr } => {
            query_calls(&db, &usr)?;
        }
        
        Command::ListModules { db } => {
            list_modules(&db)?;
        }
        
        Command::ShowStats { db } => {
            show_stats(&db)?;
        }
        
        Command::AnnotateCompiled { root, db } => {
            annotate_compiled_project(&root, &db)?;
        }
        
        Command::ScanScripts { root, db } => {
            scan_scripts(&root, &db)?;
        }
        
        Command::ScanScip { root, db } => {
            scan_scip(&root, &db)?;
        }
        
        Command::WebViewer { db } => {
            start_web_viewer(&db)?;
        }
        
        Command::Api { endpoint, db, search } => {
            handle_api_request(&endpoint, &db, search.as_deref())?;
        }
    }

    Ok(())
}
