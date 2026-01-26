"""
Unit tests for project type detection in Enhanced Symgraph GUI.

Tests the core functionality of detecting different project types
based on indicator files and source file extensions.
"""

import unittest
import tempfile
import os
from pathlib import Path
import sys

# Add parent directory to path to import the GUI module
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

# Import the project configuration from enhanced_symgraph_gui
# We'll test the detection logic without requiring tkinter
class MockEnhancedGUI:
    """Mock class with project detection logic"""
    
    def __init__(self):
        self.project_configs = {
            'C++': {
                'indicators': ['CMakeLists.txt', 'Makefile', '*.vcxproj', '*.sln'],
                'extensions': ['.cpp', '.cc', '.cxx', '.c', '.h', '.hpp', '.hxx'],
                'command': 'scan-cxx',
            },
            'Rust': {
                'indicators': ['Cargo.toml'],
                'extensions': ['.rs'],
                'command': 'scan-rust',
            },
            'Python': {
                'indicators': ['requirements.txt', 'setup.py', 'pyproject.toml', 'setup.cfg'],
                'extensions': ['.py'],
                'command': 'scan-scripts',
            },
            'JavaScript/TypeScript': {
                'indicators': ['package.json', 'tsconfig.json'],
                'extensions': ['.js', '.ts', '.jsx', '.tsx', '.mjs'],
                'command': 'scan-scripts',
            }
        }
    
    def is_project_type(self, path: str, config: dict) -> bool:
        """Check if directory matches project type"""
        if not os.path.exists(path):
            return False
            
        files = os.listdir(path)
        
        for indicator in config['indicators']:
            if indicator.startswith('*'):
                ext = indicator[1:]
                if any(f.endswith(ext) for f in files):
                    return True
            else:
                if indicator in files:
                    return True
        return False
    
    def count_source_files(self, path: str, extensions: list) -> int:
        """Count source files in directory"""
        count = 0
        for root, dirs, files in os.walk(path):
            # Skip common build/cache directories
            dirs[:] = [d for d in dirs if d not in ['build', 'target', 'node_modules', '__pycache__', '.git']]
            count += sum(1 for f in files if any(f.endswith(ext) for ext in extensions))
        return count


class TestProjectTypeDetection(unittest.TestCase):
    """Test suite for project type detection"""
    
    def setUp(self):
        """Set up test fixtures"""
        self.gui = MockEnhancedGUI()
        self.temp_dir = tempfile.mkdtemp()
    
    def tearDown(self):
        """Clean up test fixtures"""
        import shutil
        shutil.rmtree(self.temp_dir, ignore_errors=True)
    
    def create_test_project(self, files: list) -> str:
        """Helper to create a test project with specified files"""
        for file_path in files:
            full_path = os.path.join(self.temp_dir, file_path)
            os.makedirs(os.path.dirname(full_path), exist_ok=True)
            Path(full_path).touch()
        return self.temp_dir
    
    # C++ Project Tests
    
    def test_detect_cpp_cmake_project(self):
        """Test detection of C++ project with CMakeLists.txt"""
        self.create_test_project(['CMakeLists.txt', 'main.cpp', 'utils.h'])
        config = self.gui.project_configs['C++']
        self.assertTrue(self.gui.is_project_type(self.temp_dir, config))
    
    def test_detect_cpp_makefile_project(self):
        """Test detection of C++ project with Makefile"""
        self.create_test_project(['Makefile', 'main.cpp'])
        config = self.gui.project_configs['C++']
        self.assertTrue(self.gui.is_project_type(self.temp_dir, config))
    
    def test_detect_cpp_vcxproj_project(self):
        """Test detection of C++ project with .vcxproj file"""
        self.create_test_project(['MyProject.vcxproj', 'main.cpp'])
        config = self.gui.project_configs['C++']
        self.assertTrue(self.gui.is_project_type(self.temp_dir, config))
    
    def test_detect_cpp_sln_project(self):
        """Test detection of C++ project with .sln file"""
        self.create_test_project(['MySolution.sln', 'main.cpp'])
        config = self.gui.project_configs['C++']
        self.assertTrue(self.gui.is_project_type(self.temp_dir, config))
    
    def test_count_cpp_source_files(self):
        """Test counting C++ source files"""
        self.create_test_project([
            'CMakeLists.txt',
            'src/main.cpp',
            'src/utils.cpp',
            'include/utils.h',
            'include/types.hpp',
            'README.md'  # Should not be counted
        ])
        config = self.gui.project_configs['C++']
        count = self.gui.count_source_files(self.temp_dir, config['extensions'])
        self.assertEqual(count, 4)  # .cpp, .cpp, .h, .hpp
    
    # Rust Project Tests
    
    def test_detect_rust_project(self):
        """Test detection of Rust project with Cargo.toml"""
        self.create_test_project(['Cargo.toml', 'src/main.rs'])
        config = self.gui.project_configs['Rust']
        self.assertTrue(self.gui.is_project_type(self.temp_dir, config))
    
    def test_count_rust_source_files(self):
        """Test counting Rust source files"""
        self.create_test_project([
            'Cargo.toml',
            'src/main.rs',
            'src/lib.rs',
            'src/utils.rs',
            'tests/integration_test.rs',
            'README.md'  # Should not be counted
        ])
        config = self.gui.project_configs['Rust']
        count = self.gui.count_source_files(self.temp_dir, config['extensions'])
        self.assertEqual(count, 4)  # 4 .rs files
    
    def test_rust_project_without_cargo_toml(self):
        """Test that Rust project is not detected without Cargo.toml"""
        self.create_test_project(['src/main.rs'])
        config = self.gui.project_configs['Rust']
        self.assertFalse(self.gui.is_project_type(self.temp_dir, config))
    
    # Python Project Tests
    
    def test_detect_python_requirements_project(self):
        """Test detection of Python project with requirements.txt"""
        self.create_test_project(['requirements.txt', 'main.py'])
        config = self.gui.project_configs['Python']
        self.assertTrue(self.gui.is_project_type(self.temp_dir, config))
    
    def test_detect_python_setup_py_project(self):
        """Test detection of Python project with setup.py"""
        self.create_test_project(['setup.py', 'mypackage/__init__.py'])
        config = self.gui.project_configs['Python']
        self.assertTrue(self.gui.is_project_type(self.temp_dir, config))
    
    def test_detect_python_pyproject_toml_project(self):
        """Test detection of Python project with pyproject.toml"""
        self.create_test_project(['pyproject.toml', 'src/main.py'])
        config = self.gui.project_configs['Python']
        self.assertTrue(self.gui.is_project_type(self.temp_dir, config))
    
    def test_count_python_source_files(self):
        """Test counting Python source files"""
        self.create_test_project([
            'requirements.txt',
            'main.py',
            'utils.py',
            'tests/test_main.py',
            'README.md'  # Should not be counted
        ])
        config = self.gui.project_configs['Python']
        count = self.gui.count_source_files(self.temp_dir, config['extensions'])
        self.assertEqual(count, 3)  # 3 .py files
    
    def test_python_excludes_pycache(self):
        """Test that __pycache__ directories are excluded"""
        self.create_test_project([
            'requirements.txt',
            'main.py',
            '__pycache__/main.cpython-39.pyc'
        ])
        config = self.gui.project_configs['Python']
        count = self.gui.count_source_files(self.temp_dir, config['extensions'])
        self.assertEqual(count, 1)  # Only main.py
    
    # JavaScript/TypeScript Project Tests
    
    def test_detect_javascript_package_json_project(self):
        """Test detection of JavaScript project with package.json"""
        self.create_test_project(['package.json', 'index.js'])
        config = self.gui.project_configs['JavaScript/TypeScript']
        self.assertTrue(self.gui.is_project_type(self.temp_dir, config))
    
    def test_detect_typescript_tsconfig_project(self):
        """Test detection of TypeScript project with tsconfig.json"""
        self.create_test_project(['tsconfig.json', 'index.ts'])
        config = self.gui.project_configs['JavaScript/TypeScript']
        self.assertTrue(self.gui.is_project_type(self.temp_dir, config))
    
    def test_count_javascript_typescript_files(self):
        """Test counting JavaScript and TypeScript files"""
        self.create_test_project([
            'package.json',
            'src/index.js',
            'src/utils.ts',
            'src/Component.jsx',
            'src/App.tsx',
            'src/module.mjs',
            'README.md'  # Should not be counted
        ])
        config = self.gui.project_configs['JavaScript/TypeScript']
        count = self.gui.count_source_files(self.temp_dir, config['extensions'])
        self.assertEqual(count, 5)  # .js, .ts, .jsx, .tsx, .mjs
    
    def test_javascript_excludes_node_modules(self):
        """Test that node_modules directories are excluded"""
        self.create_test_project([
            'package.json',
            'index.js',
            'node_modules/package/index.js'
        ])
        config = self.gui.project_configs['JavaScript/TypeScript']
        count = self.gui.count_source_files(self.temp_dir, config['extensions'])
        self.assertEqual(count, 1)  # Only index.js in root
    
    # Edge Cases and Multiple Type Detection
    
    def test_empty_directory(self):
        """Test that empty directory is not detected as any project type"""
        for proj_type, config in self.gui.project_configs.items():
            with self.subTest(project_type=proj_type):
                self.assertFalse(self.gui.is_project_type(self.temp_dir, config))
    
    def test_mixed_cpp_rust_project(self):
        """Test detection when both C++ and Rust indicators present"""
        self.create_test_project([
            'CMakeLists.txt',
            'Cargo.toml',
            'main.cpp',
            'src/lib.rs'
        ])
        cpp_config = self.gui.project_configs['C++']
        rust_config = self.gui.project_configs['Rust']
        
        self.assertTrue(self.gui.is_project_type(self.temp_dir, cpp_config))
        self.assertTrue(self.gui.is_project_type(self.temp_dir, rust_config))
    
    def test_nonexistent_directory(self):
        """Test handling of non-existent directory"""
        fake_path = os.path.join(self.temp_dir, 'nonexistent')
        config = self.gui.project_configs['C++']
        self.assertFalse(self.gui.is_project_type(fake_path, config))
    
    def test_build_directory_exclusion(self):
        """Test that build directories are excluded from file counting"""
        self.create_test_project([
            'CMakeLists.txt',
            'src/main.cpp',
            'build/main.cpp',  # Should be excluded
            'target/debug/main.rs'  # Should be excluded
        ])
        config = self.gui.project_configs['C++']
        count = self.gui.count_source_files(self.temp_dir, config['extensions'])
        self.assertEqual(count, 1)  # Only src/main.cpp


class TestProjectConfigValidity(unittest.TestCase):
    """Test that project configurations are valid"""
    
    def setUp(self):
        self.gui = MockEnhancedGUI()
    
    def test_all_configs_have_required_fields(self):
        """Test that all project configs have required fields"""
        required_fields = ['indicators', 'extensions', 'command']
        
        for proj_type, config in self.gui.project_configs.items():
            with self.subTest(project_type=proj_type):
                for field in required_fields:
                    self.assertIn(field, config, 
                                f"{proj_type} config missing {field}")
    
    def test_indicators_are_non_empty(self):
        """Test that all configs have at least one indicator"""
        for proj_type, config in self.gui.project_configs.items():
            with self.subTest(project_type=proj_type):
                self.assertGreater(len(config['indicators']), 0,
                                 f"{proj_type} has no indicators")
    
    def test_extensions_are_non_empty(self):
        """Test that all configs have at least one extension"""
        for proj_type, config in self.gui.project_configs.items():
            with self.subTest(project_type=proj_type):
                self.assertGreater(len(config['extensions']), 0,
                                 f"{proj_type} has no extensions")
    
    def test_extensions_start_with_dot(self):
        """Test that all extensions start with a dot"""
        for proj_type, config in self.gui.project_configs.items():
            for ext in config['extensions']:
                with self.subTest(project_type=proj_type, extension=ext):
                    self.assertTrue(ext.startswith('.'),
                                  f"{proj_type} extension {ext} doesn't start with '.'")
    
    def test_command_is_valid(self):
        """Test that all commands are valid scan commands"""
        valid_commands = ['scan-cxx', 'scan-rust', 'scan-scripts']
        
        for proj_type, config in self.gui.project_configs.items():
            with self.subTest(project_type=proj_type):
                self.assertIn(config['command'], valid_commands,
                            f"{proj_type} has invalid command: {config['command']}")


def run_tests():
    """Run all tests and return results"""
    # Create test suite
    loader = unittest.TestLoader()
    suite = unittest.TestSuite()
    
    # Add all test classes
    suite.addTests(loader.loadTestsFromTestCase(TestProjectTypeDetection))
    suite.addTests(loader.loadTestsFromTestCase(TestProjectConfigValidity))
    
    # Run tests with verbose output
    runner = unittest.TextTestRunner(verbosity=2)
    result = runner.run(suite)
    
    return result


if __name__ == '__main__':
    result = run_tests()
    
    # Exit with appropriate code
    sys.exit(0 if result.wasSuccessful() else 1)
