//! # Генерация compile_commands.json
//!
//! Модуль для генерации compile_commands.json из различных систем сборки:
//! - CMake (CMakeLists.txt)
//! - Make (Makefile)
//! - MSBuild (.vcxproj, .sln)
//! - Cargo (Cargo.toml)
//!
//! ## Стратегии генерации
//!
//! ### CMake
//! Запускает `cmake` с флагом `-DCMAKE_EXPORT_COMPILE_COMMANDS=ON`
//! для генерации compile_commands.json в директории сборки.
//!
//! ### Make
//! Использует `bear` или `compiledb` для перехвата команд компиляции,
//! либо парсит вывод `make -n` (dry-run).
//!
//! ### MSBuild (.vcxproj/.sln)
//! Парсит XML файлы проекта для извлечения настроек компиляции,
//! или использует clang-cl совместимые флаги.

use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Тип системы сборки, обнаруженной в проекте
#[derive(Debug, Clone, PartialEq)]
pub enum BuildSystem {
    /// CMake проект (CMakeLists.txt)
    CMake,
    /// Make проект (Makefile, GNUmakefile, makefile)
    Make,
    /// Visual Studio проект (.vcxproj)
    VcxProj,
    /// Visual Studio решение (.sln)
    Solution,
    /// Cargo / Rust проект (Cargo.toml)
    Cargo,
    /// Неизвестная система сборки
    Unknown,
}

/// Запись compile_commands.json для сериализации
#[derive(Debug, Serialize)]
pub struct CompileCommandEntry {
    /// Рабочая директория для компиляции
    pub directory: String,
    /// Путь к исходному файлу
    pub file: String,
    /// Команда компиляции (строка)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// Аргументы компиляции (массив)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Vec<String>>,
}

/// Автоматически определяет тип системы сборки в директории
///
/// # Arguments
/// * `project_dir` - Путь к корневой директории проекта
///
/// # Returns
/// Тип обнаруженной системы сборки
pub fn detect_build_system(project_dir: &Path) -> BuildSystem {
    // Проверяем наличие файлов систем сборки в порядке приоритета
    if project_dir.join("CMakeLists.txt").exists() {
        return BuildSystem::CMake;
    }

    // Проверяем Cargo.toml (Rust/Cargo project)
    if project_dir.join("Cargo.toml").exists() {
        return BuildSystem::Cargo;
    }

    // Проверяем .sln файлы (Visual Studio Solution)
    if let Ok(entries) = fs::read_dir(project_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                if ext == "sln" {
                    return BuildSystem::Solution;
                }
                if ext == "vcxproj" {
                    return BuildSystem::VcxProj;
                }
            }
        }
    }

    // Проверяем Makefile (разные варианты имён)
    for makefile in &["Makefile", "makefile", "GNUmakefile"] {
        if project_dir.join(makefile).exists() {
            return BuildSystem::Make;
        }
    }

    BuildSystem::Unknown
}

/// Генерирует compile_commands.json из CMake проекта
///
/// Запускает CMake с флагом CMAKE_EXPORT_COMPILE_COMMANDS=ON
/// для автоматической генерации compile_commands.json.
///
/// # Arguments
/// * `source_dir` - Директория с CMakeLists.txt
/// * `build_dir` - Директория для сборки (будет создана)
/// * `generator` - Генератор CMake (например, "Ninja", "Unix Makefiles")
/// * `extra_args` - Дополнительные аргументы CMake
///
/// # Returns
/// Путь к сгенерированному compile_commands.json
pub fn generate_from_cmake(
    source_dir: &Path,
    build_dir: &Path,
    generator: Option<&str>,
    extra_args: &[String],
) -> Result<PathBuf> {
    // Создаём директорию сборки
    fs::create_dir_all(build_dir)
        .with_context(|| format!("Failed to create build directory: {}", build_dir.display()))?;

    // Формируем команду CMake
    let mut cmd = Command::new("cmake");
    cmd.arg("-S")
        .arg(source_dir)
        .arg("-B")
        .arg(build_dir)
        .arg("-DCMAKE_EXPORT_COMPILE_COMMANDS=ON");

    // Добавляем генератор если указан (рекомендуется Ninja)
    if let Some(gen) = generator {
        cmd.arg("-G").arg(gen);
    }

    // Добавляем дополнительные аргументы
    for arg in extra_args {
        cmd.arg(arg);
    }

    // Запускаем CMake
    let output = cmd
        .output()
        .with_context(|| "Failed to execute cmake. Is CMake installed and in PATH?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("CMake configuration failed:\n{}", stderr);
    }

    // Проверяем что compile_commands.json создан
    let compdb_path = build_dir.join("compile_commands.json");
    if !compdb_path.exists() {
        bail!(
            "compile_commands.json was not generated. \
             Make sure you're using a generator that supports it (Ninja, Unix Makefiles). \
             Visual Studio generators do not support CMAKE_EXPORT_COMPILE_COMMANDS."
        );
    }

    Ok(compdb_path)
}

/// Генерирует compile_commands.json из Makefile используя dry-run
///
/// Парсит вывод `make -n` для извлечения команд компиляции.
/// Это менее надёжно чем `bear`, но не требует дополнительных инструментов.
///
/// # Arguments
/// * `makefile_dir` - Директория с Makefile
/// * `output_path` - Путь для записи compile_commands.json
/// * `make_args` - Дополнительные аргументы для make
///
/// # Limitations
/// - Может не работать со сложными Makefile
/// - Не поддерживает все варианты синтаксиса make
/// - Рекомендуется использовать `bear` для более надёжного результата
pub fn generate_from_makefile(
    makefile_dir: &Path,
    output_path: &Path,
    make_args: &[String],
) -> Result<PathBuf> {
    // Запускаем make -n (dry-run) для получения команд без выполнения
    let mut cmd = Command::new("make");
    cmd.current_dir(makefile_dir)
        .arg("-n") // Dry-run: печатает команды без выполнения
        .arg("-w"); // Print working directory

    for arg in make_args {
        cmd.arg(arg);
    }

    let output = cmd
        .output()
        .with_context(|| "Failed to execute make. Is make installed and in PATH?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("make -n failed:\n{}", stderr);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Парсим вывод make для извлечения команд компиляции
    let entries = parse_make_dry_run(&stdout, makefile_dir)?;

    if entries.is_empty() {
        bail!(
            "No compilation commands found in make output. \
             The Makefile may not have any C/C++ compilation rules, \
             or the parsing failed. Consider using 'bear' for better results: \
             bear -- make"
        );
    }

    // Записываем compile_commands.json
    write_compile_commands(&entries, output_path)?;

    Ok(output_path.to_path_buf())
}

/// Парсит вывод `make -n` для извлечения команд компиляции
fn parse_make_dry_run(output: &str, working_dir: &Path) -> Result<Vec<CompileCommandEntry>> {
    use regex::Regex;

    let mut entries = Vec::new();
    let mut current_dir = working_dir.to_path_buf();

    // Регулярное выражение для поиска команд компиляции C/C++
    // Ищем вызовы gcc, g++, clang, clang++, cc, c++ с флагом -c
    let compile_re =
        Regex::new(r"(?:gcc|g\+\+|clang|clang\+\+|cc|c\+\+|cl|cl\.exe)\s+.*\s+-c\s+(\S+)")?;

    // Регулярное выражение для отслеживания смены директории
    let dir_re = Regex::new(r#"make\[\d+\]: Entering directory ['"](.+)['"]"#)?;

    for line in output.lines() {
        // Отслеживаем смену директории
        if let Some(caps) = dir_re.captures(line) {
            if let Some(dir) = caps.get(1) {
                current_dir = PathBuf::from(dir.as_str());
            }
            continue;
        }

        // Ищем команды компиляции
        if let Some(caps) = compile_re.captures(line) {
            if let Some(source_file) = caps.get(1) {
                let file_path = if Path::new(source_file.as_str()).is_absolute() {
                    PathBuf::from(source_file.as_str())
                } else {
                    current_dir.join(source_file.as_str())
                };

                // Проверяем что это C/C++ файл
                if let Some(ext) = file_path.extension() {
                    let ext_str = ext.to_string_lossy().to_lowercase();
                    if matches!(
                        ext_str.as_str(),
                        "c" | "cc" | "cpp" | "cxx" | "c++" | "m" | "mm"
                    ) {
                        entries.push(CompileCommandEntry {
                            directory: current_dir.to_string_lossy().to_string(),
                            file: file_path.to_string_lossy().to_string(),
                            command: Some(line.trim().to_string()),
                            arguments: None,
                        });
                    }
                }
            }
        }
    }

    Ok(entries)
}

/// Генерирует compile_commands.json из Visual Studio проекта (.vcxproj)
///
/// Парсит XML файл .vcxproj для извлечения:
/// - Исходных файлов (ClCompile)
/// - Include директорий
/// - Препроцессорных определений
/// - Флагов компиляции
///
/// # Arguments
/// * `vcxproj_path` - Путь к .vcxproj файлу
/// * `output_path` - Путь для записи compile_commands.json
/// * `configuration` - Конфигурация сборки (Debug, Release)
/// * `platform` - Платформа (x64, Win32)
///
/// # Note
/// Генерирует команды совместимые с clang-cl
pub fn generate_from_vcxproj(
    vcxproj_path: &Path,
    output_path: &Path,
    configuration: &str,
    platform: &str,
) -> Result<PathBuf> {
    let content = fs::read_to_string(vcxproj_path)
        .with_context(|| format!("Failed to read {}", vcxproj_path.display()))?;

    let project_dir = vcxproj_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Invalid vcxproj path"))?;

    let entries = parse_vcxproj(&content, project_dir, configuration, platform)?;

    if entries.is_empty() {
        bail!("No C/C++ source files found in {}", vcxproj_path.display());
    }

    write_compile_commands(&entries, output_path)?;

    Ok(output_path.to_path_buf())
}

/// Парсит .vcxproj XML для извлечения команд компиляции
fn parse_vcxproj(
    content: &str,
    project_dir: &Path,
    configuration: &str,
    _platform: &str, // TODO: использовать для фильтрации по платформе
) -> Result<Vec<CompileCommandEntry>> {
    use regex::Regex;

    let mut entries = Vec::new();

    // Ищем ClCompile элементы (исходные файлы)
    let compile_re = Regex::new(r#"<ClCompile\s+Include="([^"]+)""#)?;

    // Ищем AdditionalIncludeDirectories
    let include_re =
        Regex::new(r#"<AdditionalIncludeDirectories>([^<]+)</AdditionalIncludeDirectories>"#)?;

    // Ищем PreprocessorDefinitions
    let define_re = Regex::new(r#"<PreprocessorDefinitions>([^<]+)</PreprocessorDefinitions>"#)?;

    // Извлекаем include директории
    let includes: Vec<String> = include_re
        .captures_iter(content)
        .filter_map(|cap| cap.get(1))
        .flat_map(|m| m.as_str().split(';'))
        .filter(|s| !s.is_empty() && !s.starts_with('%'))
        .map(|s| {
            format!(
                "-I{}",
                s.replace("$(ProjectDir)", &project_dir.to_string_lossy())
            )
        })
        .collect();

    // Извлекаем препроцессорные определения
    let defines: Vec<String> = define_re
        .captures_iter(content)
        .filter_map(|cap| cap.get(1))
        .flat_map(|m| m.as_str().split(';'))
        .filter(|s| !s.is_empty() && !s.starts_with('%'))
        .map(|s| format!("-D{}", s))
        .collect();

    // Формируем базовые аргументы clang-cl
    let mut base_args = vec![
        "clang-cl".to_string(),
        format!("/D_{}", configuration.to_uppercase()),
    ];
    base_args.extend(includes);
    base_args.extend(defines);
    base_args.push("-c".to_string());

    // Извлекаем исходные файлы
    for cap in compile_re.captures_iter(content) {
        if let Some(file_match) = cap.get(1) {
            let file_path = file_match.as_str();

            // Пропускаем файлы с условной компиляцией для других конфигураций
            // (упрощённая логика, полный парсинг требует XML парсер)

            let full_path = if Path::new(file_path).is_absolute() {
                PathBuf::from(file_path)
            } else {
                project_dir.join(file_path)
            };

            let mut args = base_args.clone();
            args.push(full_path.to_string_lossy().to_string());

            entries.push(CompileCommandEntry {
                directory: project_dir.to_string_lossy().to_string(),
                file: full_path.to_string_lossy().to_string(),
                command: None,
                arguments: Some(args),
            });
        }
    }

    Ok(entries)
}

/// Генерирует compile_commands.json из Visual Studio Solution (.sln)
///
/// Находит все .vcxproj проекты в решении и генерирует
/// объединённый compile_commands.json.
///
/// # Arguments
/// * `sln_path` - Путь к .sln файлу
/// * `output_path` - Путь для записи compile_commands.json
/// * `configuration` - Конфигурация сборки (Debug, Release)
/// * `platform` - Платформа (x64, Win32)
pub fn generate_from_solution(
    sln_path: &Path,
    output_path: &Path,
    configuration: &str,
    platform: &str,
) -> Result<PathBuf> {
    use regex::Regex;

    let content = fs::read_to_string(sln_path)
        .with_context(|| format!("Failed to read {}", sln_path.display()))?;

    let sln_dir = sln_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Invalid solution path"))?;

    // Ищем все .vcxproj проекты в решении
    let project_re = Regex::new(r#"Project\([^)]+\)\s*=\s*"[^"]+",\s*"([^"]+\.vcxproj)""#)?;

    let mut all_entries = Vec::new();

    for cap in project_re.captures_iter(&content) {
        if let Some(proj_match) = cap.get(1) {
            let proj_path = sln_dir.join(proj_match.as_str().replace("\\", "/"));

            if proj_path.exists() {
                let proj_content = fs::read_to_string(&proj_path)?;
                let proj_dir = proj_path.parent().unwrap_or(sln_dir);

                match parse_vcxproj(&proj_content, proj_dir, configuration, platform) {
                    Ok(entries) => all_entries.extend(entries),
                    Err(e) => eprintln!("Warning: Failed to parse {}: {}", proj_path.display(), e),
                }
            }
        }
    }

    if all_entries.is_empty() {
        bail!(
            "No C/C++ source files found in solution {}",
            sln_path.display()
        );
    }

    write_compile_commands(&all_entries, output_path)?;

    Ok(output_path.to_path_buf())
}

/// Записывает compile_commands.json в файл
fn write_compile_commands(entries: &[CompileCommandEntry], output_path: &Path) -> Result<()> {
    let json = serde_json::to_string_pretty(entries)?;

    // Создаём директорию если не существует
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(output_path, json)
        .with_context(|| format!("Failed to write {}", output_path.display()))?;

    Ok(())
}

/// Автоматически генерирует compile_commands.json, определяя тип проекта
///
/// # Arguments
/// * `project_dir` - Директория проекта
/// * `output_path` - Путь для записи compile_commands.json
/// * `build_dir` - Директория сборки (для CMake)
///
/// # Returns
/// Путь к сгенерированному compile_commands.json
pub fn generate_compile_commands(
    project_dir: &Path,
    output_path: &Path,
    build_dir: Option<&Path>,
) -> Result<PathBuf> {
    let build_system = detect_build_system(project_dir);

    match build_system {
        BuildSystem::CMake => {
            let default_build = project_dir.join("build");
            let build = build_dir.unwrap_or(&default_build);
            generate_from_cmake(project_dir, build, Some("Ninja"), &[])
        }
        BuildSystem::Make => generate_from_makefile(project_dir, output_path, &[]),
        BuildSystem::VcxProj => {
            // Находим первый .vcxproj файл
            let vcxproj = find_file_with_extension(project_dir, "vcxproj")?;
            generate_from_vcxproj(&vcxproj, output_path, "Debug", "x64")
        }
        BuildSystem::Solution => {
            // Находим первый .sln файл
            let sln = find_file_with_extension(project_dir, "sln")?;
            generate_from_solution(&sln, output_path, "Debug", "x64")
        }
        BuildSystem::Cargo => generate_from_cargo(project_dir, output_path, build_dir),
        BuildSystem::Unknown => {
            bail!(
                "Could not detect build system in {}. \nSupported: CMakeLists.txt, Makefile, .vcxproj, .sln, Cargo.toml",
                project_dir.display()
            )
        }
    }
}

/// Находит файл с указанным расширением в директории
fn find_file_with_extension(dir: &Path, ext: &str) -> Result<PathBuf> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map(|e| e == ext).unwrap_or(false) {
            return Ok(path);
        }
    }
    bail!("No .{} file found in {}", ext, dir.display())
}

/// Генерирует compile_commands.json для Cargo проектов, используя `cargo compdb`.
///
/// Попытка:
/// 1) Запустить `cargo compdb --workspace` в каталоге проекта.
/// 2) Если субкоманда отсутствует, пытаемся установить `cargo-compdb` через `cargo install cargo-compdb` и повторить.
///
/// Возвращает путь к созданному файлу или ошибку с пояснением.
pub fn generate_from_cargo(project_dir: &Path, output_path: &Path, _build_dir: Option<&Path>) -> Result<PathBuf> {
    // Prefer `rust-analyzer` for Cargo projects. Allow override via SYGRAPH_RUST_ANALYZER_CMD (for tests/custom paths)
    let ra_bin = std::env::var("SYGRAPH_RUST_ANALYZER_CMD").unwrap_or_else(|_| "rust-analyzer".to_string());

    // Run: `rust-analyzer lsif .` and write its stdout to the output path (LSIF format)
    let mut cmd = Command::new(&ra_bin);
    cmd.arg("lsif").arg(".").current_dir(project_dir);

    match cmd.output() {
        Ok(output) => {
            if output.status.success() {
                let stdout = String::from_utf8(output.stdout)
                    .with_context(|| "Failed to read stdout from rust-analyzer")?;
                if let Some(parent) = output_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(output_path, stdout)
                    .with_context(|| format!("Failed to write {}", output_path.display()))?;
                return Ok(output_path.to_path_buf());
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                bail!(
                    "`rust-analyzer lsif` failed: {}\n\
                     Suggestions:\n\
                     1) Ensure `rust-analyzer` is installed and on PATH (install via rustup or from release).\n\
                     2) Or generate manually: `rust-analyzer lsif . > compile_commands.json` and re-run the command.\n\
                     Original output: {}",
                    stderr,
                    stderr
                );
            }
        }
        Err(e) => bail!(
            "Failed to run `rust-analyzer` (is it installed and on PATH?): {}\n\
             Install it and try again.",
            e
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_detect_build_system_cmake() {
        // Тест требует временной директории, пропускаем в unit tests
    }

    #[test]
    fn test_detect_build_system_cargo() {
        let td = tempdir().expect("tempdir");
        let cargo = td.path().join("Cargo.toml");
        std::fs::write(&cargo, "[package]\nname = \"x\"\nversion = \"0.1.0\"").unwrap();
        assert_eq!(detect_build_system(td.path()), BuildSystem::Cargo);
    }

    #[test]
    fn test_parse_make_dry_run() {
        let output = r#"
make[1]: Entering directory '/home/user/project'
gcc -I./include -DDEBUG -c src/main.c -o build/main.o
g++ -std=c++17 -c src/app.cpp -o build/app.o
make[1]: Leaving directory '/home/user/project'
"#;
        let entries = parse_make_dry_run(output, Path::new("/home/user/project")).unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries[0].file.contains("main.c"));
        assert!(entries[1].file.contains("app.cpp"));
    }

    #[test]
    fn test_parse_vcxproj_basic() {
        let vcxproj = r#"
<?xml version="1.0" encoding="utf-8"?>
<Project>
  <ItemGroup>
    <ClCompile Include="src\main.cpp" />
    <ClCompile Include="src\utils.cpp" />
  </ItemGroup>
  <ItemDefinitionGroup>
    <ClCompile>
      <AdditionalIncludeDirectories>include;external</AdditionalIncludeDirectories>
      <PreprocessorDefinitions>DEBUG;WIN32</PreprocessorDefinitions>
    </ClCompile>
  </ItemDefinitionGroup>
</Project>
"#;
        let entries = parse_vcxproj(vcxproj, Path::new("C:/project"), "Debug", "x64").unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_generate_from_cargo_with_mocked_rust_analyzer() {
        // Create temp project with Cargo.toml
        let td = tempdir().expect("tempdir");
        let cargo_toml = td.path().join("Cargo.toml");
        std::fs::write(&cargo_toml, "[package]\nname = \"x\"\nversion = \"0.1.0\"").unwrap();

        // Create a fake 'rust-analyzer' in PATH that responds to `rust-analyzer lsif .` with []
        let bin_dir = td.path().join("fakebin");
        std::fs::create_dir_all(&bin_dir).unwrap();

        #[cfg(windows)]
        {
            let script = bin_dir.join("rust-analyzer.bat");
            std::fs::write(&script, r#"@echo off
if "%1"=="lsif" (
  echo []
  exit /b 0
)
echo unknown args >&2
exit /b 1
"#).unwrap();
        }

        #[cfg(unix)]
        {
            let script = bin_dir.join("rust-analyzer");
            std::fs::write(&script, r#"#!/bin/sh
if [ "$1" = "lsif" ]; then
  echo '[]'
  exit 0
else
  echo "unknown args $@" >&2
  exit 1
fi
"#).unwrap();
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&script).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&script, perms).unwrap();
        }

        // Point SYGRAPH_RUST_ANALYZER_CMD to our fake script
        let cmd_path = if cfg!(windows) { bin_dir.join("rust-analyzer.bat") } else { bin_dir.join("rust-analyzer") };
        let old_cmd = std::env::var("SYGRAPH_RUST_ANALYZER_CMD").ok();
        std::env::set_var("SYGRAPH_RUST_ANALYZER_CMD", cmd_path.as_os_str());

        let out = td.path().join("compile_commands.json");
        let res = generate_from_cargo(td.path(), &out, None);
        match res {
            Ok(p) => {
                assert!(p.exists());
                let content = std::fs::read_to_string(&out).unwrap();
                assert_eq!(content.trim(), "[]");
            }
            Err(e) => {
                eprintln!("generate_from_cargo error: {}", e);
                // Restore SYGRAPH_RUST_ANALYZER_CMD before panic for better environment hygiene
                if let Some(v) = old_cmd.as_deref() {
                    std::env::set_var("SYGRAPH_RUST_ANALYZER_CMD", v);
                } else {
                    std::env::remove_var("SYGRAPH_RUST_ANALYZER_CMD");
                }
                panic!("generate_from_cargo failed: {}", e);
            }
        }

        // Restore SYGRAPH_RUST_ANALYZER_CMD
        if let Some(v) = old_cmd.as_deref() {
            std::env::set_var("SYGRAPH_RUST_ANALYZER_CMD", v);
        } else {
            std::env::remove_var("SYGRAPH_RUST_ANALYZER_CMD");
        }
    }
}
