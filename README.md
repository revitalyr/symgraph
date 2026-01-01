
# symgraph

**Semantic symbol graph builder for C/C++ and Rust projects.**

Symgraph analyzes source code to build a comprehensive symbol graph that captures:
- **Symbols**: Functions, classes, variables, types with their definitions
- **Occurrences**: Where each symbol is used (calls, references, type references)
- **Relationships**: Call graphs, inheritance hierarchies, class members
- **C++20 Modules**: Module dependencies (`export module`/`import`)

Results are stored in **SQLite** for easy querying and integration.

## Features

- 🔍 **libclang-based analysis** - Accurate parsing using LLVM's libclang
- 📊 **Call graph extraction** - Track function calls across the codebase
- 🏗️ **Inheritance tracking** - Map class hierarchies and virtual methods
- 📦 **C++20 module support** - Parse module interface files (.cppm, .ixx, .mxx)
- 🛠️ **Build system integration** - Generate compile_commands.json from CMake, Make, Visual Studio
- 💾 **SQLite storage** - Query results with SQL, easy integration with other tools

## Installation

### Prerequisites

1. **Rust toolchain**:
   ```powershell
   winget install Rustlang.Rustup
   rustup default stable
   ```

2. **LLVM/Clang** (for libclang):
   ```powershell
   winget install LLVM.LLVM
   ```

3. **Build tools** (optional, for CMake projects):
   ```powershell
   winget install Ninja-build.Ninja
   ```

### Build from source

```powershell
git clone https://github.com/example/symgraph.git
cd symgraph
cargo build --release
```

## Quick Start

### 1. Generate compile_commands.json

Symgraph requires a `compile_commands.json` file to understand your project's compilation settings.

**Option A: CMake project (recommended)**
```powershell
# Auto-detect and generate
cargo run -p symgraph-cli -- generate-compdb --project /path/to/project

# Or with specific options
cargo run -p symgraph-cli -- generate-compdb --project . --generator Ninja --build-dir build
```

**Option B: Visual Studio project**
```powershell
cargo run -p symgraph-cli -- generate-compdb --project . --build-system solution --configuration Release --platform x64
```

**Option C: Makefile project**
```powershell
cargo run -p symgraph-cli -- generate-compdb --project . --build-system make
# Note: For better results, use `bear -- make` instead
```

**Option D: Manual CMake**
```powershell
cmake -S . -B build -G "Ninja" -DCMAKE_EXPORT_COMPILE_COMMANDS=ON
```

**Option E: Cargo (Rust) project**
```powershell
# Use rust-analyzer to export LSIF (supported by symgraph discovery):
rust-analyzer lsif . > compile_commands.json
# Or let the CLI attempt it automatically for Cargo projects:
cargo run -p symgraph-cli -- generate-compdb --project .
```

### 2. Analyze C/C++ code

```powershell
cargo run -p symgraph-cli -- scan-cxx --compdb build/compile_commands.json --db project.db
```

This extracts:
- All symbols (functions, classes, variables, etc.)
- Symbol occurrences (where they're used)
- Relationship edges (calls, inheritance, members)

### 3. Import C++20 modules (optional)

```powershell
cargo run -p symgraph-cli -- import-modules --root src/ --db project.db
```

Scans for `.cppm`, `.ixx`, `.mxx` files and builds the module dependency graph.

### 4. Query the symbol graph

```powershell
# List all functions called by main()
cargo run -p symgraph-cli -- query-calls --db project.db --usr "c:@F@main#"

# Query a class method
cargo run -p symgraph-cli -- query-calls --db project.db --usr "c:@S@MyClass@F@process#"
```

## Complete Workflow Example

```powershell
# Clone a project
git clone https://github.com/example/myproject.git
cd myproject

# Generate compile_commands.json
cargo run -p symgraph-cli -- generate-compdb --project . --build-dir build

# Analyze the code
cargo run -p symgraph-cli -- scan-cxx --compdb build/compile_commands.json --db myproject.db

# Import C++20 modules (if any)
cargo run -p symgraph-cli -- import-modules --root src/ --db myproject.db

# Query: what does main() call?
cargo run -p symgraph-cli -- query-calls --db myproject.db --usr "c:@F@main#"
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
-- Source files
CREATE TABLE files (
    id INTEGER PRIMARY KEY,
    path TEXT UNIQUE,
    lang TEXT  -- "c++", "rust", etc.
);

-- Symbols (functions, classes, variables)
CREATE TABLE symbols (
    id INTEGER PRIMARY KEY,
    file_id INTEGER,
    usr TEXT UNIQUE,     -- libclang USR
    module_id INTEGER,   -- for module-level symbols
    name TEXT,
    kind TEXT,           -- "function", "class", "variable", etc.
    is_definition INTEGER
);

-- Symbol occurrences (usages)
CREATE TABLE occurrences (
    id INTEGER PRIMARY KEY,
    symbol_id INTEGER,
    file_id INTEGER,
    usage_kind TEXT,     -- "call", "reference", "type_ref"
    line INTEGER,
    column INTEGER
);

-- Relationship edges
CREATE TABLE edges (
    id INTEGER PRIMARY KEY,
    from_sym INTEGER,    -- caller/parent
    to_sym INTEGER,      -- callee/child
    from_module INTEGER,
    to_module INTEGER,
    kind TEXT            -- "call", "inherit", "member", "module-import"
);

-- C++20 modules
CREATE TABLE modules (
    id INTEGER PRIMARY KEY,
    name TEXT UNIQUE,
    kind TEXT,           -- "cpp20-module"
    path TEXT
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

## Project Structure

```
symgraph/
├── Cargo.toml              # Workspace configuration
├── README.md
├── crates/
│   ├── symgraph-cli/       # Command-line interface
│   │   └── src/main.rs     # CLI entry point with all commands
│   ├── symgraph-core/      # SQLite database operations
│   │   └── src/lib.rs      # Db, insert_*, query_* functions
│   ├── symgraph-discovery/ # Build system detection & compile_commands.json
│   │   └── src/
│   │       ├── lib.rs      # load_compile_commands()
│   │       └── generate.rs # generate_from_cmake/make/vcxproj/sln
│   ├── symgraph-cxx/       # C++ analysis with libclang
│   │   └── src/
│   │       ├── lib.rs      # scan_tu() - AST traversal
│   │       └── modules.rs  # C++20 module parsing
│   └── symgraph-rust/      # Rust analysis (future)
└── examples/
    └── cpp20_modules/      # Example C++20 module project
        ├── CMakeLists.txt
        ├── foo.cppm
        └── main.cpp
```

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

### `import-modules`
Import C++20 module dependency graph.

```
USAGE:
    symgraph-cli import-modules --root <DIR> [--db <PATH>]

OPTIONS:
    --root <DIR>    Directory to scan for .cppm/.ixx/.mxx files
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

## Notes

- `compile_commands.json` is generated by **Ninja/Makefiles** generators (not by Visual Studio generators). Use Ninja for CMake analysis.
- The `clang` crate uses `clang_10_0` feature. If your libclang version differs, you may need to adjust.
- On Windows, run from **Developer PowerShell for VS 2022** for proper MSVC environment.

## License

MIT


