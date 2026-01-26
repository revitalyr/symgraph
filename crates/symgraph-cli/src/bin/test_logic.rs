use cargo_metadata::MetadataCommand;

fn main() {
    let mut cmd = MetadataCommand::new();
    let metadata = cmd.exec().expect("Failed to get metadata");
    
    println!("=== Testing Package Logic ===");
    
    // Test the new logic
    let packages = if !metadata.workspace_members.is_empty() {
        println!("Workspace case: {} members", metadata.workspace_members.len());
        metadata.workspace_members
            .iter()
            .filter_map(|id| metadata.packages.iter().find(|p| &p.id == id).cloned())
            .collect()
    } else if let Some(root) = metadata.root_package() {
        println!("Single package case: {}", root.name);
        vec![root.clone()]
    } else {
        panic!("No packages found!");
    };
    
    println!("Total packages to process: {}", packages.len());
    for (i, package) in packages.iter().enumerate() {
        println!("  [{}]: {} ({})", i, package.name, package.id);
    }
    
    println!("\n=== Comparison ===");
    println!("workspace_members: {}", metadata.workspace_members.len());
    println!("root_package: {}", metadata.root_package().is_some());
    println!("packages found: {}", packages.len());
    
    // Check if root package is included in workspace members
    if let Some(root) = metadata.root_package() {
        let root_in_workspace = metadata.workspace_members.contains(&root.id);
        println!("root_package in workspace_members: {}", root_in_workspace);
    }
}
