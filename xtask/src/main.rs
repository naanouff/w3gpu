use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn main() {
    let task = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Usage: cargo xtask <task>");
        eprintln!();
        eprintln!("Tasks:");
        eprintln!("  www          Build WASM then start Vite dev server");
        eprintln!("  client       Build and run the native desktop client");
        eprintln!("  check        cargo check for all targets (native + wasm32)");
        eprintln!("  setup-hooks  Install .githooks/pre-commit into .git/hooks");
        std::process::exit(1);
    });

    let root = workspace_root();

    match task.as_str() {
        "www"         => run_www(&root),
        "client"      => run_client(&root),
        "check"       => run_check(&root),
        "setup-hooks" => setup_hooks(&root),
        other => {
            eprintln!("Unknown task: '{other}'");
            eprintln!("Available tasks: www, client, check, setup-hooks");
            std::process::exit(1);
        }
    }
}

fn run_www(root: &Path) {
    let wasm_crate = root.join("crates/w3drs-wasm");
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

    if !www_dir.join("node_modules").exists() {
        println!("==> Installing npm dependencies...");
        run(Command::new(npm).arg("install").current_dir(&www_dir), "npm install");
    }

    println!("==> Starting Vite dev server  (http://localhost:5173)");
    run(
        Command::new(npm).args(["run", "vite"]).current_dir(&www_dir),
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

fn run_check(root: &Path) {
    println!("==> cargo check — native targets...");
    run(
        Command::new("cargo")
            .args(["check", "--workspace", "--exclude", "w3drs-wasm"])
            .current_dir(root),
        "cargo check native",
    );

    println!("==> cargo check — wasm32 target...");
    run(
        Command::new("cargo")
            .args(["check", "-p", "w3drs-wasm", "--target", "wasm32-unknown-unknown"])
            .current_dir(root),
        "cargo check wasm32",
    );

    println!("==> All checks passed.");
}

fn setup_hooks(root: &Path) {
    let hooks_src = root.join(".githooks").join("pre-commit");
    let hooks_dst = root.join(".git").join("hooks").join("pre-commit");

    if !hooks_src.exists() {
        eprintln!("Hook source not found: {}", hooks_src.display());
        std::process::exit(1);
    }

    std::fs::copy(&hooks_src, &hooks_dst)
        .unwrap_or_else(|e| panic!("Failed to copy hook: {e}"));

    // Make executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&hooks_dst).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&hooks_dst, perms).unwrap();
    }

    println!("==> Installed .githooks/pre-commit → .git/hooks/pre-commit");
    println!("    Run 'cargo xtask check' to test it manually.");
}

// ── helpers ────────────────────────────────────────────────────────────────

fn workspace_root() -> PathBuf {
    let manifest = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    PathBuf::from(manifest)
        .parent()
        .expect("xtask must be inside workspace")
        .to_owned()
}

fn npm_cmd() -> &'static str {
    if cfg!(target_os = "windows") { "npm.cmd" } else { "npm" }
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
