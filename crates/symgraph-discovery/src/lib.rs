//! # symgraph-discovery
//!
//! Модуль для обнаружения и загрузки информации о проектах C/C++.
//!
//! ## Возможности
//! - Загрузка compile_commands.json
//! - Генерация compile_commands.json из CMake, Make, Visual Studio проектов
//! - Автоматическое определение типа системы сборки

pub mod generate;

use anyhow::Result;
use serde::Deserialize;

// Реэкспорт основных типов и функций из модуля generate
pub use generate::{
    detect_build_system, generate_compile_commands, generate_from_cmake, generate_from_makefile,
    generate_from_solution, generate_from_vcxproj, generate_from_cargo, BuildSystem, CompileCommandEntry,
};

#[derive(Debug, Deserialize)]
pub struct CompileCommand {
    pub directory: String,
    pub file: String,
    pub command: Option<String>,
    pub arguments: Option<Vec<String>>,
}

pub fn load_compile_commands(path: &str) -> Result<Vec<CompileCommand>> {
    let f = std::fs::File::open(path)?;
    let cmds: Vec<CompileCommand> = serde_json::from_reader(f)?;
    Ok(cmds)
}

/// Парсинг compile_commands.json из строки (для тестирования)
pub fn parse_compile_commands(json: &str) -> Result<Vec<CompileCommand>> {
    let cmds: Vec<CompileCommand> = serde_json::from_str(json)?;
    Ok(cmds)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Демонстрация: парсинг compile_commands.json с полем "command"
    #[test]
    fn test_parse_with_command_field() {
        let json = r#"[
            {
                "directory": "/home/user/project/build",
                "file": "/home/user/project/src/main.cpp",
                "command": "clang++ -std=c++20 -I/usr/include -c main.cpp -o main.o"
            }
        ]"#;

        let cmds = parse_compile_commands(json).unwrap();
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].directory, "/home/user/project/build");
        assert_eq!(cmds[0].file, "/home/user/project/src/main.cpp");
        assert!(cmds[0].command.is_some());
        assert!(cmds[0].arguments.is_none());
    }

    /// Демонстрация: парсинг compile_commands.json с полем "arguments"
    #[test]
    fn test_parse_with_arguments_field() {
        let json = r#"[
            {
                "directory": "C:/projects/myapp/build",
                "file": "C:/projects/myapp/src/app.cpp",
                "arguments": ["clang++", "-std=c++20", "-Wall", "-c", "app.cpp", "-o", "app.o"]
            }
        ]"#;

        let cmds = parse_compile_commands(json).unwrap();
        assert_eq!(cmds.len(), 1);
        assert!(cmds[0].command.is_none());
        assert!(cmds[0].arguments.is_some());

        let args = cmds[0].arguments.as_ref().unwrap();
        assert!(args.contains(&"clang++".to_string()));
        assert!(args.contains(&"-std=c++20".to_string()));
        assert!(args.contains(&"-Wall".to_string()));
    }

    /// Демонстрация: парсинг нескольких файлов
    #[test]
    fn test_parse_multiple_files() {
        let json = r#"[
            {
                "directory": "/build",
                "file": "/src/main.cpp",
                "command": "clang++ -c main.cpp"
            },
            {
                "directory": "/build",
                "file": "/src/utils.cpp",
                "command": "clang++ -c utils.cpp"
            },
            {
                "directory": "/build",
                "file": "/src/math.cpp",
                "command": "clang++ -c math.cpp"
            }
        ]"#;

        let cmds = parse_compile_commands(json).unwrap();
        assert_eq!(cmds.len(), 3);

        let files: Vec<&str> = cmds.iter().map(|c| c.file.as_str()).collect();
        assert!(files.contains(&"/src/main.cpp"));
        assert!(files.contains(&"/src/utils.cpp"));
        assert!(files.contains(&"/src/math.cpp"));
    }

    /// Демонстрация: реальный формат от CMake/Ninja
    #[test]
    fn test_parse_cmake_ninja_format() {
        let json = r#"[
            {
                "directory": "C:/Users/dev/project/build",
                "command": "C:\\PROGRA~1\\LLVM\\bin\\clang++.exe  -IC:/Users/dev/project/include -std=c++20 -MD -MT CMakeFiles/app.dir/src/main.cpp.obj -MF CMakeFiles\\app.dir\\src\\main.cpp.obj.d -o CMakeFiles/app.dir/src/main.cpp.obj -c C:/Users/dev/project/src/main.cpp",
                "file": "C:/Users/dev/project/src/main.cpp"
            }
        ]"#;

        let cmds = parse_compile_commands(json).unwrap();
        assert_eq!(cmds.len(), 1);
        assert!(cmds[0].command.as_ref().unwrap().contains("-std=c++20"));
        assert!(cmds[0].file.ends_with("main.cpp"));
    }

    /// Демонстрация: пустой compile_commands.json
    #[test]
    fn test_parse_empty() {
        let json = "[]";
        let cmds = parse_compile_commands(json).unwrap();
        assert!(cmds.is_empty());
    }

    /// Демонстрация: обработка разных директорий
    #[test]
    fn test_different_directories() {
        let json = r#"[
            {
                "directory": "/build/debug",
                "file": "/src/a.cpp",
                "command": "clang++ -g -O0 -c a.cpp"
            },
            {
                "directory": "/build/release",
                "file": "/src/a.cpp",
                "command": "clang++ -O3 -DNDEBUG -c a.cpp"
            }
        ]"#;

        let cmds = parse_compile_commands(json).unwrap();
        assert_eq!(cmds.len(), 2);
        assert_eq!(cmds[0].directory, "/build/debug");
        assert_eq!(cmds[1].directory, "/build/release");
        // Один и тот же файл может компилироваться по-разному
        assert_eq!(cmds[0].file, cmds[1].file);
    }
}
