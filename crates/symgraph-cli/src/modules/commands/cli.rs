use clap::{Parser, Subcommand, ValueEnum};

/// Тип системы сборки для явного указания
#[derive(Debug, Clone, ValueEnum)]
pub enum BuildSystemType {
    /// Автоматическое определение
    Auto,
    /// CMake проект (CMakeLists.txt)
    CMake,
    /// Makefile проект
    Make,
    /// Visual Studio solution (.sln)
    Solution,
    /// Cargo проект (Rust)
    Cargo,
}

#[derive(Subcommand)]
pub enum Command {
    /// Generate compile_commands.json from a build system.
    ///
    /// Automatically detects the build system (CMake, Make, Visual Studio, Cargo)
    /// and generates compile_commands.json for use with clang-based tools.
    GenerateCompdb {
        /// Project root directory
        #[arg(short, long)]
        project: String,

        /// Output file path (default: compile_commands.json in project root)
        #[arg(short, long)]
        output: Option<String>,

        /// Build directory (for CMake)
        #[arg(short, long)]
        build_dir: Option<String>,

        /// Build system type (auto-detect if not specified)
        #[arg(short, long, value_enum)]
        build_system: Option<BuildSystemType>,

        /// CMake generator (Ninja, Makefiles, etc.)
        #[arg(short, long)]
        generator: Option<String>,

        /// Visual Studio configuration (Debug/Release)
        #[arg(short, long)]
        configuration: Option<String>,

        /// Visual Studio platform (x64/Win32)
        #[arg(short, long)]
        platform: Option<String>,
    },

    /// Scan C/C++ source code using compile_commands.json.
    ScanCxx {
        /// Path to compile_commands.json
        #[arg(short, long)]
        compdb: String,

        /// Database file path
        #[arg(short, long)]
        db: String,
    },

    /// Import C++20 module dependencies.
    ImportModules {
        /// Root directory containing module files
        #[arg(short, long)]
        root: String,

        /// Database file path
        #[arg(short, long)]
        db: String,
    },

    /// Scan C++20 modules directly from source.
    ScanModules {
        /// Root directory containing module files
        #[arg(short, long)]
        root: String,

        /// Database file path
        #[arg(short, long)]
        db: String,
    },

    /// Generate LSIF index from Rust project.
    GenerateLsif {
        /// Project directory
        #[arg(short, long)]
        project: String,

        /// Output LSIF file path
        #[arg(short, long)]
        output: String,
    },

    /// Scan Rust project using LSIF or cargo metadata.
    ScanRust {
        /// Path to Cargo.toml
        #[arg(short, long)]
        manifest: String,

        /// Optional LSIF file path
        #[arg(short, long)]
        lsif: Option<String>,

        /// Database file path
        #[arg(short, long)]
        db: String,
    },

    /// Query call graph for a symbol.
    QueryCalls {
        /// Database file path
        #[arg(short, long)]
        db: String,

        /// USR of the symbol
        #[arg(short, long)]
        usr: String,
    },

    /// List all modules in the database.
    ListModules {
        /// Database file path
        #[arg(short, long)]
        db: String,
    },

    /// Show database statistics.
    ShowStats {
        /// Database file path
        #[arg(short, long)]
        db: String,
    },

    /// Generate project annotation.
    AnnotateCompiled {
        /// Project root directory
        #[arg(short, long)]
        root: String,

        /// Database file path
        #[arg(short, long)]
        db: String,
    },

    /// Analyze script projects using SCIP.
    ScanScripts {
        /// Project root directory
        #[arg(short, long)]
        root: String,

        /// Database file path
        #[arg(short, long)]
        db: String,
    },

    /// Generate SCIP index from project.
    ScanScip {
        /// Project root directory
        #[arg(short, long)]
        root: String,

        /// Database file path
        #[arg(short, long)]
        db: String,
    },

    /// Start web viewer for database.
    WebViewer {
        /// Database file path
        #[arg(short, long)]
        db: String,
    },

    /// API endpoint for web viewer (internal use).
    Api {
        /// API endpoint (stats, files, symbols)
        endpoint: String,

        /// Database file path
        #[arg(short, long)]
        db: String,

        /// Search query (optional)
        #[arg(short, long)]
        search: Option<String>,
    },
}

/// Command line arguments
#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[command(subcommand)]
    pub cmd: Command,
}
