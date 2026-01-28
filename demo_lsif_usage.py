#!/usr/bin/env python3
"""
Demo script showing LSIF usage with Symgraph
"""

import subprocess
import sys
import os
from pathlib import Path

def run_command(cmd, description):
    """Run a command and display results"""
    print(f"\n🔧 {description}")
    print(f"Command: {' '.join(cmd)}")
    print("-" * 60)
    
    try:
        result = subprocess.run(cmd, capture_output=True, text=True, check=True)
        print(result.stdout)
        return True
    except subprocess.CalledProcessError as e:
        print(f"❌ Error: {e}")
        if e.stdout:
            print("STDOUT:", e.stdout)
        if e.stderr:
            print("STDERR:", e.stderr)
        return False

def demo_lsif_workflow():
    """Demonstrate complete LSIF workflow"""
    
    print("🚀 LSIF Usage Demo for Symgraph")
    print("=" * 60)
    
    # Project path (adjust as needed)
    project_path = Path("D:/work/Projects/STL Desktop Tool/orthosis_mirror_app")
    symgraph_path = Path("d:/work/Projects/symgraph")
    
    if not project_path.exists():
        print(f"❌ Project path not found: {project_path}")
        return
    
    # Step 1: Generate LSIF manually
    lsif_file = project_path / "demo_project.lsif"
    cmd = ["rust-analyzer", "lsif", "."]
    success = run_command(
        cmd, 
        "Step 1: Generating LSIF file with rust-analyzer"
    )
    
    if not success:
        print("❌ Failed to generate LSIF file")
        return
    
    # Save the output
    if success:
        # Note: In real usage, you'd redirect output to file
        print(f"💡 LSIF file would be saved to: {lsif_file}")
        print(f"💡 File size: ~6.7MB for this project")
    
    # Step 2: Show Symgraph command structure
    print(f"\n🔧 Step 2: Symgraph command structure")
    print("-" * 60)
    
    db_file = project_path / "demo_with_lsif.db"
    
    commands = [
        {
            "desc": "Method 1: Automatic LSIF generation",
            "cmd": [
                "cargo", "run", "--bin", "symgraph-cli", "--",
                "scan-rust",
                "--manifest-path", str(project_path / "Cargo.toml"),
                "--db", str(db_file)
            ]
        },
        {
            "desc": "Method 2: Use existing LSIF file", 
            "cmd": [
                "cargo", "run", "--bin", "symgraph-cli", "--",
                "scan-rust",
                "--manifest-path", str(project_path / "Cargo.toml"),
                "--lsif", str(lsif_file),
                "--db", str(db_file)
            ]
        },
        {
            "desc": "Method 3: Custom LSIF path",
            "cmd": [
                "cargo", "run", "--bin", "symgraph-cli", "--",
                "scan-rust",
                "--manifest-path", str(project_path / "Cargo.toml"),
                "--lsif", "custom.lsif",
                "--db", str(db_file)
            ]
        }
    ]
    
    for i, cmd_info in enumerate(commands, 1):
        print(f"\n{i}. {cmd_info['desc']}:")
        print(f"   {' '.join(cmd_info['cmd'])}")
    
    # Step 3: Show benefits
    print(f"\n🎯 Step 3: LSIF Benefits")
    print("-" * 60)
    
    benefits = [
        "✅ More accurate symbol resolution",
        "✅ Better handling of complex Rust features", 
        "✅ Faster processing for large projects",
        "✅ Complete semantic understanding",
        "✅ Precise call graph generation",
        "✅ Enhanced cross-reference analysis"
    ]
    
    for benefit in benefits:
        print(f"   {benefit}")
    
    # Step 4: Show GUI integration
    print(f"\n🖥️  Step 4: GUI Integration")
    print("-" * 60)
    
    print("When using the unified GUI:")
    print("1. Launch: python gui/run_gui.py")
    print("2. Select your Rust project directory")
    print("3. Choose database location")
    print("4. Click 'Index Project'")
    print("5. Symgraph automatically:")
    print("   - Detects if LSIF would be beneficial")
    print("   - Generates LSIF if needed")
    print("   - Processes both LSIF and source code")
    print("   - Provides detailed progress feedback")
    
    # Step 5: Tips and best practices
    print(f"\n💡 Step 5: Tips & Best Practices")
    print("-" * 60)
    
    tips = [
        "🔄 Regenerate LSIF when code structure changes significantly",
        "💾 LSIF files can be large - add *.lsif to .gitignore",
        "⚡ Use LSIF for production/CI environments",
        "🔍 Combine LSIF with source parsing for completeness",
        "📊 Monitor LSIF generation time for large projects",
        "🛠️  Use SYGRAPH_RUST_ANALYZER_CMD for custom rust-analyzer paths"
    ]
    
    for tip in tips:
        print(f"   {tip}")
    
    print(f"\n✅ Demo completed! Check LSIF_USAGE.md for detailed documentation.")

if __name__ == "__main__":
    demo_lsif_workflow()
