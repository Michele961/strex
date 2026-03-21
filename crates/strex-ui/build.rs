use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=frontend/src");
    println!("cargo:rerun-if-changed=frontend/package.json");
    
    // Skip frontend build in CI (it's built separately before cargo build)
    if std::env::var("CI").is_ok() {
        println!("cargo:warning=CI detected - skipping frontend build (should be built separately)");
        return;
    }
    
    // Only build frontend if we're in development mode or frontend/dist doesn't exist
    let dist_dir = std::path::Path::new("frontend/dist");
    if !dist_dir.exists() || std::env::var("PROFILE").unwrap_or_default() == "debug" {
        println!("cargo:warning=Building frontend assets...");
        
        let status = Command::new("npm")
            .args(["run", "build"])
            .current_dir("frontend")
            .status();
        
        match status {
            Ok(s) if s.success() => {
                println!("cargo:warning=Frontend build completed successfully");
            }
            Ok(s) => {
                panic!("Frontend build failed with exit code: {:?}", s.code());
            }
            Err(e) => {
                eprintln!("Warning: Failed to run npm ({}). Make sure Node.js and npm are installed.", e);
                eprintln!("You may need to run `cd frontend && npm install && npm run build` manually.");
            }
        }
    }
}
