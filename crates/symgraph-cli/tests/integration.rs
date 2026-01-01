use assert_cmd::Command;
use std::fs;
use symgraph_core::Db;
use symgraph_rust::analyze_rust_module_from_text;
use tempfile::tempdir;

#[test]
fn cli_scan_modules_and_rust_integration() {
    // Setup temporary workspace
    let td = tempdir().expect("tempdir");
    let mods_dir = td.path().join("mods");
    fs::create_dir_all(&mods_dir).expect("create mods dir");

    // Create a simple C++20 module file
    let cppm = mods_dir.join("m.cppm");
    fs::write(&cppm, "export module M; export void f();").expect("write cppm");

    // DB path
    let db_path = td.path().join("test.db");
    let db_str = db_path.to_str().unwrap();

    // Run the CLI command: scan-modules --root <mods_dir> --db <db>
    let mut cmd = Command::cargo_bin("symgraph-cli").expect("binary");
    cmd.arg("scan-modules")
        .arg("--root")
        .arg(mods_dir.to_str().unwrap())
        .arg("--db")
        .arg(db_str)
        .assert()
        .success();

    // Verify DB contains module M
    let mut db = Db::open(db_str).expect("open db");
    let count: i64 = db
        .conn
        .query_row("SELECT COUNT(*) FROM modules WHERE name=?1", [&"M"], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(count, 1);

    // Now analyze a small Rust module and insert results into the DB to simulate integration
    let rust_src = "pub fn rfoo() {}";
    let analysis = analyze_rust_module_from_text(rust_src, "r.rs")
        .unwrap()
        .unwrap();

    // Insert into DB (mimic scan_modules behavior)
    let _mid = symgraph_core::upsert_module(
        &mut db.conn,
        &analysis.info.name,
        "rust-module",
        &analysis.info.path,
    )
    .unwrap();
    let fid = db.ensure_file(&analysis.info.path, "rust").unwrap();
    for sym in &analysis.symbols {
        let usr = format!("module:{}:{}", analysis.info.name, sym.name);
        let _ = symgraph_core::insert_symbol(
            &mut db.conn,
            fid,
            Some(&usr),
            None,
            &sym.name,
            &sym.kind,
            sym.is_exported,
        )
        .unwrap();
    }

    // Check the Rust symbol exists
    let count_sym: i64 = db
        .conn
        .query_row(
            "SELECT COUNT(*) FROM symbols WHERE name=?1",
            [&"rfoo"],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count_sym, 1);
}
