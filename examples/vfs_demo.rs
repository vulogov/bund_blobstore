use bund_blobstore::common::{VfsNodeType, VirtualFilesystem};
use bund_blobstore::{DataDistributionManager, DistributionStrategy};
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // 1. Initialize the backend manager
    let base_path = "./vfs_storage";

    // Updated to use RoundRobin since it doesn't require an additional config struct
    let strategy = DistributionStrategy::RoundRobin;

    let manager = Arc::new(DataDistributionManager::new(base_path, strategy)?);
    let vfs = VirtualFilesystem::new(manager);

    println!("--- Initializing Project Structure ---");

    // 2. Create a directory tree
    vfs.mkdir("/deployments")?;
    vfs.mkdir("/deployments/production")?;

    // 3. Store documents
    vfs.mktext(
        "/deployments/production/notes.txt",
        "Deploying version 1.2.4 to the main cluster.",
        "en-GB",
    )?;

    let rust_code = "fn main() { println!(\"Hello World\"); }";
    vfs.script("/deployments/production/init.rs", rust_code, "rust")?;

    vfs.mkjson(
        "/deployments/production/config.json",
        "doc_550e8400",
        "fp_a7b2",
        "v2",
        1024,
    )?;

    println!("Success: Files created.");

    // 4. List and Inspect Metadata
    println!("\n--- Inspecting /deployments/production ---");
    let files = vfs.ls("/deployments/production")?;
    for file_name in files {
        let path = format!("/deployments/production/{}", file_name);
        let node = vfs.get_node_by_path(&path)?;

        print!("- Found: {:<12} | ID: {:<38}", node.name, node.id);

        match node.node_type {
            VfsNodeType::CodeDocument {
                lines,
                ref functions,
                ..
            } => {
                println!(" | Code: {} lines, fns: {:?}", lines, functions);
            }
            VfsNodeType::TextDocument { word_count, .. } => {
                println!(" | Text: {} words", word_count);
            }
            _ => println!(" | Type: Other"),
        }
    }

    // 5. Demonstrate Safety Guards
    println!("\n--- Safety Guard Test ---");
    let result = vfs.mkdir("/deployments/production/notes.txt/nested_dir");
    if result.is_err() {
        println!("Correctly blocked: Cannot create directory inside a file.");
    }

    Ok(())
}
