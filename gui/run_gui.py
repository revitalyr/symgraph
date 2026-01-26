#!/usr/bin/env python3
"""
Symgraph GUI Launcher
Simple launcher for the unified Symgraph GUI
"""
import sys
import os
from pathlib import Path

def main():
    # Add current directory to Python path
    gui_dir = Path(__file__).parent
    sys.path.insert(0, str(gui_dir))
    
    try:
        # Import and run the unified GUI
        from unified_symgraph_gui import main as gui_main
        gui_main()
    except ImportError as e:
        print(f"Error importing GUI: {e}")
        print("Make sure unified_symgraph_gui.py exists in the same directory")
        sys.exit(1)
    except Exception as e:
        print(f"Error running GUI: {e}")
        sys.exit(1)

if __name__ == "__main__":
    main()
