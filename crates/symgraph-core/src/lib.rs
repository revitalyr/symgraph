use anyhow::Result;
use rusqlite::{params, Connection};

pub struct Db {
    pub conn: Connection,
}

impl Db {
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(include_str!("schema.sql"))?;
        Ok(Self { conn })
    }
    pub fn ensure_file(&mut self, path: &str, lang: &str) -> Result<i64> {
        self.conn.execute(
            "INSERT OR IGNORE INTO files(path, lang) VALUES (?1, ?2)",
            params![path, lang],
        )?;
        Ok(self
            .conn
            .query_row("SELECT id FROM files WHERE path=?1", params![path], |r| {
                r.get::<_, i64>(0)
            })?)
    }
    pub fn find_symbol_by_usr(&self, usr: &str) -> Result<Option<i64>> {
        let mut st = self.conn.prepare("SELECT id FROM symbols WHERE usr=?1")?;
        let mut rows = st.query(params![usr])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }
    pub fn query_edges_by_kind_from(&self, kind: &str, from_usr: &str) -> Result<Vec<String>> {
        let mut st = self.conn.prepare(
            "SELECT s2.name
             FROM edges e
             JOIN symbols s1 ON s1.id=e.from_sym
             JOIN symbols s2 ON s2.id=e.to_sym
            WHERE e.kind=?1 AND s1.usr=?2",
        )?;
        let rows = st.query_map(params![kind, from_usr], |r| Ok(r.get::<_, String>(0)?))?;
        Ok(rows.filter_map(|x| x.ok()).collect())
    }
}

pub fn insert_symbol(
    conn: &mut Connection,
    file_id: i64,
    usr: Option<&str>,
    key: Option<&str>,
    name: &str,
    kind: &str,
    is_def: bool,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO symbols(file_id, usr, key, name, kind, is_definition)
       VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![file_id, usr, key, name, kind, is_def as i32],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn insert_occurrence(
    conn: &mut Connection,
    sym_id: i64,
    file_id: i64,
    usage: &str,
    line: u32,
    col: u32,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO occurrences(symbol_id, file_id, usage_kind, line, column)
       VALUES (?1, ?2, ?3, ?4, ?5)",
        params![sym_id, file_id, usage, line, col],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn insert_edge(
    conn: &mut Connection,
    from_sym: Option<i64>,
    to_sym: Option<i64>,
    from_module: Option<i64>,
    to_module: Option<i64>,
    kind: &str,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO edges(from_sym, to_sym, from_module, to_module, kind)
       VALUES (?1, ?2, ?3, ?4, ?5)",
        params![from_sym, to_sym, from_module, to_module, kind],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn upsert_module(conn: &mut Connection, name: &str, kind: &str, path: &str) -> Result<i64> {
    conn.execute(
        "INSERT OR IGNORE INTO modules(name, kind, path) VALUES (?1, ?2, ?3)",
        params![name, kind, path],
    )?;
    Ok(
        conn.query_row("SELECT id FROM modules WHERE name=?1", params![name], |r| {
            r.get::<_, i64>(0)
        })?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Демонстрация: создание in-memory базы данных
    #[test]
    fn test_db_open_in_memory() {
        let db = Db::open(":memory:").expect("Failed to open in-memory database");
        // Проверяем, что таблицы созданы
        let count: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            count >= 5,
            "Should have at least 5 tables (modules, files, symbols, occurrences, edges)"
        );
    }

    /// Демонстрация: добавление файла в базу
    #[test]
    fn test_ensure_file() {
        let mut db = Db::open(":memory:").unwrap();

        let file_id1 = db.ensure_file("src/main.cpp", "c++").unwrap();
        let file_id2 = db.ensure_file("src/main.cpp", "c++").unwrap();
        let file_id3 = db.ensure_file("src/lib.rs", "rust").unwrap();

        // Повторный ensure_file возвращает тот же ID
        assert_eq!(file_id1, file_id2);
        // Разные файлы имеют разные ID
        assert_ne!(file_id1, file_id3);
    }

    /// Демонстрация: добавление символов (функции, классы)
    #[test]
    fn test_insert_symbol() {
        let mut db = Db::open(":memory:").unwrap();
        let file_id = db.ensure_file("example.cpp", "c++").unwrap();

        // Добавляем функцию
        let sym1 = insert_symbol(
            &mut db.conn,
            file_id,
            Some("c:@F@main#"),
            None,
            "main",
            "FunctionDecl",
            true,
        )
        .unwrap();

        // Добавляем класс
        let sym2 = insert_symbol(
            &mut db.conn,
            file_id,
            Some("c:@S@MyClass#"),
            None,
            "MyClass",
            "ClassDecl",
            true,
        )
        .unwrap();

        assert!(sym1 > 0);
        assert!(sym2 > 0);
        assert_ne!(sym1, sym2);
    }

    /// Демонстрация: поиск символа по USR
    #[test]
    fn test_find_symbol_by_usr() {
        let mut db = Db::open(":memory:").unwrap();
        let file_id = db.ensure_file("test.cpp", "c++").unwrap();

        let sym_id = insert_symbol(
            &mut db.conn,
            file_id,
            Some("c:@F@foo#"),
            None,
            "foo",
            "FunctionDecl",
            true,
        )
        .unwrap();

        // Найти существующий символ
        let found = db.find_symbol_by_usr("c:@F@foo#").unwrap();
        assert_eq!(found, Some(sym_id));

        // Не найти несуществующий символ
        let not_found = db.find_symbol_by_usr("c:@F@bar#").unwrap();
        assert_eq!(not_found, None);
    }

    /// Демонстрация: добавление occurrences (места использования)
    #[test]
    fn test_insert_occurrence() {
        let mut db = Db::open(":memory:").unwrap();
        let file_id = db.ensure_file("main.cpp", "c++").unwrap();

        let sym_id = insert_symbol(
            &mut db.conn,
            file_id,
            Some("c:@F@print#"),
            None,
            "print",
            "FunctionDecl",
            true,
        )
        .unwrap();

        // Добавляем несколько мест использования
        let occ1 = insert_occurrence(&mut db.conn, sym_id, file_id, "call", 10, 5).unwrap();
        let occ2 = insert_occurrence(&mut db.conn, sym_id, file_id, "call", 25, 8).unwrap();
        let occ3 = insert_occurrence(&mut db.conn, sym_id, file_id, "reference", 42, 12).unwrap();

        assert!(occ1 > 0);
        assert!(occ2 > 0);
        assert!(occ3 > 0);

        // Проверяем количество occurrences
        let count: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM occurrences WHERE symbol_id=?1",
                params![sym_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 3);
    }

    /// Демонстрация: создание графа вызовов (call graph)
    #[test]
    fn test_call_graph() {
        let mut db = Db::open(":memory:").unwrap();
        let file_id = db.ensure_file("app.cpp", "c++").unwrap();

        // Создаём функции
        let main_id = insert_symbol(
            &mut db.conn,
            file_id,
            Some("c:@F@main#"),
            None,
            "main",
            "FunctionDecl",
            true,
        )
        .unwrap();
        let foo_id = insert_symbol(
            &mut db.conn,
            file_id,
            Some("c:@F@foo#"),
            None,
            "foo",
            "FunctionDecl",
            true,
        )
        .unwrap();
        let bar_id = insert_symbol(
            &mut db.conn,
            file_id,
            Some("c:@F@bar#"),
            None,
            "bar",
            "FunctionDecl",
            true,
        )
        .unwrap();
        let baz_id = insert_symbol(
            &mut db.conn,
            file_id,
            Some("c:@F@baz#"),
            None,
            "baz",
            "FunctionDecl",
            true,
        )
        .unwrap();

        // main() вызывает foo() и bar()
        insert_edge(
            &mut db.conn,
            Some(main_id),
            Some(foo_id),
            None,
            None,
            "call",
        )
        .unwrap();
        insert_edge(
            &mut db.conn,
            Some(main_id),
            Some(bar_id),
            None,
            None,
            "call",
        )
        .unwrap();

        // foo() вызывает baz()
        insert_edge(&mut db.conn, Some(foo_id), Some(baz_id), None, None, "call").unwrap();

        // Запрос: кого вызывает main?
        let callees = db.query_edges_by_kind_from("call", "c:@F@main#").unwrap();
        assert_eq!(callees.len(), 2);
        assert!(callees.contains(&"foo".to_string()));
        assert!(callees.contains(&"bar".to_string()));

        // Запрос: кого вызывает foo?
        let foo_callees = db.query_edges_by_kind_from("call", "c:@F@foo#").unwrap();
        assert_eq!(foo_callees, vec!["baz".to_string()]);
    }

    /// Демонстрация: граф наследования классов
    #[test]
    fn test_inheritance_graph() {
        let mut db = Db::open(":memory:").unwrap();
        let file_id = db.ensure_file("classes.cpp", "c++").unwrap();

        // Создаём классы
        let base_id = insert_symbol(
            &mut db.conn,
            file_id,
            Some("c:@S@Base#"),
            None,
            "Base",
            "ClassDecl",
            true,
        )
        .unwrap();
        let derived_id = insert_symbol(
            &mut db.conn,
            file_id,
            Some("c:@S@Derived#"),
            None,
            "Derived",
            "ClassDecl",
            true,
        )
        .unwrap();
        let child_id = insert_symbol(
            &mut db.conn,
            file_id,
            Some("c:@S@Child#"),
            None,
            "Child",
            "ClassDecl",
            true,
        )
        .unwrap();

        // Derived наследует от Base
        insert_edge(
            &mut db.conn,
            Some(base_id),
            Some(derived_id),
            None,
            None,
            "inherit",
        )
        .unwrap();
        // Child наследует от Derived
        insert_edge(
            &mut db.conn,
            Some(derived_id),
            Some(child_id),
            None,
            None,
            "inherit",
        )
        .unwrap();

        // Проверяем: от кого наследует Base?
        let base_children = db
            .query_edges_by_kind_from("inherit", "c:@S@Base#")
            .unwrap();
        assert_eq!(base_children, vec!["Derived".to_string()]);
    }

    /// Демонстрация: граф членов класса
    #[test]
    fn test_member_graph() {
        let mut db = Db::open(":memory:").unwrap();
        let file_id = db.ensure_file("person.cpp", "c++").unwrap();

        // Класс Person
        let person_id = insert_symbol(
            &mut db.conn,
            file_id,
            Some("c:@S@Person#"),
            None,
            "Person",
            "ClassDecl",
            true,
        )
        .unwrap();

        // Поля и методы
        let name_id = insert_symbol(
            &mut db.conn,
            file_id,
            Some("c:@S@Person#name_"),
            None,
            "name_",
            "FieldDecl",
            true,
        )
        .unwrap();
        let age_id = insert_symbol(
            &mut db.conn,
            file_id,
            Some("c:@S@Person#age_"),
            None,
            "age_",
            "FieldDecl",
            true,
        )
        .unwrap();
        let get_name_id = insert_symbol(
            &mut db.conn,
            file_id,
            Some("c:@S@Person#getName#"),
            None,
            "getName",
            "Method",
            true,
        )
        .unwrap();

        // Связи членства
        insert_edge(
            &mut db.conn,
            Some(person_id),
            Some(name_id),
            None,
            None,
            "member",
        )
        .unwrap();
        insert_edge(
            &mut db.conn,
            Some(person_id),
            Some(age_id),
            None,
            None,
            "member",
        )
        .unwrap();
        insert_edge(
            &mut db.conn,
            Some(person_id),
            Some(get_name_id),
            None,
            None,
            "member",
        )
        .unwrap();

        // Запрос: какие члены у Person?
        let members = db
            .query_edges_by_kind_from("member", "c:@S@Person#")
            .unwrap();
        assert_eq!(members.len(), 3);
        assert!(members.contains(&"name_".to_string()));
        assert!(members.contains(&"age_".to_string()));
        assert!(members.contains(&"getName".to_string()));
    }

    /// Демонстрация: работа с модулями (C++20 modules)
    #[test]
    fn test_module_graph() {
        let mut db = Db::open(":memory:").unwrap();

        // Создаём модули
        let foo_mod = upsert_module(&mut db.conn, "foo", "cpp20-module", "src/foo.cppm").unwrap();
        let bar_mod = upsert_module(&mut db.conn, "bar", "cpp20-module", "src/bar.cppm").unwrap();
        let main_mod = upsert_module(&mut db.conn, "main", "cpp20-module", "src/main.cpp").unwrap();

        // main импортирует foo и bar
        insert_edge(
            &mut db.conn,
            None,
            None,
            Some(main_mod),
            Some(foo_mod),
            "module-import",
        )
        .unwrap();
        insert_edge(
            &mut db.conn,
            None,
            None,
            Some(main_mod),
            Some(bar_mod),
            "module-import",
        )
        .unwrap();

        // bar импортирует foo
        insert_edge(
            &mut db.conn,
            None,
            None,
            Some(bar_mod),
            Some(foo_mod),
            "module-import",
        )
        .unwrap();

        // Проверяем количество импортов
        let count: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM edges WHERE kind='module-import'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 3);
    }

    /// Демонстрация: upsert_module не создаёт дубликаты
    #[test]
    fn test_upsert_module_idempotent() {
        let mut db = Db::open(":memory:").unwrap();

        let id1 = upsert_module(
            &mut db.conn,
            "my_module",
            "cpp20-module",
            "src/my_module.cppm",
        )
        .unwrap();
        let id2 = upsert_module(
            &mut db.conn,
            "my_module",
            "cpp20-module",
            "src/my_module.cppm",
        )
        .unwrap();
        let id3 = upsert_module(
            &mut db.conn,
            "other_module",
            "cpp20-module",
            "src/other.cppm",
        )
        .unwrap();

        // Один и тот же модуль возвращает один ID
        assert_eq!(id1, id2);
        // Разные модули имеют разные ID
        assert_ne!(id1, id3);
    }
}
