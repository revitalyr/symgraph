//! # SCIP (Source Code Intelligence Protocol) поддержка
//!
//! Модуль для генерации и парсинга SCIP индексов для различных языков.
//! SCIP предоставляет унифицированный формат для представления символов,
//! их определений, ссылок и отношений в коде.
//!
//! ## Поддерживаемые языки и инструменты
//!
//! - **Rust**: `scip-rust` - `cargo install scip-rust`
//! - **C++**: `scip-clang` - требует compile_commands.json
//! - **Python**: `scip-python` - `pip install scip-python`
//! - **JavaScript/TypeScript**: `@sourcegraph/scip-typescript`
//! - **Shell**: `scip-shell`
//! - **Ruby**: `scip-ruby`
//! - **PHP**: `sourcegraph/scip-php`
//! - **Lua**: `scip-lua`

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Язык программирования для SCIP индексации
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ScipLanguage {
    Rust,
    Cpp,
    Python,
    JavaScript,
    TypeScript,
    Shell,
    Ruby,
    PHP,
    Lua,
    Unknown,
}

impl std::fmt::Display for ScipLanguage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScipLanguage::Rust => write!(f, "Rust"),
            ScipLanguage::Cpp => write!(f, "C++"),
            ScipLanguage::Python => write!(f, "Python"),
            ScipLanguage::JavaScript => write!(f, "JavaScript"),
            ScipLanguage::TypeScript => write!(f, "TypeScript"),
            ScipLanguage::Shell => write!(f, "Shell"),
            ScipLanguage::Ruby => write!(f, "Ruby"),
            ScipLanguage::PHP => write!(f, "PHP"),
            ScipLanguage::Lua => write!(f, "Lua"),
            ScipLanguage::Unknown => write!(f, "Unknown"),
        }
    }
}

impl From<&str> for ScipLanguage {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "rust" | "rs" => ScipLanguage::Rust,
            "c++" | "cpp" | "cxx" | "cc" | "c" => ScipLanguage::Cpp,
            "python" | "py" => ScipLanguage::Python,
            "javascript" | "js" => ScipLanguage::JavaScript,
            "typescript" | "ts" => ScipLanguage::TypeScript,
            "shell" | "bash" | "sh" => ScipLanguage::Shell,
            "ruby" | "rb" => ScipLanguage::Ruby,
            "php" => ScipLanguage::PHP,
            "lua" => ScipLanguage::Lua,
            _ => ScipLanguage::Unknown,
        }
    }
}

/// Конфигурация для генерации SCIP индекса
#[derive(Debug, Clone)]
pub struct ScipConfig {
    /// Язык программирования
    pub language: ScipLanguage,
    /// Путь к проекту
    pub project_path: PathBuf,
    /// Путь для вывода SCIP файла
    pub output_path: PathBuf,
    /// Дополнительные аргументы для SCIP инструмента
    pub extra_args: Vec<String>,
    /// Путь к compile_commands.json (требуется для C++)
    pub compile_commands: Option<PathBuf>,
}

impl ScipConfig {
    pub fn new(language: ScipLanguage, project_path: impl AsRef<Path>, output_path: impl AsRef<Path>) -> Self {
        Self {
            language,
            project_path: project_path.as_ref().to_path_buf(),
            output_path: output_path.as_ref().to_path_buf(),
            extra_args: Vec::new(),
            compile_commands: None,
        }
    }

    pub fn with_extra_args(mut self, args: Vec<String>) -> Self {
        self.extra_args = args;
        self
    }

    pub fn with_compile_commands(mut self, compdb: impl AsRef<Path>) -> Self {
        self.compile_commands = Some(compdb.as_ref().to_path_buf());
        self
    }
}

/// Генерирует SCIP индекс для указанного языка
pub fn generate_scip_index(config: &ScipConfig) -> Result<PathBuf> {
    match config.language {
        ScipLanguage::Rust => generate_rust_scip(config),
        ScipLanguage::Cpp => generate_cpp_scip(config),
        ScipLanguage::Python => generate_python_scip(config),
        ScipLanguage::JavaScript | ScipLanguage::TypeScript => generate_typescript_scip(config),
        ScipLanguage::Shell => generate_shell_scip(config),
        ScipLanguage::Ruby => generate_ruby_scip(config),
        ScipLanguage::PHP => generate_php_scip(config),
        ScipLanguage::Lua => generate_lua_scip(config),
        ScipLanguage::Unknown => bail!("Unknown language for SCIP generation"),
    }
}

/// Генерирует SCIP индекс для Rust с помощью rust-analyzer
fn generate_rust_scip(config: &ScipConfig) -> Result<PathBuf> {
    println!("Generating SCIP index for Rust project: {}", config.project_path.display());

    let mut cmd = Command::new("rust-analyzer");
    cmd.arg("scip")
        .arg(".")
        .arg("--output")
        .arg(&config.output_path)
        .arg("--exclude-vendored-libraries")
        .current_dir(&config.project_path);

    // Добавляем дополнительные аргументы
    for arg in &config.extra_args {
        cmd.arg(arg);
    }

    let output = cmd.output()
        .with_context(|| "Failed to execute rust-analyzer. Install with: rustup component add rust-analyzer")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("rust-analyzer scip failed:\n{}", stderr);
    }

    if !config.output_path.exists() {
        bail!("SCIP file was not generated: {}", config.output_path.display());
    }

    let file_size = fs::metadata(&config.output_path)
        .with_context(|| format!("Failed to read SCIP file metadata: {}", config.output_path.display()))?
        .len();

    println!("Generated SCIP index: {} ({} bytes)", config.output_path.display(), file_size);
    Ok(config.output_path.clone())
}

/// Генерирует SCIP индекс для C++ с помощью scip-clang
fn generate_cpp_scip(config: &ScipConfig) -> Result<PathBuf> {
    let compdb = config.compile_commands.as_ref()
        .ok_or_else(|| anyhow::anyhow!("compile_commands.json is required for C++ SCIP generation"))?;

    if !compdb.exists() {
        bail!("compile_commands.json not found: {}", compdb.display());
    }

    println!("Generating SCIP index for C++ project: {}", config.project_path.display());

    let mut cmd = Command::new("scip-clang");
    cmd.arg("index")
        .arg(compdb)
        .arg("--output")
        .arg(&config.output_path)
        .current_dir(&config.project_path);

    // Добавляем дополнительные аргументы
    for arg in &config.extra_args {
        cmd.arg(arg);
    }

    let output = cmd.output()
        .with_context(|| "Failed to execute scip-clang. Install with: cargo install scip-clang")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("scip-clang failed:\n{}", stderr);
    }

    if !config.output_path.exists() {
        bail!("SCIP file was not generated: {}", config.output_path.display());
    }

    println!("Generated SCIP index: {}", config.output_path.display());
    Ok(config.output_path.clone())
}

/// Генерирует SCIP индекс для Python с помощью scip-python
fn generate_python_scip(config: &ScipConfig) -> Result<PathBuf> {
    println!("Generating SCIP index for Python project: {}", config.project_path.display());

    let mut cmd = Command::new("scip-python");
    cmd.arg("index")
        .arg(".")
        .arg("--output")
        .arg(&config.output_path)
        .current_dir(&config.project_path);

    // Добавляем дополнительные аргументы
    for arg in &config.extra_args {
        cmd.arg(arg);
    }

    let output = cmd.output()
        .with_context(|| "Failed to execute scip-python. Install with: pip install scip-python")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("scip-python failed:\n{}", stderr);
    }

    if !config.output_path.exists() {
        bail!("SCIP file was not generated: {}", config.output_path.display());
    }

    println!("Generated SCIP index: {}", config.output_path.display());
    Ok(config.output_path.clone())
}

/// Генерирует SCIP индекс для JavaScript/TypeScript с помощью scip-typescript
fn generate_typescript_scip(config: &ScipConfig) -> Result<PathBuf> {
    println!("Generating SCIP index for TypeScript/JavaScript project: {}", config.project_path.display());

    let mut cmd = Command::new("scip-typescript");
    cmd.arg("index")
        .arg("--project-root")
        .arg(".")
        .arg("--output")
        .arg(&config.output_path)
        .current_dir(&config.project_path);

    // Добавляем дополнительные аргументы
    for arg in &config.extra_args {
        cmd.arg(arg);
    }

    let output = cmd.output()
        .with_context(|| "Failed to execute scip-typescript. Install with: npm install -g @sourcegraph/scip-typescript")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("scip-typescript failed:\n{}", stderr);
    }

    if !config.output_path.exists() {
        bail!("SCIP file was not generated: {}", config.output_path.display());
    }

    println!("Generated SCIP index: {}", config.output_path.display());
    Ok(config.output_path.clone())
}

/// Генерирует SCIP индекс для Shell с помощью scip-shell
fn generate_shell_scip(config: &ScipConfig) -> Result<PathBuf> {
    println!("Generating SCIP index for Shell project: {}", config.project_path.display());

    let mut cmd = Command::new("scip-shell");
    cmd.arg("index")
        .arg(".")
        .current_dir(&config.project_path);

    // scip-shell выводит в stdout, перенаправляем в файл
    let output = cmd.output()
        .with_context(|| "Failed to execute scip-shell. Install with: cargo install scip-shell")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("scip-shell failed:\n{}", stderr);
    }

    // Создаем директорию если нужно
    if let Some(parent) = config.output_path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(&config.output_path, &output.stdout)
        .with_context(|| format!("Failed to write SCIP file: {}", config.output_path.display()))?;

    println!("Generated SCIP index: {}", config.output_path.display());
    Ok(config.output_path.clone())
}

/// Генерирует SCIP индекс для Ruby с помощью scip-ruby
fn generate_ruby_scip(config: &ScipConfig) -> Result<PathBuf> {
    println!("Generating SCIP index for Ruby project: {}", config.project_path.display());

    let mut cmd = Command::new("scip-ruby");
    cmd.arg("index")
        .arg(".")
        .arg("--output")
        .arg(&config.output_path)
        .current_dir(&config.project_path);

    // Добавляем дополнительные аргументы
    for arg in &config.extra_args {
        cmd.arg(arg);
    }

    let output = cmd.output()
        .with_context(|| "Failed to execute scip-ruby. Install with: gem install scip-ruby")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("scip-ruby failed:\n{}", stderr);
    }

    if !config.output_path.exists() {
        bail!("SCIP file was not generated: {}", config.output_path.display());
    }

    println!("Generated SCIP index: {}", config.output_path.display());
    Ok(config.output_path.clone())
}

/// Генерирует SCIP индекс для PHP с помощью scip-php
fn generate_php_scip(config: &ScipConfig) -> Result<PathBuf> {
    println!("Generating SCIP index for PHP project: {}", config.project_path.display());

    let mut cmd = Command::new("vendor/bin/scip-php");
    cmd.arg("index")
        .arg("--output")
        .arg(&config.output_path)
        .current_dir(&config.project_path);

    // Добавляем дополнительные аргументы
    for arg in &config.extra_args {
        cmd.arg(arg);
    }

    let output = cmd.output()
        .with_context(|| "Failed to execute scip-php. Install with: composer require sourcegraph/scip-php")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("scip-php failed:\n{}", stderr);
    }

    if !config.output_path.exists() {
        bail!("SCIP file was not generated: {}", config.output_path.display());
    }

    println!("Generated SCIP index: {}", config.output_path.display());
    Ok(config.output_path.clone())
}

/// Генерирует SCIP индекс для Lua с помощью scip-lua
fn generate_lua_scip(config: &ScipConfig) -> Result<PathBuf> {
    println!("Generating SCIP index for Lua project: {}", config.project_path.display());

    let mut cmd = Command::new("scip-lua");
    cmd.arg("index")
        .arg(".")
        .arg("--output")
        .arg(&config.output_path)
        .current_dir(&config.project_path);

    // Добавляем дополнительные аргументы
    for arg in &config.extra_args {
        cmd.arg(arg);
    }

    let output = cmd.output()
        .with_context(|| "Failed to execute scip-lua. Install scip-lua from: https://github.com/sourcegraph/scip-lua")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("scip-lua failed:\n{}", stderr);
    }

    if !config.output_path.exists() {
        bail!("SCIP file was not generated: {}", config.output_path.display());
    }

    println!("Generated SCIP index: {}", config.output_path.display());
    Ok(config.output_path.clone())
}

/// Автоматически определяет язык проекта по файлам в директории
pub fn detect_language(project_dir: &Path) -> ScipLanguage {
    // Проверяем наличие файлов для каждого языка
    if project_dir.join("Cargo.toml").exists() {
        return ScipLanguage::Rust;
    }

    if let Ok(dir_entries) = std::fs::read_dir(project_dir) {
        for entry in dir_entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    log::warn!("Failed to read directory entry in '{}': {}", project_dir.display(), e);
                    continue;
                }
            };
            let path = entry.path();
            if let Some(ext) = path.extension() {
                match ext.to_str().unwrap_or("") {
                    "rs" => return ScipLanguage::Rust,
                    "cpp" | "cxx" | "cc" | "c" | "h" | "hpp" | "hxx" => return ScipLanguage::Cpp,
                    "py" => return ScipLanguage::Python,
                    "js" | "mjs" => return ScipLanguage::JavaScript,
                    "ts" => return ScipLanguage::TypeScript,
                    "sh" | "bash" => return ScipLanguage::Shell,
                    "rb" => return ScipLanguage::Ruby,
                    "php" => return ScipLanguage::PHP,
                    "lua" => return ScipLanguage::Lua,
                    _ => {}
                }
            }
        }
    }

    ScipLanguage::Unknown
}

/// Проверяет доступность SCIP инструмента для указанного языка
pub fn check_scip_tool_availability(language: &ScipLanguage) -> Result<bool> {
    let tool_name = match language {
        ScipLanguage::Rust => "rust-analyzer",
        ScipLanguage::Cpp => "scip-clang",
        ScipLanguage::Python => "scip-python",
        ScipLanguage::JavaScript | ScipLanguage::TypeScript => "scip-typescript",
        ScipLanguage::Shell => "scip-shell",
        ScipLanguage::Ruby => "scip-ruby",
        ScipLanguage::PHP => "vendor/bin/scip-php",
        ScipLanguage::Lua => "scip-lua",
        ScipLanguage::Unknown => return Ok(false),
    };

    let output = Command::new(tool_name).arg("--help").output();
    match output {
        Ok(result) => Ok(result.status.success()),
        Err(_) => Ok(false),
    }
}

/// Возвращает инструкцию по установке SCIP инструмента
pub fn get_installation_instruction(language: &ScipLanguage) -> &'static str {
    match language {
        ScipLanguage::Rust => "rustup component add rust-analyzer",
        ScipLanguage::Cpp => "cargo install scip-clang",
        ScipLanguage::Python => "pip install scip-python",
        ScipLanguage::JavaScript | ScipLanguage::TypeScript => "npm install -g @sourcegraph/scip-typescript",
        ScipLanguage::Shell => "cargo install scip-shell",
        ScipLanguage::Ruby => "gem install scip-ruby",
        ScipLanguage::PHP => "composer require sourcegraph/scip-php",
        ScipLanguage::Lua => "Install scip-lua from: https://github.com/sourcegraph/scip-lua",
        ScipLanguage::Unknown => "Unknown language",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_detection() {
        assert_eq!(detect_language(Path::new("/tmp/Cargo.toml").parent().unwrap()), ScipLanguage::Rust);
        assert_eq!(ScipLanguage::from("rust"), ScipLanguage::Rust);
        assert_eq!(ScipLanguage::from("cpp"), ScipLanguage::Cpp);
        assert_eq!(ScipLanguage::from("python"), ScipLanguage::Python);
    }

    #[test]
    fn test_scip_config() {
        let config = ScipConfig::new(
            ScipLanguage::Rust,
            "/tmp/project",
            "/tmp/output.scip"
        ).with_extra_args(vec!["--verbose".to_string()]);

        assert_eq!(config.language, ScipLanguage::Rust);
        assert_eq!(config.extra_args.len(), 1);
    }
}
