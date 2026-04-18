use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn main() {
    let task = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Usage: cargo xtask <task>");
        eprintln!();
        eprintln!("Tasks:");
        eprintln!("  www      Build WASM then start Vite dev server");
        eprintln!("  client   Build and run the native desktop client");
        std::process::exit(1);
    });

    let root = workspace_root();

    match task.as_str() {
        "www" => run_www(&root),
        "client" => run_client(&root),
        other => {
            eprintln!("Unknown task: '{other}'");
            eprintln!("Available tasks: www, client");
            std::process::exit(1);
        }
    }
}

fn run_www(root: &Path) {
    let wasm_crate = root.join("crates/w3gpu-wasm");
    let out_dir = root.join("www/pkg");
    let www_dir = root.join("www");

    println!("==> Building WASM package...");
    run(
        Command::new("wasm-pack")
            .args(["build", "--target", "web"])
            .arg("--out-dir")
            .arg(&out_dir)
            .arg(&wasm_crate),
        "wasm-pack build",
    );

    let npm = npm_cmd();

    // Install node_modules if needed
    if !www_dir.join("node_modules").exists() {
        println!("==> Installing npm dependencies...");
        run(Command::new(&npm).arg("install").current_dir(&www_dir), "npm install");
    }

    println!("==> Starting Vite dev server  (http://localhost:5173)");
    run(
        Command::new(&npm).args(["run", "vite"]).current_dir(&www_dir),
        "vite dev",
    );
}

fn run_client(root: &Path) {
    println!("==> Building and running native client...");
    run(
        Command::new("cargo")
            .args(["run", "-p", "native-triangle", "--release"])
            .current_dir(root),
        "cargo run native-triangle",
    );
}

// ── helpers ────────────────────────────────────────────────────────────────

fn workspace_root() -> PathBuf {
    // CARGO_MANIFEST_DIR points to xtask/, parent is workspace root
    let manifest = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    PathBuf::from(manifest)
        .parent()
        .expect("xtask must be inside workspace")
        .to_owned()
}

fn npm_cmd() -> &'static str {
    // prefer npm, works on all platforms
    "npm"
}

fn run(cmd: &mut Command, label: &str) {
    let status = cmd
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .unwrap_or_else(|e| panic!("Failed to launch '{label}': {e}"));

    if !status.success() {
        eprintln!("Command '{label}' exited with {status}");
        std::process::exit(status.code().unwrap_or(1));
    }
}
