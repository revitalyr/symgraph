#!/usr/bin/env python3
"""
Unified Symgraph GUI - combines indexing and viewing functionality
"""
import tkinter as tk
from tkinter import ttk, filedialog, messagebox, scrolledtext
import os
import subprocess
import threading
import json
import webbrowser
from pathlib import Path
from typing import Dict, List, Optional, Tuple
import tempfile
import shutil

class UnifiedSymgraphGUI:
    def __init__(self, root):
        self.root = root
        self.root.title("Symgraph - Unified Project Analyzer & Viewer")
        self.root.geometry("1200x800")
        
        # Settings
        self.settings_file = 'unified_gui_settings.json'
        self.load_settings()
        
        # Project configurations
        self.project_configs = {
            'C++': {
                'indicators': ['CMakeLists.txt', 'Makefile', '*.vcxproj', '*.sln'],
                'extensions': ['.cpp', '.cc', '.cxx', '.c', '.h', '.hpp', '.hxx'],
                'command': 'scan-cxx',
                'icon': '🔧',
                'color': '#3498db',
                'params': [
                    ('build_dir', 'Build Directory', 'Directory for build files', 'build'),
                    ('generator', 'CMake Generator', 'CMake generator to use', 'Ninja'),
                    ('compdb_path', 'CompDB Path', 'Path to compile_commands.json', 'auto')
                ]
            },
            'Rust': {
                'indicators': ['Cargo.toml'],
                'extensions': ['.rs'],
                'command': 'scan-rust',
                'icon': '🦀',
                'color': '#e67e22',
                'params': [
                    ('manifest_path', 'Manifest Path', 'Path to Cargo.toml', 'Cargo.toml'),
                    ('lsif_path', 'LSIF Path', 'Optional LSIF file path', '')
                ]
            },
            'Python': {
                'indicators': ['requirements.txt', 'setup.py', 'pyproject.toml'],
                'extensions': ['.py'],
                'command': 'scan-scripts',
                'icon': '🐍',
                'color': '#27ae60',
                'params': [
                    ('entry_point', 'Entry Point', 'Main Python file', 'main.py')
                ]
            },
            'JavaScript/TypeScript': {
                'indicators': ['package.json', 'tsconfig.json'],
                'extensions': ['.js', '.ts', '.jsx', '.tsx'],
                'command': 'scan-scripts',
                'icon': '📜',
                'color': '#f39c12',
                'params': [
                    ('entry_point', 'Entry Point', 'Main entry file', 'index.js')
                ]
            }
        }
        
        # State
        self.current_project = None
        self.current_db_path = None
        self.indexing_thread = None
        self.web_server_process = None
        
        self.create_widgets()
        self.apply_styles()
        
    def load_settings(self):
        """Load settings from file"""
        try:
            if os.path.exists(self.settings_file):
                with open(self.settings_file, 'r') as f:
                    self.settings = json.load(f)
            else:
                self.settings = {
                    'last_project_dir': '',
                    'last_db_path': '',
                    'window_geometry': '',
                    'auto_open_viewer': True
                }
        except Exception:
            self.settings = {
                'last_project_dir': '',
                'last_db_path': '',
                'window_geometry': '',
                'auto_open_viewer': True
            }
    
    def save_settings(self):
        """Save settings to file"""
        try:
            self.settings['window_geometry'] = self.root.geometry()
            with open(self.settings_file, 'w') as f:
                json.dump(self.settings, f, indent=2)
        except Exception as e:
            print(f"Failed to save settings: {e}")
    
    def create_widgets(self):
        """Create all GUI widgets"""
        # Create notebook for tabs
        self.notebook = ttk.Notebook(self.root)
        self.notebook.pack(fill='both', expand=True, padx=5, pady=5)
        
        # Create tabs
        self.create_indexing_tab()
        self.create_viewer_tab()
        self.create_settings_tab()
        
        # Status bar
        self.status_var = tk.StringVar()
        self.status_var.set("Ready")
        status_bar = ttk.Label(self.root, textvariable=self.status_var, relief='sunken')
        status_bar.pack(side='bottom', fill='x')
        
        # Apply saved geometry
        if self.settings.get('window_geometry'):
            self.root.geometry(self.settings['window_geometry'])
    
    def create_indexing_tab(self):
        """Create the indexing tab"""
        self.index_frame = ttk.Frame(self.notebook)
        self.notebook.add(self.index_frame, text="📊 Index Project")
        
        # Main container
        main_container = ttk.Frame(self.index_frame)
        main_container.pack(fill='both', expand=True, padx=10, pady=10)
        
        # Project selection section
        project_frame = ttk.LabelFrame(main_container, text="Project Selection", padding="10")
        project_frame.pack(fill='x', pady=(0, 10))
        
        # Directory selection
        dir_row = ttk.Frame(project_frame)
        dir_row.pack(fill='x', pady=5)
        
        ttk.Label(dir_row, text="Project Directory:").pack(side='left')
        self.project_dir_var = tk.StringVar(value=self.settings.get('last_project_dir', ''))
        self.project_dir_entry = ttk.Entry(dir_row, textvariable=self.project_dir_var, width=50)
        self.project_dir_entry.pack(side='left', padx=(10, 5), fill='x', expand=True)
        
        ttk.Button(dir_row, text="Browse...", command=self.browse_project_dir).pack(side='left', padx=5)
        ttk.Button(dir_row, text="Auto-Detect", command=self.auto_detect_project).pack(side='left')
        
        # Project type display
        self.project_type_var = tk.StringVar(value="Unknown")
        type_row = ttk.Frame(project_frame)
        type_row.pack(fill='x', pady=5)
        ttk.Label(type_row, text="Detected Type:").pack(side='left')
        ttk.Label(type_row, textvariable=self.project_type_var, font=('Arial', 10, 'bold')).pack(side='left', padx=(10, 0))
        
        # Configuration section
        config_frame = ttk.LabelFrame(main_container, text="Configuration", padding="10")
        config_frame.pack(fill='x', pady=(0, 10))
        
        # Database path
        db_row = ttk.Frame(config_frame)
        db_row.pack(fill='x', pady=5)
        ttk.Label(db_row, text="Database Path:").pack(side='left')
        self.db_path_var = tk.StringVar(value=self.settings.get('last_db_path', 'symgraph.db'))
        self.db_path_entry = ttk.Entry(db_row, textvariable=self.db_path_var, width=40)
        self.db_path_entry.pack(side='left', padx=(10, 5), fill='x', expand=True)
        ttk.Button(db_row, text="Browse...", command=self.browse_db_path).pack(side='left')
        
        # Dynamic parameters frame
        self.params_frame = ttk.Frame(config_frame)
        self.params_frame.pack(fill='x', pady=10)
        
        # Action buttons
        action_frame = ttk.Frame(main_container)
        action_frame.pack(fill='x', pady=10)
        
        self.index_button = ttk.Button(action_frame, text="🚀 Start Indexing", command=self.start_indexing, style='Accent.TButton')
        self.index_button.pack(side='left', padx=5)
        
        self.stop_button = ttk.Button(action_frame, text="⏹ Stop", command=self.stop_indexing, state='disabled')
        self.stop_button.pack(side='left', padx=5)
        
        ttk.Button(action_frame, text="🗑 Clear Database", command=self.clear_database).pack(side='left', padx=5)
        
        # Progress section
        progress_frame = ttk.LabelFrame(main_container, text="Progress", padding="10")
        progress_frame.pack(fill='both', expand=True)
        
        self.progress_var = tk.StringVar(value="No indexing in progress")
        ttk.Label(progress_frame, textvariable=self.progress_var).pack(anchor='w')
        
        self.progress_bar = ttk.Progressbar(progress_frame, mode='indeterminate')
        self.progress_bar.pack(fill='x', pady=5)
        
        # Output log
        log_frame = ttk.LabelFrame(progress_frame, text="Output Log", padding="5")
        log_frame.pack(fill='both', expand=True, pady=(10, 0))
        
        self.output_log = scrolledtext.ScrolledText(log_frame, height=10, wrap='word')
        self.output_log.pack(fill='both', expand=True)
        
    def create_viewer_tab(self):
        """Create the viewer tab"""
        self.viewer_frame = ttk.Frame(self.notebook)
        self.notebook.add(self.viewer_frame, text="👁️ View Results")
        
        # Main container
        main_container = ttk.Frame(self.viewer_frame)
        main_container.pack(fill='both', expand=True, padx=10, pady=10)
        
        # Database selection
        db_frame = ttk.LabelFrame(main_container, text="Database Selection", padding="10")
        db_frame.pack(fill='x', pady=(0, 10))
        
        db_row = ttk.Frame(db_frame)
        db_row.pack(fill='x')
        ttk.Label(db_row, text="Database:").pack(side='left')
        self.viewer_db_var = tk.StringVar(value=self.settings.get('last_db_path', 'symgraph.db'))
        self.viewer_db_entry = ttk.Entry(db_row, textvariable=self.viewer_db_var, width=50)
        self.viewer_db_entry.pack(side='left', padx=(10, 5), fill='x', expand=True)
        ttk.Button(db_row, text="Browse...", command=self.browse_viewer_db).pack(side='left')
        ttk.Button(db_row, text="🔄 Refresh", command=self.refresh_database_info).pack(side='left', padx=5)
        
        # Database info
        self.db_info_var = tk.StringVar(value="No database loaded")
        ttk.Label(db_frame, textvariable=self.db_info_var).pack(anchor='w', pady=(10, 0))
        
        # Action buttons
        action_frame = ttk.Frame(main_container)
        action_frame.pack(fill='x', pady=10)
        
        ttk.Button(action_frame, text="🌐 Open Web Viewer", command=self.open_web_viewer, 
                  style='Accent.TButton').pack(side='left', padx=5)
        ttk.Button(action_frame, text="📊 Show Statistics", command=self.show_statistics).pack(side='left', padx=5)
        
        # Results preview
        preview_frame = ttk.LabelFrame(main_container, text="Database Preview", padding="10")
        preview_frame.pack(fill='both', expand=True)
        
        # Create treeview for database preview
        columns = ('Type', 'Name', 'File', 'Count')
        self.results_tree = ttk.Treeview(preview_frame, columns=columns, show='tree headings')
        self.results_tree.pack(fill='both', expand=True)
        
        # Configure columns
        self.results_tree.heading('#0', text='Category')
        self.results_tree.column('#0', width=200)
        for col in columns:
            self.results_tree.heading(col, text=col)
            self.results_tree.column(col, width=150)
        
        # Scrollbar
        scrollbar = ttk.Scrollbar(preview_frame, orient='vertical', command=self.results_tree.yview)
        scrollbar.pack(side='right', fill='y')
        self.results_tree.configure(yscrollcommand=scrollbar.set)
        
    def create_settings_tab(self):
        """Create the settings tab"""
        self.settings_frame = ttk.Frame(self.notebook)
        self.notebook.add(self.settings_frame, text="⚙️ Settings")
        
        # Main container
        main_container = ttk.Frame(self.settings_frame)
        main_container.pack(fill='both', expand=True, padx=10, pady=10)
        
        # General settings
        general_frame = ttk.LabelFrame(main_container, text="General Settings", padding="10")
        general_frame.pack(fill='x', pady=(0, 10))
        
        self.auto_open_viewer_var = tk.BooleanVar(value=self.settings.get('auto_open_viewer', True))
        ttk.Checkbutton(general_frame, text="Auto-open web viewer after indexing", 
                       variable=self.auto_open_viewer_var).pack(anchor='w', pady=5)
        
        ttk.Button(general_frame, text="Clear All Settings", 
                  command=self.clear_settings).pack(pady=10)
        
        # About section
        about_frame = ttk.LabelFrame(main_container, text="About", padding="10")
        about_frame.pack(fill='x')
        
        about_text = """
Symgraph Unified GUI
Version 1.0

A unified interface for indexing and viewing project code structure.

Features:
• Multi-language support (C++, Rust, Python, JavaScript/TypeScript)
• Automatic project detection
• Real-time indexing progress
• Web-based result viewer
• Database statistics and search
        """
        ttk.Label(about_frame, text=about_text.strip(), justify='left').pack(anchor='w')
    
    def apply_styles(self):
        """Apply custom styles"""
        style = ttk.Style()
        style.configure('Accent.TButton', font=('Arial', 10, 'bold'))
    
    def browse_project_dir(self):
        """Browse for project directory"""
        directory = filedialog.askdirectory(initialdir=self.project_dir_var.get())
        if directory:
            self.project_dir_var.set(directory)
            self.detect_project_type()
    
    def browse_db_path(self):
        """Browse for database file"""
        filename = filedialog.asksaveasfilename(
            initialdir=os.path.dirname(self.db_path_var.get()),
            defaultextension=".db",
            filetypes=[("Sled Database", "*.db"), ("All Files", "*.*")]
        )
        if filename:
            self.db_path_var.set(filename)
    
    def browse_viewer_db(self):
        """Browse for viewer database file/directory"""
        # First try to select a directory (for Sled databases)
        directory = filedialog.askdirectory(
            initialdir=os.path.dirname(self.viewer_db_var.get()),
            title="Select Sled Database Directory"
        )
        
        if directory:
            # Check if this looks like a Sled database
            if os.path.exists(os.path.join(directory, 'db')):
                self.viewer_db_var.set(directory)
                self.refresh_database_info()
            else:
                messagebox.showwarning("Invalid Database", "Selected directory doesn't appear to be a valid Sled database")
                return
        
        # If no directory selected, fall back to file selection (for legacy support)
        else:
            filename = filedialog.askopenfilename(
                initialdir=os.path.dirname(self.viewer_db_var.get()),
                filetypes=[("Sled Database Directory", "*"), ("All Files", "*.*")]
            )
            if filename:
                self.viewer_db_var.set(filename)
                self.refresh_database_info()
    
    def auto_detect_project(self):
        """Auto-detect project type in current directory"""
        directory = filedialog.askdirectory()
        if directory:
            self.project_dir_var.set(directory)
            self.detect_project_type()
    
    def detect_project_type(self):
        """Detect project type from directory contents"""
        directory = self.project_dir_var.get()
        if not directory or not os.path.exists(directory):
            self.project_type_var.set("Directory not found")
            return
        
        detected_type = "Unknown"
        for project_type, config in self.project_configs.items():
            for indicator in config['indicators']:
                if '*' in indicator:
                    import glob
                    if glob.glob(os.path.join(directory, indicator)):
                        detected_type = project_type
                        break
                else:
                    if os.path.exists(os.path.join(directory, indicator)):
                        detected_type = project_type
                        break
            if detected_type != "Unknown":
                break
        
        self.project_type_var.set(detected_type)
        self.update_params_frame(detected_type)
    
    def update_params_frame(self, project_type):
        """Update parameters frame based on project type"""
        # Clear existing widgets
        for widget in self.params_frame.winfo_children():
            widget.destroy()
        
        if project_type not in self.project_configs:
            return
        
        config = self.project_configs[project_type]
        if 'params' not in config:
            return
        
        self.param_vars = {}
        
        for i, (param_name, param_label, param_desc, default_value) in enumerate(config['params']):
            row_frame = ttk.Frame(self.params_frame)
            row_frame.pack(fill='x', pady=2)
            
            ttk.Label(row_frame, text=f"{param_label}:").pack(side='left')
            
            var = tk.StringVar(value=default_value)
            self.param_vars[param_name] = var
            
            entry = ttk.Entry(row_frame, textvariable=var, width=30)
            entry.pack(side='left', padx=(10, 0), fill='x', expand=True)
    
    def start_indexing(self):
        """Start the indexing process"""
        project_dir = self.project_dir_var.get()
        db_path = self.db_path_var.get()
        
        if not project_dir or not os.path.exists(project_dir):
            messagebox.showerror("Error", "Please select a valid project directory")
            return
        
        if not db_path:
            messagebox.showerror("Error", "Please specify a database path")
            return
        
        # Detect project type
        project_type = self.project_type_var.get()
        if project_type == "Unknown":
            messagebox.showerror("Error", "Could not detect project type. Please select a valid project directory.")
            return
        
        # Update UI
        self.index_button.config(state='disabled')
        self.stop_button.config(state='normal')
        self.progress_bar.start()
        self.status_var.set("Indexing...")
        
        # Clear output log
        self.output_log.delete(1.0, tk.END)
        self.log_output(f"Starting indexing for {project_type} project: {project_dir}")
        
        # Save settings
        self.settings['last_project_dir'] = project_dir
        self.settings['last_db_path'] = db_path
        self.save_settings()
        
        # Start indexing in background thread
        self.indexing_thread = threading.Thread(target=self.run_indexing, args=(project_dir, db_path, project_type))
        self.indexing_thread.daemon = True
        self.indexing_thread.start()
    
    def run_indexing(self, project_dir, db_path, project_type):
        """Run indexing in background thread"""
        try:
            config = self.project_configs[project_type]
            command = config['command']
            
            # Build command arguments
            cmd_args = ['cargo', 'run', '--package', 'symgraph-cli', '--', command, '--db', db_path]
            
            # Add project-specific parameters
            if project_type == 'C++':
                build_dir = self.param_vars.get('build_dir', tk.StringVar()).get()
                if build_dir:
                    cmd_args.extend(['--build-dir', build_dir])
                
                generator = self.param_vars.get('generator', tk.StringVar()).get()
                if generator:
                    cmd_args.extend(['--generator', generator])
                
                compdb_path = self.param_vars.get('compdb_path', tk.StringVar()).get()
                if compdb_path and compdb_path != 'auto':
                    cmd_args.extend(['--compdb', compdb_path])
            
            elif project_type == 'Rust':
                manifest_path = self.param_vars.get('manifest_path', tk.StringVar()).get()
                if manifest_path:
                    # Convert relative path to absolute path
                    if not os.path.isabs(manifest_path):
                        full_manifest_path = os.path.join(project_dir, manifest_path)
                    else:
                        full_manifest_path = manifest_path
                    cmd_args.extend(['--manifest', full_manifest_path])
                
                lsif_path = self.param_vars.get('lsif_path', tk.StringVar()).get()
                if lsif_path:
                    cmd_args.extend(['--lsif', lsif_path])
            
            else:  # For C++, Python, JavaScript/TypeScript
                # Add project directory
                cmd_args.append(project_dir)
            
            self.log_output(f"Running command: {' '.join(cmd_args)}")
            
            # Run command
            process = subprocess.Popen(
                cmd_args,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                cwd='d:\\work\\Projects\\symgraph'
            )
            
            # Read output in real-time
            while True:
                output = process.stdout.readline()
                if output == '' and process.poll() is not None:
                    break
                if output:
                    self.log_output(output.strip())
            
            # Check result
            if process.returncode == 0:
                self.log_output("Indexing completed successfully!")
                self.progress_var.set("Indexing completed successfully!")
                
                # Update current project info
                self.current_project = project_dir
                self.current_db_path = db_path
                
                # Auto-open viewer if enabled
                if self.auto_open_viewer_var.get():
                    self.root.after(1000, self.open_web_viewer)
            else:
                self.log_output(f"Indexing failed with return code: {process.returncode}")
                self.progress_var.set("Indexing failed!")
        
        except Exception as e:
            self.log_output(f"Error during indexing: {str(e)}")
            self.progress_var.set(f"Error: {str(e)}")
        
        finally:
            # Update UI
            self.root.after(0, self.indexing_complete)
    
    def indexing_complete(self):
        """Called when indexing is complete"""
        self.index_button.config(state='normal')
        self.stop_button.config(state='disabled')
        self.progress_bar.stop()
        self.status_var.set("Ready")
        
        # Refresh database info
        self.refresh_database_info()
    
    def stop_indexing(self):
        """Stop the indexing process"""
        if self.indexing_thread and self.indexing_thread.is_alive():
            self.log_output("Stopping indexing...")
            self.progress_var.set("Stopping...")
    
    def log_output(self, message):
        """Log message to output widget"""
        self.output_log.insert(tk.END, f"{message}\n")
        self.output_log.see(tk.END)
        self.root.update_idletasks()
    
    def clear_database(self):
        """Clear the database"""
        db_path = self.db_path_var.get()
        if not db_path:
            messagebox.showerror("Error", "Please specify a database path")
            return
        
        if messagebox.askyesno("Confirm", "Are you sure you want to clear the database? This will delete all indexed data."):
            try:
                if os.path.exists(db_path):
                    if os.path.isdir(db_path):
                        # For Sled databases, remove the entire directory
                        import shutil
                        shutil.rmtree(db_path)
                    else:
                        # For files (legacy), just remove the file
                        os.remove(db_path)
                self.log_output(f"Database cleared: {db_path}")
                messagebox.showinfo("Success", "Database cleared successfully")
            except Exception as e:
                messagebox.showerror("Error", f"Failed to clear database: {str(e)}")
    
    def refresh_database_info(self):
        """Refresh database information"""
        db_path = self.viewer_db_var.get()
        if not db_path:
            self.db_info_var.set("No database selected")
            self.results_tree.delete(*self.results_tree.get_children())
            return
        
        # Check if database exists (Sled databases are directories)
        if not os.path.exists(db_path):
            self.db_info_var.set("Database not found")
            self.results_tree.delete(*self.results_tree.get_children())
            return
        
        # For Sled, check if it's a directory with the expected files
        if os.path.isdir(db_path):
            if not os.path.exists(os.path.join(db_path, 'db')):
                self.db_info_var.set("Invalid Sled database (missing db file)")
                self.results_tree.delete(*self.results_tree.get_children())
                return
        else:
            # If it's a file, it might be an old SQLite database
            self.db_info_var.set("Database appears to be SQLite format (not supported)")
            self.results_tree.delete(*self.results_tree.get_children())
            return
        
        try:
            # Use CLI API to get stats
            result = subprocess.run([
                'cargo', 'run', '--package', 'symgraph-cli', '--', 
                'api', 'stats', '--db', db_path
            ], capture_output=True, text=True, cwd='d:\\work\\Projects\\symgraph')
            
            if result.returncode == 0:
                stats = json.loads(result.stdout)
                file_count = stats.get('files', 0)
                symbol_count = stats.get('symbols', 0)
                edge_count = stats.get('edges', 0)
                
                self.db_info_var.set(f"Files: {file_count}, Symbols: {symbol_count}, Edges: {edge_count}")
                
                # Get files using CLI API
                files_result = subprocess.run([
                    'cargo', 'run', '--package', 'symgraph-cli', '--', 
                    'api', 'files', '--db', db_path
                ], capture_output=True, text=True, cwd='d:\\work\\Projects\\symgraph')
                
                # Populate tree view
                self.results_tree.delete(*self.results_tree.get_children())
                
                if files_result.returncode == 0:
                    files = json.loads(files_result.stdout)
                    
                    # Group files by category
                    categories = {}
                    for file_info in files:
                        category = file_info.get('category', 'unknown')
                        if category not in categories:
                            categories[category] = []
                        categories[category].append(file_info)
                    
                    # Add categories to tree
                    for category, files_list in categories.items():
                        category_node = self.results_tree.insert('', 'end', text=f"{category} ({len(files_list)})", values=('Category', category, '', len(files_list)))
                        
                        # Add files in this category (limit to 10 for performance)
                        for file_info in sorted(files_list, key=lambda x: x.get('symbol_count', 0), reverse=True)[:10]:
                            file_path = file_info.get('path', '')
                            symbol_count = file_info.get('symbol_count', 0)
                            self.results_tree.insert(category_node, 'end', text=os.path.basename(file_path), 
                                                   values=('File', os.path.basename(file_path), file_path, symbol_count))
                else:
                    self.db_info_var.set(f"Error getting files: {files_result.stderr}")
            else:
                self.db_info_var.set(f"Error reading database: {result.stderr}")
            
        except Exception as e:
            self.db_info_var.set(f"Error reading database: {str(e)}")
    
    def open_web_viewer(self):
        """Open web viewer"""
        db_path = self.current_db_path or self.viewer_db_var.get()
        if not db_path:
            messagebox.showerror("Error", "Please select a database path")
            return
        
        # Check if database exists (Sled databases are directories)
        if not os.path.exists(db_path):
            messagebox.showerror("Error", "Database not found")
            return
        
        # For Sled, check if it's a valid database directory
        if os.path.isdir(db_path):
            if not os.path.exists(os.path.join(db_path, 'db')):
                messagebox.showerror("Error", "Invalid Sled database (missing db file)")
                return
        else:
            messagebox.showerror("Error", "Database appears to be SQLite format (not supported)")
            return
        
        try:
            # Start Flask web server
            self.start_web_server(db_path)
            
            # Open browser
            webbrowser.open('http://localhost:5000')
            
            self.status_var.set("Web viewer opened at http://localhost:5000")
            
        except Exception as e:
            messagebox.showerror("Error", f"Failed to open web viewer: {str(e)}")
    
    def start_web_server(self, db_path):
        """Start Flask web server"""
        if self.web_server_process:
            self.web_server_process.terminate()
        
        # Simple Flask app content
        app_content = f'''
from flask import Flask, request, jsonify, send_from_directory
import subprocess
import json
import os

app = Flask(__name__)

def call_rust_api(endpoint, db_path, search=None):
    """Call Rust symgraph CLI to get data"""
    try:
        cmd = ['cargo', 'run', '--package', 'symgraph-cli', '--', 'api', endpoint, '--db', db_path]
        if search:
            cmd.extend(['--search', search])
        
        result = subprocess.run(
            cmd, 
            capture_output=True, 
            text=True, 
            cwd='d:\\\\work\\\\Projects\\\\symgraph'
        )
        
        if result.returncode == 0:
            return json.loads(result.stdout)
        else:
            return {{"error": result.stderr, "code": result.returncode}}
    except Exception as e:
        return {{"error": str(e), "code": 500}}

@app.route('/')
def index():
    # Serve the static HTML file
    try:
        with open('static/index.html', 'r', encoding='utf-8') as f:
            return f.read()
    except FileNotFoundError:
        return """
        <!DOCTYPE html>
        <html>
        <head>
            <title>Error</title>
        </head>
        <body>
            <h1>Error</h1>
            <p>Static files not found. Please ensure the static directory exists with index.html</p>
        </body>
        </html>
        """, 404

@app.route('/static/<path:filename>')
def serve_static(filename):
    """Serve static files"""
    try:
        return send_from_directory('static', filename)
    except FileNotFoundError:
        return "File not found", 404

@app.route('/api/stats')
def get_stats():
    data = call_rust_api('stats', r'{db_path}')
    if 'error' in data:
        return jsonify(data), 500
    return jsonify(data)

@app.route('/api/files')
def get_files():
    search = request.args.get('search', '')
    data = call_rust_api('files', r'{db_path}', search if search else None)
    if 'error' in data:
        return jsonify(data), 500
    return jsonify(data)

@app.route('/api/symbols')
def get_symbols():
    search = request.args.get('search', '')
    data = call_rust_api('symbols', r'{db_path}', search if search else None)
    if 'error' in data:
        return jsonify(data), 500
    return jsonify(data)

@app.route('/api/scip/documents')
def get_scip_documents():
    # For SCIP documents, we'll use files endpoint and filter by SCIP-related categories
    data = call_rust_api('files', r'{db_path}')
    if 'error' in data:
        return jsonify(data), 500
    
    # Filter files that might be SCIP-related (category containing 'scip' or similar)
    scip_files = [
        {{
            'relative_path': f.get('path', ''),
            'language': f.get('language', f.get('lang', '')),
            'category': f.get('category', ''),
            'purpose': f.get('purpose', ''),
            'symbol_count': f.get('symbol_count', 0)
        }}
        for f in data 
        if 'scip' in f.get('category', '').lower() or 'scip' in f.get('purpose', '').lower()
    ]
    
    return jsonify(scip_files)

@app.route('/api/scip/symbols')
def get_scip_symbols():
    search = request.args.get('search', '')
    data = call_rust_api('symbols', r'{db_path}', search if search else None)
    if 'error' in data:
        return jsonify(data), 500
    
    # Transform symbols to SCIP format
    scip_symbols = [
        {{
            'display_name': s.get('name', ''),
            'symbol_kind': s.get('kind', ''),
            'symbol': s.get('name', ''),
            'file_path': s.get('file_id', ''),
            'category': 'scip'
        }}
        for s in data
    ]
    
    return jsonify(scip_symbols)

if __name__ == '__main__':
    app.run(debug=False, port=5000)
'''
        
        # Write the Flask app to a temporary file
        temp_dir = tempfile.mkdtemp()
        app_file = os.path.join(temp_dir, 'symgraph_viewer.py')
        with open(app_file, 'w') as f:
            f.write(app_content)
        
        # Copy static files to temp directory
        static_dir = os.path.join(temp_dir, 'static')
        os.makedirs(static_dir, exist_ok=True)
        
        # Get the path to our static files
        current_dir = os.path.dirname(os.path.abspath(__file__))
        source_static = os.path.join(current_dir, '..', 'static')
        
        # Copy all static files
        if os.path.exists(source_static):
            for item in os.listdir(source_static):
                source_item = os.path.join(source_static, item)
                dest_item = os.path.join(static_dir, item)
                if os.path.isdir(source_item):
                    shutil.copytree(source_item, dest_item, dirs_exist_ok=True)
                else:
                    shutil.copy2(source_item, dest_item)
        
        # Start the Flask server
        self.web_server_process = subprocess.Popen(
            ['python', app_file],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE
        )
    
    def show_statistics(self):
        """Show detailed statistics"""
        db_path = self.current_db_path or self.viewer_db_var.get()
        if not db_path:
            messagebox.showerror("Error", "Please select a database path")
            return
        
        # Check if database exists (Sled databases are directories)
        if not os.path.exists(db_path):
            messagebox.showerror("Error", "Database not found")
            return
        
        # For Sled, check if it's a valid database directory
        if os.path.isdir(db_path):
            if not os.path.exists(os.path.join(db_path, 'db')):
                messagebox.showerror("Error", "Invalid Sled database (missing db file)")
                return
        else:
            messagebox.showerror("Error", "Database appears to be SQLite format (not supported)")
            return
        
        try:
            # Use CLI API to get stats
            result = subprocess.run([
                'cargo', 'run', '--package', 'symgraph-cli', '--', 
                'api', 'stats', '--db', db_path
            ], capture_output=True, text=True, cwd='d:\\work\\Projects\\symgraph')
            
            if result.returncode == 0:
                stats = json.loads(result.stdout)
                file_count = stats.get('files', 0)
                symbol_count = stats.get('symbols', 0)
                edge_count = stats.get('edges', 0)
                
                # Get files using CLI API for categories
                files_result = subprocess.run([
                    'cargo', 'run', '--package', 'symgraph-cli', '--', 
                    'api', 'files', '--db', db_path
                ], capture_output=True, text=True, cwd='d:\\work\\Projects\\symgraph')
                
                # Get file categories
                categories = {}
                if files_result.returncode == 0:
                    files = json.loads(files_result.stdout)
                    for file_info in files:
                        category = file_info.get('category', 'unknown')
                        categories[category] = categories.get(category, 0) + 1
                
                # Create statistics message
                stats_text = f"""
Database Statistics
==================

Files: {file_count}
Symbols: {symbol_count}  
Edges: {edge_count}

File Categories:
"""
                for category, count in sorted(categories.items(), key=lambda x: x[1], reverse=True):
                    stats_text += f"  {category}: {count}\n"
                
                messagebox.showinfo("Database Statistics", stats_text)
            else:
                messagebox.showerror("Error", f"Failed to get statistics: {result.stderr}")
            
        except Exception as e:
            messagebox.showerror("Error", f"Failed to show statistics: {str(e)}")
    
    def clear_settings(self):
        """Clear all settings"""
        if messagebox.askyesno("Confirm", "Are you sure you want to clear all settings?"):
            self.settings = {
                'last_project_dir': '',
                'last_db_path': '',
                'window_geometry': '',
                'auto_open_viewer': True
            }
            self.save_settings()
            messagebox.showinfo("Success", "Settings cleared successfully")

def main():
    root = tk.Tk()
    app = UnifiedSymgraphGUI(root)
    
    def on_closing():
        app.save_settings()
        if app.web_server_process:
            app.web_server_process.terminate()
        root.destroy()
    
    root.protocol("WM_DELETE_WINDOW", on_closing)
    root.mainloop()

if __name__ == "__main__":
    main()
