# LSIF Usage Guide

## What is LSIF?

LSIF (Language Server Index Format) is a standard format for code intelligence data. Rust Analyzer can generate LSIF files that contain detailed information about your code structure, definitions, references, and more.

## Why use LSIF with Symgraph?

1. **More accurate analysis** - Rust Analyzer has complete semantic understanding
2. **Better symbol resolution** - Handles complex Rust features correctly
3. **Faster processing** - Pre-analyzed data vs source code parsing
4. **Enhanced relationships** - More precise call graphs and dependencies

## Installation

First, ensure you have Rust Analyzer installed:

```bash
rustup component add rust-analyzer
# or
cargo install rust-analyzer
```

## Usage Methods

### Method 1: Automatic Generation (Recommended)

Symgraph can generate LSIF files automatically:

```bash
# Basic usage - Symgraph will generate LSIF if needed
symgraph-cli scan-rust --manifest-path ./Cargo.toml --db project.db

# Specify custom LSIF file path
symgraph-cli scan-rust --manifest-path ./Cargo.toml --lsif project.lsif --db project.db
```

### Method 2: Manual Generation

Generate LSIF file manually, then use it:

```bash
# Generate LSIF file
rust-analyzer lsif . > project.lsif

# Use with Symgraph
symgraph-cli scan-rust --manifest-path ./Cargo.toml --lsif project.lsif --db project.db
```

### Method 3: Environment Override

Override the rust-analyzer command if needed:

```bash
export SYGRAPH_RUST_ANALYZER_CMD="/path/to/custom/rust-analyzer"
symgraph-cli scan-rust --manifest-path ./Cargo.toml --db project.db
```

## Workflow Examples

### Example 1: Complete Workspace Analysis

```bash
# Analyze entire workspace with LSIF
cd /path/to/rust/workspace
symgraph-cli scan-rust --manifest-path ./Cargo.toml --lsif workspace.lsif --db workspace.db

# View results
cd /path/to/symgraph
python gui/run_gui.py
```

### Example 2: Large Project Optimization

```bash
# For large projects, generate LSIF once
rust-analyzer lsif . > large-project.lsif

# Use the LSIF file for multiple analyses
symgraph-cli scan-rust --lsif large-project.lsif --db analysis1.db
symgraph-cli scan-rust --lsif large-project.lsif --db analysis2.db
```

### Example 3: CI/CD Integration

```bash
# In CI pipeline
rust-analyzer lsif . > project.lsif
symgraph-cli scan-rust --lsif project.lsif --db project.db
# Upload project.db for analysis
```

## LSIF vs Source Parsing

| Feature | LSIF | Source Parsing |
|---------|------|----------------|
| **Accuracy** | ✅ High (semantic) | ⚠️ Medium (syntactic) |
| **Speed** | ✅ Fast (pre-analyzed) | ⚠️ Slower (parse on demand) |
| **Complex Features** | ✅ Handles all Rust features | ⚠️ Limited for complex cases |
| **Dependencies** | Requires rust-analyzer | Self-contained |
| **Setup** | One-time generation | No setup needed |

## Troubleshooting

### Common Issues

1. **"rust-analyzer not found"**
   ```bash
   rustup component add rust-analyzer
   ```

2. **"LSIF generation failed"**
   ```bash
   # Check if project compiles
   cargo check
   
   # Try manual generation
   rust-analyzer lsif . > test.lsif
   ```

3. **"Large LSIF files"**
   ```bash
   # LSIF files can be large for big projects
   # Consider using .gitignore
   echo "*.lsif" >> .gitignore
   ```

### Debug Mode

Enable verbose output to see LSIF processing:

```bash
RUST_LOG=debug symgraph-cli scan-rust --manifest-path ./Cargo.toml --db project.db
```

## Best Practices

1. **Use LSIF for production** - More reliable analysis
2. **Cache LSIF files** - Regenerate only when code changes
3. **Combine approaches** - LSIF + source parsing for completeness
4. **Version control** - Add *.lsif to .gitignore, regenerate as needed

## Integration with GUI

When using the unified GUI, LSIF integration is automatic:

1. Select your Rust project directory
2. Choose database location
3. Click "Index Project"
4. Symgraph will automatically generate and use LSIF if beneficial

The GUI will show LSIF processing status in the output log.
