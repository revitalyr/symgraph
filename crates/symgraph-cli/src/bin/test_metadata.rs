use cargo_metadata::MetadataCommand;

fn main() {
    let mut cmd = MetadataCommand::new();
    let metadata = cmd.exec().expect("Failed to get metadata");
    
    println!("=== Metadata Structure ===");
    println!("workspace_members: {} items", metadata.workspace_members.len());
    for (i, member) in metadata.workspace_members.iter().enumerate() {
        println!("  [{}]: {}", i, member);
    }
    
    println!("\npackages: {} items", metadata.packages.len());
    for (i, package) in metadata.packages.iter().enumerate() {
        println!("  [{}]: {} ({})", i, package.name, package.id);
    }
    
    if let Some(root) = metadata.root_package() {
        println!("\nroot_package: {} ({})", root.name, root.id);
    } else {
        println!("\nroot_package: None");
    }
    
    println!("\nworkspace_root: {:?}", metadata.workspace_root);
    println!("target_directory: {:?}", metadata.target_directory);
    println!("resolve: {:?}", metadata.resolve.is_some());
}
