# symgraph-models

Shared types used by language-specific analyzers (C++, Rust) and the CLI.

Types:
- `ModuleInfo { name, path, imports }` — basic module metadata
- `Symbol { name, kind, signature, is_exported, line }` — exported symbol
- `Relation { from_name, to_name, kind }` — relation between symbols/types
- `ModuleAnalysis { info, symbols, relations }` — result of module analysis

Example:
```rust
use symgraph_models::ModuleInfo;

let mi = ModuleInfo { name: "m".to_string(), path: "src/m.cppm".to_string(), imports: vec!["std".to_string()] };
println!("module: {} at {}", mi.name, mi.path);
```
