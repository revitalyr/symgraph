
# symgraph

**Semantic symbol graph builder for C/C++, Rust, and script projects (Python, JavaScript, TypeScript).**

Symgraph analyzes source code to build a comprehensive symbol graph that captures:
- **Symbols**: Functions, classes, variables, types with their definitions
- **Occurrences**: Where each symbol is used (calls, references, type references)
- **Relationships**: Call graphs, inheritance hierarchies, class members
- **C++20 Modules**: Module dependencies (`export module`/`import`)
- **File Categories**: Entry points, tests, core logic, utilities with AI-assisted classification
- **Project Annotations**: Purpose, architecture, dependencies, structure analysis

Results are stored in **SQLite** for easy querying and integration.

## Features

- 🔍 **Multi-language analysis** - C/C++ (libclang), Rust (syn), Python/JS/TS (tree-sitter)
- 📊 **Call graph extraction** - Track function calls across the codebase
- 🏗️ **Inheritance tracking** - Map class hierarchies and virtual methods
- 📦 **C++20 module support** - Parse module interface files (.cppm, .ixx, .mxx)
- 🛠️ **Build system integration** - Generate compile_commands.json from CMake, Make, Visual Studio
- 🏷️ **Smart categorization** - Classify files as entry points, tests, core logic, utilities
- 🤖 **AI-assisted annotations** - Infer project purpose, architecture patterns, dependencies
- 💾 **SQLite storage** - Query results with SQL, easy integration with other tools
- 🖥️ **GUI Tools** - Multiple graphical interfaces for easy project analysis

## Table of Contents

- [Installation](#installation)
- [Quick Start](#quick-start)
- [GUI Tools](#gui-tools)
- [CLI Commands](#cli-commands-reference)
- [Database Schema](#database-schema)
- [Project Structure](#project-structure)
- [License](#license)

## Installation

### Prerequisites

1. **Rust toolchain**:
   ```powershell
   winget install Rustlang.Rustup
   rustup default stable
   ```

2. **LLVM/Clang** (for C++ analysis):
   ```powershell
   winget install LLVM.LLVM
   ```

3. **Build tools** (optional, for CMake projects):
   ```powershell
   winget install Ninja-build.Ninja
   ```

4. **Python 3.7+** (for GUI tools):
   ```powershell
   winget install Python.Python.3
   ```

### Build from Source

```powershell
git clone https://github.com/example/symgraph.git
cd symgraph
cargo build --release
```

## Quick Start

### Using the Enhanced GUI (Recommended)

The easiest way to analyze projects is using the Enhanced GUI with smart defaults:

```powershell
python gui/enhanced_gui.py
```

**Features:**
- 🎯 Automatic project type detection (C++, Rust, Python, JavaScript/TypeScript)
- ⚙️ Smart parameter auto-fill based on project type
- 📑 Tabbed interface for different workflow stages
- 💾 Save/load configuration presets
- 🎨 Visual project type indicators

**Quick Workflow:**
1. Click "Browse..." and select your project directory
2. Click "Detect" to identify project type
3. Review auto-filled parameters (or customize if needed)
4. Click "Start Analysis"
5. Open GUI Viewer to visualize results

### Using CLI

#### 1. Generate compile_commands.json

For C/C++ projects, generate compilation database:

```powershell
# Auto-detect and generate
cargo run -p symgraph-cli -- generate-compdb --project /path/to/project

# With specific options
cargo run -p symgraph-cli -- generate-compdb --project . --generator Ninja --build-dir build
```

#### 2. Analyze Code

**C/C++ Projects:**
```powershell
cargo run -p symgraph-cli -- scan-cxx --compdb build/compile_commands.json --db project.db
```

**Rust Projects:**
```powershell
cargo run -p symgraph-cli -- scan-rust --manifest-path Cargo.toml --db project.db
```

**Python/JavaScript/TypeScript Projects:**
```powershell
cargo run -p symgraph-cli -- scan-scripts --root . --db project.db
```

#### 3. Generate Annotations

```powershell
cargo run -p symgraph-cli -- annotate-project --root . --db project.db
```

#### 4. Query Results

```powershell
# List all functions called by main()
cargo run -p symgraph-cli -- query-calls --db project.db --usr "c:@F@main#"
```

## GUI Tools

### 1. Enhanced Symgraph GUI (`gui/enhanced_gui.py`)

**Recommended** - Modern interface with smart defaults and streamlined workflow.

```powershell
python enhanced_symgraph_gui.py
```

**Key Features:**
- **Smart Project Detection**: Automatically identifies C++, Rust, Python, JS/TS projects
- **Auto-Fill Parameters**: Smart defaults based on detected project type
- **Configuration Management**: Save/load project configurations
- **Tabbed Interface**: Separate tabs for Selection, Configuration, and Analysis
- **Real-time Logging**: Detailed analysis progress with status indicators

**Supported Project Types:**

| Type | Indicators | Extensions | Command |
|------|-----------|------------|---------|
| C++ 🔧 | CMakeLists.txt, Makefile, *.vcxproj, *.sln | .cpp, .cc, .cxx, .c, .h, .hpp, .hxx | scan-cxx |
| Rust 🦀 | Cargo.toml | .rs | scan-rust |
| Python 🐍 | requirements.txt, setup.py, pyproject.toml | .py | scan-scripts |
| JavaScript/TypeScript 📜 | package.json, tsconfig.json | .js, .ts, .jsx, .tsx, .mjs | scan-scripts |

**Usage Example:**

```
1. Browse → Select directory with CMakeLists.txt
2. Detect → Type "🔧 C++" detected
3. Parameters auto-set:
   - Build Directory: build
   - CMake Generator: Ninja
   - CompDB Path: auto
4. Start Analysis → Runs:
   - compile_commands.json generation
   - C++ code analysis
   - Annotation creation
```

**Configuration Files:**

Configurations are saved in JSON format:
```json
{
  "project_type": "C++",
  "project_path": "D:/projects/my_cpp_project",
  "db_name": "my_cpp_project.db",
  "auto_annotate": true,
  "params": {
    "build_dir": "build",
    "generator": "Ninja",
    "compdb_path": "auto"
  }
}
```

### 2. Project Explorer (`gui/project_explorer.py`)

Simple GUI for quick project discovery and analysis.

```powershell
python gui/project_explorer.py
```

**Features:**
- 🔍 Automatic detection of supported projects in a directory
- 📊 Display project statistics (file count, size)
- ⚡ Quick analysis of selected projects
- 🛠️ Generate compile_commands.json for C++ projects

### 3. Web Viewer (`gui/symgraph_viewer/flask_app/`)

Interactive web interface for symbol graph visualization.

```powershell
cd gui/symgraph_viewer/flask_app
pip install -r requirements.txt
python app.py
```

Then open http://localhost:5000

**Features:**
- 🕸️ Interactive D3.js graph visualization
- 🎨 Color coding by symbol categories
- 🔍 Detailed symbol information on click
- 🔗 Navigate through symbol relationships
- ⚙️ Customizable visualization settings
- 🖱️ Pan & Zoom controls

**Symbol Categories:**
- 🔴 **Tests** - test methods and classes
- 🟠 **Entry Points** - main, start, init functions
- 🟣 **Utilities** - helper, util, tool functions
- 🟦 **Configuration** - config, setting
- 🔵 **API** - controller, handler, api
- 🟢 **Data** - model, data, entity, struct
- 🟡 **Services** - service, manager, processor
- 🟠 **UI** - view, component, widget

## CLI Commands Reference

### `generate-compdb`
Generate `compile_commands.json` from various build systems.

```
USAGE:
    symgraph-cli generate-compdb [OPTIONS]

OPTIONS:
    --project <DIR>          Project directory [default: .]
    --output <PATH>          Output path for compile_commands.json
    --build-dir <DIR>        Build directory for CMake
    --generator <GEN>        CMake generator [default: Ninja]
    --build-system <TYPE>    auto, cmake, make, vcxproj, solution
    --configuration <CFG>    VS configuration [default: Debug]
    --platform <PLAT>        VS platform [default: x64]
```

### `scan-cxx`
Analyze C/C++ source files using libclang.

```
USAGE:
    symgraph-cli scan-cxx --compdb <PATH> [--db <PATH>]

OPTIONS:
    --compdb <PATH>    Path to compile_commands.json
    --db <PATH>        Output database [default: symgraph.db]
```

### `scan-rust`
Analyze Rust projects.

```
USAGE:
    symgraph-cli scan-rust --manifest-path <PATH> [--db <PATH>]

OPTIONS:
    --manifest-path <PATH>    Path to Cargo.toml
    --db <PATH>               Output database [default: symgraph.db]
```

### `scan-scripts`
Analyze script projects (Python, JavaScript, TypeScript).

```
USAGE:
    symgraph-cli scan-scripts --root <DIR> [--db <PATH>]

OPTIONS:
    --root <DIR>    Directory to scan for script files
    --db <PATH>     Output database [default: symgraph.db]
```

### `annotate-project`
Generate project annotation with AI-assisted analysis.

```
USAGE:
    symgraph-cli annotate-project --root <DIR> [--db <PATH>]

OPTIONS:
    --root <DIR>    Root directory of the project
    --db <PATH>     Output database [default: symgraph.db]
```

### `query-calls`
Query call graph from a function.

```
USAGE:
    symgraph-cli query-calls --db <PATH> --usr <USR>

OPTIONS:
    --db <PATH>     Path to database
    --usr <USR>     USR of the caller function
```

## USR (Unified Symbol Resolution) Format

USR is libclang's unique identifier for symbols:

| Pattern | Description | Example |
|---------|-------------|---------|
| `c:@F@name#` | Global function | `c:@F@main#` |
| `c:@S@ClassName#` | Class/struct | `c:@S@MyClass#` |
| `c:@S@Class@F@method#` | Class method | `c:@S@MyClass@F@process#` |
| `c:@N@ns@F@func#` | Namespaced function | `c:@N@utils@F@helper#` |
| `c:@F@func#I#` | Function with int param | `c:@F@add#I#I#` |

## Database Schema

The SQLite database contains these tables:

```sql
-- Projects
CREATE TABLE projects (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    root_path TEXT NOT NULL,
    description TEXT,
    purpose TEXT,
    structure TEXT,
    dependencies TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Source files with categorization
CREATE TABLE files (
    id INTEGER PRIMARY KEY,
    project_id INTEGER,
    path TEXT NOT NULL,
    lang TEXT NOT NULL,
    category TEXT,  -- "entry_point", "unit_test", "core_logic", etc.
    purpose TEXT,
    FOREIGN KEY(project_id) REFERENCES projects(id)
);

-- Symbols (functions, classes, variables)
CREATE TABLE symbols (
    id INTEGER PRIMARY KEY,
    file_id INTEGER,
    usr TEXT UNIQUE,     -- libclang USR or custom identifier
    name TEXT,
    kind TEXT,           -- "function", "class", "variable", etc.
    is_definition INTEGER,
    FOREIGN KEY(file_id) REFERENCES files(id)
);

-- Symbol occurrences (usages)
CREATE TABLE occurrences (
    id INTEGER PRIMARY KEY,
    symbol_id INTEGER,
    file_id INTEGER,
    usage_kind TEXT,     -- "call", "reference", "type_ref"
    line INTEGER,
    column INTEGER,
    FOREIGN KEY(symbol_id) REFERENCES symbols(id),
    FOREIGN KEY(file_id) REFERENCES files(id)
);

-- Relationship edges
CREATE TABLE edges (
    id INTEGER PRIMARY KEY,
    from_sym INTEGER,    -- caller/parent
    to_sym INTEGER,      -- callee/child
    from_module INTEGER,
    to_module INTEGER,
    kind TEXT,           -- "call", "inherit", "member", "module-import"
    FOREIGN KEY(from_sym) REFERENCES symbols(id),
    FOREIGN KEY(to_sym) REFERENCES symbols(id),
    FOREIGN KEY(from_module) REFERENCES modules(id),
    FOREIGN KEY(to_module) REFERENCES modules(id)
);

-- Modules (C++20, Rust, etc.)
CREATE TABLE modules (
    id INTEGER PRIMARY KEY,
    project_id INTEGER,
    name TEXT NOT NULL,
    kind TEXT,           -- "cpp20-module", "rust-crate"
    path TEXT,
    FOREIGN KEY(project_id) REFERENCES projects(id)
);
```

### Example Queries

```sql
-- Find all functions called by a specific function
SELECT s2.name FROM edges e
JOIN symbols s1 ON e.from_sym = s1.id
JOIN symbols s2 ON e.to_sym = s2.id
WHERE s1.usr = 'c:@F@main#' AND e.kind = 'call';

-- Find all classes that inherit from a base class
SELECT s1.name AS derived, s2.name AS base FROM edges e
JOIN symbols s1 ON e.from_sym = s1.id
JOIN symbols s2 ON e.to_sym = s2.id
WHERE e.kind = 'inherit';

-- Count occurrences per symbol
SELECT s.name, COUNT(o.id) as usage_count FROM symbols s
LEFT JOIN occurrences o ON s.id = o.symbol_id
GROUP BY s.id ORDER BY usage_count DESC;

-- Find module dependencies
SELECT m1.name AS importer, m2.name AS imported FROM edges e
JOIN modules m1 ON e.from_module = m1.id
JOIN modules m2 ON e.to_module = m2.id
WHERE e.kind = 'module-import';
```

## File Categories

### General Categories
- **entry_point** - Entry points (main.py, main.cpp, main.rs)
- **unit_test** - Unit tests
- **integration_test** - Integration tests
- **core_logic** - Core application logic
- **utility** - Utilities and helper functions
- **configuration** - Configuration files

### Language-Specific Categories
- **header** (C++) - Header files
- **implementation** (C++) - Implementation files
- **library** (Rust) - lib.rs
- **benchmark** (Rust) - Benchmarks
- **example** (Rust) - Usage examples
- **build** (Rust) - build.rs

## Project Structure

```
symgraph/
├── Cargo.toml                      # Workspace configuration
├── README.md
├── crates/
│   ├── symgraph-cli/               # Command-line interface
│   │   └── src/main.rs             # CLI entry point with all commands
│   ├── symgraph-core/              # SQLite database operations
│   │   └── src/lib.rs              # Db, insert_*, query_* functions
│   ├── symgraph-discovery/         # Build system detection & compile_commands.json
│   │   └── src/
│   │       ├── lib.rs              # load_compile_commands()
│   │       └── generate.rs         # generate_from_cmake/make/vcxproj/sln
│   ├── symgraph-cxx/               # C++ analysis with libclang
│   │   └── src/
│   │       ├── lib.rs              # scan_tu() - AST traversal
│   │       └── modules.rs          # C++20 module parsing
│   ├── symgraph-scripts/           # Script language analysis (Python, JS, TS)
│   │   └── src/
│   │       ├── lib.rs              # ScriptAnalyzer with tree-sitter
│   │       └── project.rs          # ProjectAnalyzer for annotations
│   ├── symgraph-rust/              # Rust analysis
│   └── symgraph-models/            # Data models
├── gui/
│   └── symgraph_viewer/
│       └── flask_app/              # Web visualization interface
│           ├── app.py              # Flask application
│           ├── templates/          # HTML templates
│           └── static/             # CSS, JavaScript, assets
├── gui/
│   ├── enhanced_gui.py             # Enhanced GUI with smart defaults
│   ├── project_explorer.py         # Simple project explorer GUI
│   └── symgraph_viewer/            # Web visualization interface
│       └── flask_app/
└── examples/
    └── cpp20_modules/              # Example C++20 module project
        ├── CMakeLists.txt
        ├── foo.cppm
        └── main.cpp
```

## Troubleshooting

### "Symgraph CLI not found"
1. Ensure symgraph is built: `cargo build --release`
2. Verify Cargo.toml contains all necessary crates
3. Check that Rust toolchain is properly installed

### "Database not found"
1. Run project analysis first
2. Verify analysis completed successfully
3. Database is created in the project directory or specified path

### "Flask app not starting"
1. Install Flask: `pip install flask`
2. Check path to flask_app directory
3. Ensure port 5000 is available

### "No supported project type detected" (GUI)
1. Ensure correct project directory is selected
2. Check for indicator files (CMakeLists.txt, Cargo.toml, package.json, etc.)
3. For C++ projects, ensure source files with .cpp, .h extensions exist

### "Command failed with code X"
1. Check log for error details
2. For C++ projects, verify LLVM/Clang is installed
3. Verify parameter paths are correct
4. Ensure compile_commands.json is valid (for C++ projects)

## Notes

- `compile_commands.json` is generated by **Ninja/Makefiles** generators (not by Visual Studio generators). Use Ninja for CMake analysis.
- The `clang` crate uses `clang_10_0` feature. If your libclang version differs, you may need to adjust.
- On Windows, run from **Developer PowerShell for VS 2022** for proper MSVC environment.

## License

MIT
