
use anyhow::Result;
use clap::{Parser, Subcommand};
use clang::{Clang, Index};
use symgraph_discovery::load_compile_commands;
use symgraph_cxx::scan_tu;
use symgraph_core::{Db, insert_symbol, insert_occurrence, insert_edge, upsert_module};

#[derive(Parser)]
#[command(version, about="symgraph CLI")]
struct Args {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Analyze C/C++ from compile_commands.json
    ScanCxx {
        #[arg(long)]
        compdb: String,
        #[arg(long, default_value="symgraph.db")]
        db: String,
    },
    /// Import C++20 module graph (simple scan)
    ImportModules {
        #[arg(long)]
        root: String,
        #[arg(long, default_value="symgraph.db")]
        db: String,
    },
    /// Query: list callees from a function USR
    QueryCalls {
        #[arg(long)]
        db: String,
        #[arg(long)]
        usr: String,
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    match args.cmd {
        Cmd::ScanCxx { compdb, db } => scan_cxx(&compdb, &db)?,
        Cmd::ImportModules { root, db } => import_modules(&root, &db)?,
        Cmd::QueryCalls { db, usr } => query_calls(&db, &usr)?,
    }
    Ok(())
}

fn scan_cxx(compdb: &str, db_path: &str) -> Result<()> {
    let clang = Clang::new().map_err(|e| anyhow::anyhow!("{}", e))?;
    let index = Index::new(&clang, false, true);
    let cmds = load_compile_commands(compdb)?;

    let mut db = Db::open(db_path)?;

    for cc in cmds {
        let args = if let Some(a) = cc.arguments { a } else if let Some(cmd) = cc.command { shell_words::split(&cmd)? } else { Vec::new() };
        let clean_args: Vec<String> = args.iter()
            .skip_while(|a| a.ends_with("cl") || a.ends_with("clang") || a.ends_with("clang++") || a.ends_with("gcc") || a.ends_with("g++"))
            .cloned().collect();
        let clean_args_refs: Vec<&str> = clean_args.iter().map(|s| s.as_str()).collect();
        let tu = match index.parser(&cc.file).arguments(&clean_args_refs).parse() { 
            Ok(tu) => tu, 
            Err(e) => { eprintln!("parse failed for {}: {:?}", cc.file, e); continue; } 
        };
        let (symbols, occs, edges) = scan_tu(&tu);

        for s in symbols {
            let fid = db.ensure_file(&s.file, "c++")?;
            let _sid = insert_symbol(&mut db.conn, fid, s.usr.as_deref(), None, &s.name, &s.kind, s.is_definition)?;
        }
        for o in occs {
            if let Some(usr) = o.usr.as_deref() {
                if let Some(sid) = db.find_symbol_by_usr(usr)? {
                    let fid = db.ensure_file(&o.file, "c++")?;
                    let _oid = insert_occurrence(&mut db.conn, sid, fid, &o.usage_kind, o.line, o.column)?;
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

fn import_modules(root: &str, db_path: &str) -> Result<()> {
    use walkdir::WalkDir;
    use symgraph_cxx::modules::scan_cpp20_module;
    let mut db = Db::open(db_path)?;
    for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path().display().to_string();
        if p.ends_with(".cppm") || p.ends_with(".ixx") || p.ends_with(".mxx") {
            if let Some(mi) = scan_cpp20_module(&p)? {
                let mid = upsert_module(&mut db.conn, &mi.name, "cpp20-module", &mi.path)?;
                let _fid = db.ensure_file(&mi.path, "c++")?;
                for imp in mi.imports {
                    let to = upsert_module(&mut db.conn, &imp, "cpp20-module", "")?;
                    let _eid = insert_edge(&mut db.conn, None, None, Some(mid), Some(to), "module-import")?;
                }
            }
        }
    }
    Ok(())
}

fn query_calls(db_path: &str, usr: &str) -> Result<()> {
    let db = Db::open(db_path)?;
    let rows = db.query_edges_by_kind_from("call", usr)?;
    for r in rows { println!("{r}"); }
    Ok(())
}
