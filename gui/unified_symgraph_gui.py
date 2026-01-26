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
import sqlite3
import webbrowser
from pathlib import Path
from typing import Dict, List, Optional, Tuple
import tempfile

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
            filetypes=[("SQLite Database", "*.db"), ("All Files", "*.*")]
        )
        if filename:
            self.db_path_var.set(filename)
    
    def browse_viewer_db(self):
        """Browse for viewer database file"""
        filename = filedialog.askopenfilename(
            initialdir=os.path.dirname(self.viewer_db_var.get()),
            filetypes=[("SQLite Database", "*.db"), ("All Files", "*.*")]
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
                    cmd_args.extend(['--manifest-path', full_manifest_path])
                
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
                    os.remove(db_path)
                self.log_output(f"Database cleared: {db_path}")
                messagebox.showinfo("Success", "Database cleared successfully")
            except Exception as e:
                messagebox.showerror("Error", f"Failed to clear database: {str(e)}")
    
    def refresh_database_info(self):
        """Refresh database information"""
        db_path = self.viewer_db_var.get()
        if not db_path or not os.path.exists(db_path):
            self.db_info_var.set("Database file not found")
            self.results_tree.delete(*self.results_tree.get_children())
            return
        
        try:
            conn = sqlite3.connect(db_path)
            cursor = conn.cursor()
            
            # Get basic statistics
            cursor.execute("SELECT COUNT(*) FROM files")
            file_count = cursor.fetchone()[0]
            
            cursor.execute("SELECT COUNT(*) FROM symbols")
            symbol_count = cursor.fetchone()[0]
            
            cursor.execute("SELECT COUNT(*) FROM edges")
            edge_count = cursor.fetchone()[0]
            
            self.db_info_var.set(f"Files: {file_count}, Symbols: {symbol_count}, Edges: {edge_count}")
            
            # Populate tree view
            self.results_tree.delete(*self.results_tree.get_children())
            
            # Add file categories
            cursor.execute("""
                SELECT category, COUNT(*) as count 
                FROM files 
                WHERE category IS NOT NULL 
                GROUP BY category 
                ORDER BY count DESC
            """)
            categories = cursor.fetchall()
            
            for category, count in categories:
                category_node = self.results_tree.insert('', 'end', text=f"{category} ({count})", values=('Category', category, '', count))
                
                # Add files in this category
                cursor.execute("""
                    SELECT path, COUNT(s.id) as symbol_count 
                    FROM files f 
                    LEFT JOIN symbols s ON f.id = s.file_id 
                    WHERE f.category = ?
                    GROUP BY f.id 
                    ORDER BY symbol_count DESC 
                    LIMIT 10
                """, (category,))
                
                for file_path, symbol_count in cursor.fetchall():
                    self.results_tree.insert(category_node, 'end', text=os.path.basename(file_path), 
                                           values=('File', os.path.basename(file_path), file_path, symbol_count))
            
            conn.close()
            
        except Exception as e:
            self.db_info_var.set(f"Error reading database: {str(e)}")
    
    def open_web_viewer(self):
        """Open web viewer"""
        db_path = self.current_db_path or self.viewer_db_var.get()
        if not db_path or not os.path.exists(db_path):
            messagebox.showerror("Error", "Please select a valid database file")
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
from flask import Flask, request, jsonify
import sqlite3
import os

app = Flask(__name__)

@app.route('/')
def index():
    return """
<!DOCTYPE html>
<html>
<head>
    <title>Symgraph Viewer</title>
    <style>
        body {{ font-family: Arial, sans-serif; margin: 20px; }}
        .container {{ max-width: 1200px; margin: 0 auto; }}
        .stats {{ background: #f5f5f5; padding: 15px; border-radius: 5px; margin-bottom: 20px; }}
        .section {{ margin-bottom: 30px; }}
        table {{ width: 100%; border-collapse: collapse; }}
        th, td {{ padding: 8px; text-align: left; border-bottom: 1px solid #ddd; }}
        th {{ background-color: #f2f2f2; }}
        .search {{ margin-bottom: 20px; }}
        input[type="text"] {{ padding: 5px; width: 300px; }}
        button {{ padding: 5px 15px; background: #007cba; color: white; border: none; cursor: pointer; }}
    </style>
</head>
<body>
    <div class="container">
        <h1>Symgraph Project Viewer</h1>
        
        <div class="search">
            <input type="text" id="searchInput" placeholder="Search symbols..." onkeyup="searchSymbols()">
            <button onclick="loadData()">Refresh</button>
        </div>
        
        <div id="stats" class="stats">
            <h3>Database Statistics</h3>
            <p>Loading...</p>
        </div>
        
        <div class="section">
            <h3>Files by Category</h3>
            <div id="files">Loading...</div>
        </div>
        
        <div class="section">
            <h3>Symbols</h3>
            <div id="symbols">Loading...</div>
        </div>
    </div>
    
    <script>
        function loadData() {{
            loadStats();
            loadFiles();
            loadSymbols();
        }}
        
        function loadStats() {{
            fetch('/api/stats')
                .then(response => response.json())
                .then(data => {{
                    document.getElementById('stats').innerHTML = `
                        <h3>Database Statistics</h3>
                        <p><strong>Files:</strong> ${{data.files}}</p>
                        <p><strong>Symbols:</strong> ${{data.symbols}}</p>
                        <p><strong>Edges:</strong> ${{data.edges}}</p>
                    `;
                }});
        }}
        
        function loadFiles() {{
            fetch('/api/files')
                .then(response => response.json())
                .then(data => {{
                    let html = '<table><tr><th>Path</th><th>Language</th><th>Category</th><th>Symbols</th></tr>';
                    data.forEach(file => {{
                        html += `<tr><td>${{file.path}}</td><td>${{file.lang}}</td><td>${{file.category}}</td><td>${{file.symbol_count}}</td></tr>`;
                    }});
                    html += '</table>';
                    document.getElementById('files').innerHTML = html;
                }});
        }}
        
        function loadSymbols() {{
            fetch('/api/symbols')
                .then(response => response.json())
                .then(data => {{
                    let html = '<table><tr><th>Name</th><th>Kind</th><th>File</th><th>Category</th></tr>';
                    data.forEach(symbol => {{
                        html += `<tr><td>${{symbol.name}}</td><td>${{symbol.kind}}</td><td>${{symbol.file_path}}</td><td>${{symbol.category}}</td></tr>`;
                    }});
                    html += '</table>';
                    document.getElementById('symbols').innerHTML = html;
                }});
        }}
        
        function searchSymbols() {{
            const query = document.getElementById('searchInput').value;
            fetch(`/api/symbols?search=${{query}}`)
                .then(response => response.json())
                .then(data => {{
                    let html = '<table><tr><th>Name</th><th>Kind</th><th>File</th><th>Category</th></tr>';
                    data.forEach(symbol => {{
                        html += `<tr><td>${{symbol.name}}</td><td>${{symbol.kind}}</td><td>${{symbol.file_path}}</td><td>${{symbol.category}}</td></tr>`;
                    }});
                    html += '</table>';
                    document.getElementById('symbols').innerHTML = html;
                }});
        }}
        
        loadData();
    </script>
</body>
</html>
"""

@app.route('/api/stats')
def get_stats():
    conn = sqlite3.connect(r'{db_path}')
    cursor = conn.cursor()
    
    cursor.execute("SELECT COUNT(*) FROM files")
    files = cursor.fetchone()[0]
    
    cursor.execute("SELECT COUNT(*) FROM symbols")
    symbols = cursor.fetchone()[0]
    
    cursor.execute("SELECT COUNT(*) FROM edges")
    edges = cursor.fetchone()[0]
    
    conn.close()
    
    return jsonify({{'files': files, 'symbols': symbols, 'edges': edges}})

@app.route('/api/files')
def get_files():
    conn = sqlite3.connect(r'{db_path}')
    cursor = conn.cursor()
    
    cursor.execute("""
        SELECT f.path, f.lang, f.category, f.purpose, COUNT(s.id) as symbol_count
        FROM files f
        LEFT JOIN symbols s ON f.id = s.file_id
        GROUP BY f.id
        ORDER BY f.category, f.path
    """)
    files = cursor.fetchall()
    conn.close()
    
    return jsonify([{{'path': f[0], 'lang': f[1], 'category': f[2], 'purpose': f[3], 'symbol_count': f[4]}} for f in files])

@app.route('/api/symbols')
def get_symbols():
    search = request.args.get('search', '')
    
    conn = sqlite3.connect(r'{db_path}')
    cursor = conn.cursor()
    
    if search:
        cursor.execute("""
            SELECT s.name, s.kind, f.path as file_path, f.category, f.purpose
            FROM symbols s
            JOIN files f ON s.file_id = f.id
            WHERE s.name LIKE ?
            ORDER BY s.name
            LIMIT 100
        """, (f'%{{search}}%',))
    else:
        cursor.execute("""
            SELECT s.name, s.kind, f.path as file_path, f.category, f.purpose
            FROM symbols s
            JOIN files f ON s.file_id = f.id
            ORDER BY s.name
            LIMIT 100
        """)
    
    symbols = cursor.fetchall()
    conn.close()
    
    return jsonify([{{'name': s[0], 'kind': s[1], 'file_path': s[2], 'category': s[3], 'purpose': s[4]}} for s in symbols])

if __name__ == '__main__':
    app.run(debug=False, port=5000)
'''
        
        # Write the Flask app to a temporary file
        temp_dir = tempfile.mkdtemp()
        app_file = os.path.join(temp_dir, 'symgraph_viewer.py')
        with open(app_file, 'w') as f:
            f.write(app_content)
        
        # Start the Flask server
        self.web_server_process = subprocess.Popen(
            ['python', app_file],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE
        )
    
    def show_statistics(self):
        """Show detailed statistics"""
        db_path = self.current_db_path or self.viewer_db_var.get()
        if not db_path or not os.path.exists(db_path):
            messagebox.showerror("Error", "Please select a valid database file")
            return
        
        try:
            conn = sqlite3.connect(db_path)
            cursor = conn.cursor()
            
            # Get detailed statistics
            cursor.execute("SELECT COUNT(*) FROM files")
            file_count = cursor.fetchone()[0]
            
            cursor.execute("SELECT COUNT(*) FROM symbols")
            symbol_count = cursor.fetchone()[0]
            
            cursor.execute("SELECT COUNT(*) FROM edges")
            edge_count = cursor.fetchone()[0]
            
            # Get file categories
            cursor.execute("""
                SELECT category, COUNT(*) as count
                FROM files
                WHERE category IS NOT NULL
                GROUP BY category
                ORDER BY count DESC
            """)
            categories = cursor.fetchall()
            
            # Get symbol types
            cursor.execute("""
                SELECT kind, COUNT(*) as count
                FROM symbols
                GROUP BY kind
                ORDER BY count DESC
            """)
            symbol_types = cursor.fetchall()
            
            conn.close()
            
            # Create statistics window
            stats_window = tk.Toplevel(self.root)
            stats_window.title("Database Statistics")
            stats_window.geometry("600x400")
            
            # Create text widget
            text_widget = scrolledtext.ScrolledText(stats_window, wrap='word')
            text_widget.pack(fill='both', expand=True, padx=10, pady=10)
            
            # Add statistics
            stats_text = f"""
DATABASE STATISTICS
==================

Overview:
--------
Files: {file_count}
Symbols: {symbol_count}
Edges: {edge_count}

File Categories:
---------------
"""
            
            for category, count in categories:
                stats_text += f"{category}: {count}\n"
            
            stats_text += "\nSymbol Types:\n-------------\n"
            for kind, count in symbol_types:
                stats_text += f"{kind}: {count}\n"
            
            text_widget.insert('1.0', stats_text)
            text_widget.config(state='disabled')
            
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
