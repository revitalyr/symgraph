use cargo_metadata::MetadataCommand;

fn main() {
    let mut cmd = MetadataCommand::new();
    let metadata = cmd.exec().expect("Failed to get metadata");
    
    println!("=== Workspace Members ===");
    println!("workspace_members: {} items", metadata.workspace_members.len());
    
    if metadata.workspace_members.is_empty() {
        println!("  No workspace members (single package)");
    } else {
        for (i, member) in metadata.workspace_members.iter().enumerate() {
            println!("  [{}]: {}", i, member);
            
            // Find corresponding package
            if let Some(package) = metadata.packages.iter().find(|p| &p.id == member) {
                println!("    -> Package: {} ({})", package.name, package.manifest_path);
            } else {
                println!("    -> No package found!");
            }
        }
    }
    
    println!("\n=== Root Package ===");
    if let Some(root) = metadata.root_package() {
        println!("root_package: {} ({})", root.name, root.id);
        println!("  manifest_path: {}", root.manifest_path);
    } else {
        println!("root_package: None");
    }
    
    println!("\n=== Workspace Info ===");
    println!("workspace_root: {:?}", metadata.workspace_root);
    println!("Is workspace: {}", metadata.workspace_members.len() > 1);
}
