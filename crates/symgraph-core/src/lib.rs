pub mod annotations;
pub mod scip;
pub mod database;

// Re-export database types and functions for easier access
pub use database::{
    SymgraphDb, Project, Module, File, Symbol, Occurrence, Edge,
    insert_symbol, insert_occurrence, insert_edge, upsert_module
};

// Re-export SCIP functions for easier access
pub use scip::{parse_scip_file, parse_scip_bytes, load_scip_to_database};

// Legacy type alias for backward compatibility
pub type Db = SymgraphDb;

#[cfg(test)]
mod tests {
    use super::*;

    /// Демонстрация: создание in-memory базы данных
    #[test]
    fn test_db_open_in_memory() {
        let db = Db::open("test_db_1").expect("Failed to open database");
        // Проверяем, что база данных открыта
        drop(db);
        std::fs::remove_dir_all("test_db_1").ok();
    }

    /// Демонстрация: добавление файла в базу
    #[test]
    fn test_ensure_file() {
        let mut db = Db::open("test_db_2").unwrap();

        let file_id1 = db.ensure_file("src/main.cpp", "c++").unwrap();
        let file_id2 = db.ensure_file("src/main.cpp", "c++").unwrap();
        let file_id3 = db.ensure_file("src/lib.rs", "rust").unwrap();

        // Повторный ensure_file возвращает тот же ID
        assert_eq!(file_id1, file_id2);
        // Разные файлы имеют разные ID
        assert_ne!(file_id1, file_id3);
        
        drop(db);
        std::fs::remove_dir_all("test_db_2").ok();
    }

    /// Демонстрация: добавление символов (функции, классы)
    #[test]
    fn test_insert_symbol() {
        let mut db = Db::open("test_db_3").unwrap();
        let file_id = db.ensure_file("example.cpp", "c++").unwrap();

        // Добавляем функцию
        let sym1 = insert_symbol(
            &mut db,
            &file_id,
            Some("c:@F@main#"),
            None,
            "main",
            "FunctionDecl",
            true,
        )
        .unwrap();

        // Добавляем класс
        let sym2 = insert_symbol(
            &mut db,
            &file_id,
            Some("c:@S@MyClass#"),
            None,
            "MyClass",
            "ClassDecl",
            true,
        )
        .unwrap();

        assert!(!sym1.is_empty());
        assert!(!sym2.is_empty());
        assert_ne!(sym1, sym2);
        
        drop(db);
        std::fs::remove_dir_all("test_db_3").ok();
    }

    /// Демонстрация: поиск символа по USR
    #[test]
    fn test_find_symbol_by_usr() {
        let mut db = Db::open("test_db_4").unwrap();
        let file_id = db.ensure_file("test.cpp", "c++").unwrap();

        let sym_id = insert_symbol(
            &mut db,
            &file_id,
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
        
        drop(db);
        std::fs::remove_dir_all("test_db_4").ok();
    }

    /// Демонстрация: добавление occurrences (места использования)
    #[test]
    fn test_insert_occurrence() {
        let mut db = Db::open("test_db_5").unwrap();
        let file_id = db.ensure_file("main.cpp", "c++").unwrap();

        let sym_id = insert_symbol(
            &mut db,
            &file_id,
            Some("c:@F@print#"),
            None,
            "print",
            "FunctionDecl",
            true,
        )
        .unwrap();

        // Добавляем несколько мест использования
        let occ1 = insert_occurrence(&mut db, &sym_id, &file_id, "call", 10, 5).unwrap();
        let occ2 = insert_occurrence(&mut db, &sym_id, &file_id, "call", 25, 8).unwrap();
        let occ3 = insert_occurrence(&mut db, &sym_id, &file_id, "reference", 42, 12).unwrap();

        assert!(!occ1.is_empty());
        assert!(!occ2.is_empty());
        assert!(!occ3.is_empty());
        
        drop(db);
        std::fs::remove_dir_all("test_db_5").ok();
    }

    /// Демонстрация: создание графа вызовов (call graph)
    #[test]
    fn test_call_graph() {
        let mut db = Db::open("test_db_6").unwrap();
        let file_id = db.ensure_file("app.cpp", "c++").unwrap();

        // Создаём функции
        let main_id = insert_symbol(
            &mut db,
            &file_id,
            Some("c:@F@main#"),
            None,
            "main",
            "FunctionDecl",
            true,
        )
        .unwrap();
        let foo_id = insert_symbol(
            &mut db,
            &file_id,
            Some("c:@F@foo#"),
            None,
            "foo",
            "FunctionDecl",
            true,
        )
        .unwrap();
        let bar_id = insert_symbol(
            &mut db,
            &file_id,
            Some("c:@F@bar#"),
            None,
            "bar",
            "FunctionDecl",
            true,
        )
        .unwrap();
        let baz_id = insert_symbol(
            &mut db,
            &file_id,
            Some("c:@F@baz#"),
            None,
            "baz",
            "FunctionDecl",
            true,
        )
        .unwrap();

        // main() вызывает foo() и bar()
        insert_edge(
            &mut db,
            Some(&main_id),
            Some(&foo_id),
            None,
            None,
            "call",
        )
        .unwrap();
        insert_edge(
            &mut db,
            Some(&main_id),
            Some(&bar_id),
            None,
            None,
            "call",
        )
        .unwrap();

        // foo() вызывает baz()
        insert_edge(&mut db, Some(&foo_id), Some(&baz_id), None, None, "call").unwrap();

        // Запрос: кого вызывает main?
        let callees = db.query_edges_by_kind_from("call", "c:@F@main#").unwrap();
        assert_eq!(callees.len(), 2);
        assert!(callees.contains(&"foo".to_string()));
        assert!(callees.contains(&"bar".to_string()));

        // Запрос: кого вызывает foo?
        let foo_callees = db.query_edges_by_kind_from("call", "c:@F@foo#").unwrap();
        assert_eq!(foo_callees, vec!["baz".to_string()]);
        
        drop(db);
        std::fs::remove_dir_all("test_db_6").ok();
    }

    /// Демонстрация: граф наследования классов
    #[test]
    fn test_inheritance_graph() {
        let mut db = Db::open("test_db_7").unwrap();
        let file_id = db.ensure_file("classes.cpp", "c++").unwrap();

        // Создаём классы
        let base_id = insert_symbol(
            &mut db,
            &file_id,
            Some("c:@S@Base#"),
            None,
            "Base",
            "ClassDecl",
            true,
        )
        .unwrap();
        let derived_id = insert_symbol(
            &mut db,
            &file_id,
            Some("c:@S@Derived#"),
            None,
            "Derived",
            "ClassDecl",
            true,
        )
        .unwrap();
        let child_id = insert_symbol(
            &mut db,
            &file_id,
            Some("c:@S@Child#"),
            None,
            "Child",
            "ClassDecl",
            true,
        )
        .unwrap();

        // Derived наследует от Base
        insert_edge(
            &mut db,
            Some(&base_id),
            Some(&derived_id),
            None,
            None,
            "inherit",
        )
        .unwrap();
        // Child наследует от Derived
        insert_edge(
            &mut db,
            Some(&derived_id),
            Some(&child_id),
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
        
        drop(db);
        std::fs::remove_dir_all("test_db_7").ok();
    }

    /// Демонстрация: граф членов класса
    #[test]
    fn test_member_graph() {
        let mut db = Db::open("test_db_8").unwrap();
        let file_id = db.ensure_file("person.cpp", "c++").unwrap();

        // Класс Person
        let person_id = insert_symbol(
            &mut db,
            &file_id,
            Some("c:@S@Person#"),
            None,
            "Person",
            "ClassDecl",
            true,
        )
        .unwrap();

        // Поля и методы
        let name_id = insert_symbol(
            &mut db,
            &file_id,
            Some("c:@S@Person#name_"),
            None,
            "name_",
            "FieldDecl",
            true,
        )
        .unwrap();
        let age_id = insert_symbol(
            &mut db,
            &file_id,
            Some("c:@S@Person#age_"),
            None,
            "age_",
            "FieldDecl",
            true,
        )
        .unwrap();
        let get_name_id = insert_symbol(
            &mut db,
            &file_id,
            Some("c:@S@Person#getName#"),
            None,
            "getName",
            "Method",
            true,
        )
        .unwrap();

        // Связи членства
        insert_edge(
            &mut db,
            Some(&person_id),
            Some(&name_id),
            None,
            None,
            "member",
        )
        .unwrap();
        insert_edge(
            &mut db,
            Some(&person_id),
            Some(&age_id),
            None,
            None,
            "member",
        )
        .unwrap();
        insert_edge(
            &mut db,
            Some(&person_id),
            Some(&get_name_id),
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
        
        drop(db);
        std::fs::remove_dir_all("test_db_8").ok();
    }

    /// Демонстрация: работа с модулями (C++20 modules)
    #[test]
    fn test_module_graph() {
        let mut db = Db::open("test_db_9").unwrap();

        // Создаём модули
        let foo_mod = upsert_module(&mut db, "foo", "cpp20-module", "src/foo.cppm").unwrap();
        let bar_mod = upsert_module(&mut db, "bar", "cpp20-module", "src/bar.cppm").unwrap();
        let main_mod = upsert_module(&mut db, "main", "cpp20-module", "src/main.cpp").unwrap();

        // main импортирует foo и bar
        insert_edge(
            &mut db,
            None,
            None,
            Some(&main_mod),
            Some(&foo_mod),
            "module-import",
        )
        .unwrap();
        insert_edge(
            &mut db,
            None,
            None,
            Some(&main_mod),
            Some(&bar_mod),
            "module-import",
        )
        .unwrap();

        // bar импортирует foo
        insert_edge(
            &mut db,
            None,
            None,
            Some(&bar_mod),
            Some(&foo_mod),
            "module-import",
        )
        .unwrap();

        // Проверяем количество импортов
        let count = db.db.scan_prefix("edge:").count();
        assert_eq!(count, 3);
        
        drop(db);
        std::fs::remove_dir_all("test_db_9").ok();
    }

    /// Демонстрация: upsert_module не создаёт дубликаты
    #[test]
    fn test_upsert_module_idempotent() {
        let mut db = Db::open("test_db_10").unwrap();

        let id1 = upsert_module(
            &mut db,
            "my_module",
            "cpp20-module",
            "src/my_module.cppm",
        )
        .unwrap();
        let id2 = upsert_module(
            &mut db,
            "my_module",
            "cpp20-module",
            "src/my_module.cppm",
        )
        .unwrap();
        let id3 = upsert_module(
            &mut db,
            "other_module",
            "cpp20-module",
            "src/other.cppm",
        )
        .unwrap();

        // Один и тот же модуль возвращает один ID
        assert_eq!(id1, id2);
        // Разные модули имеют разные ID
        assert_ne!(id1, id3);
        
        drop(db);
        std::fs::remove_dir_all("test_db_10").ok();
    }
}
