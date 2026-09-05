//! xtask — build & verification automation for MyBoot (report §5). A small std
//! binary that encapsulates the exact commands to build the UEFI application,
//! assemble an ESP, run it under QEMU+OVMF, and run the host tests / verifiers.
//! It shells out to cargo, qemu, kani, and tlc so contributors never memorise the
//! flags. One subcommand = one task (SRP).
//!
//! Build/run this tool with an explicit HOST target (the repo default target is
//! the UEFI triple):
//!   cargo run --manifest-path xtask/Cargo.toml --target <host-triple> -- <cmd>

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const UEFI_TARGET: &str = "x86_64-unknown-uefi";
const PURE_CRATES: &[&str] = &[
    "graph", "storage", "ports", "persistence", "config", "policy",
    "health", "discovery", "transaction", "security", "providers",
    "ui-shell", "ui-gfx",
];

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(String::as_str).unwrap_or("help");
    let rest = &args[args.len().min(2)..];
    let release = rest.iter().any(|a| a == "--release");

    let result = match cmd {
        "build" => build(release),
        "esp" => esp(release, rest.get(0).map(PathBuf::from)),
        "run" => run_qemu(release),
        "test" => test_host(),
        "kani" => kani(),
        "tlc" => tlc(),
        "help" | "-h" | "--help" => { help(); Ok(()) }
        other => { eprintln!("unknown task: {other}\n"); help(); Err(format!("unknown task {other}")) }
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => { eprintln!("xtask: {e}"); ExitCode::FAILURE }
    }
}

fn help() {
    println!("xtask — MyBoot build & verification automation\n");
    println!("TASKS:");
    println!("  build [--release]     build the UEFI app (bin/myboot) for {UEFI_TARGET}");
    println!("  esp [DIR] [--release] assemble a bootable ESP tree under DIR (default: target/esp)");
    println!("  run [--release]       build + esp + boot under QEMU with OVMF firmware");
    println!("  test                  run the host unit tests for every pure crate");
    println!("  kani                  run the Kani proofs on the storage parsers");
    println!("  tlc                   model-check the TLA+ boot transaction spec");
    println!("  help                  show this help\n");
    println!("Requires: rustup target add {UEFI_TARGET}; qemu-system-x86_64 + OVMF for `run`.");
}

fn repo_root() -> PathBuf {
    // xtask lives at <root>/xtask; its parent is the repo root.
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().map(Path::to_path_buf).unwrap_or(manifest)
}

fn run(cmd: &mut Command) -> Result<(), String> {
    let shown = format!("{cmd:?}");
    let status = cmd.status().map_err(|e| format!("spawn failed: {e} ({shown})"))?;
    if status.success() { Ok(()) } else { Err(format!("command failed ({}): {shown}", status)) }
}

fn build(release: bool) -> Result<(), String> {
    let root = repo_root();
    let mut c = Command::new("cargo");
    c.current_dir(&root)
     .args(["build", "-p", "myboot", "--target", UEFI_TARGET]);
    if release { c.arg("--release"); }
    run(&mut c)?;
    println!("built: {}", efi_binary(&root, release).display());
    Ok(())
}

fn efi_binary(root: &Path, release: bool) -> PathBuf {
    let profile = if release { "release" } else { "debug" };
    root.join("target").join(UEFI_TARGET).join(profile).join("myboot.efi")
}

fn esp(release: bool, dir: Option<PathBuf>) -> Result<(), String> {
    build(release)?;
    let root = repo_root();
    let esp = dir.unwrap_or_else(|| root.join("target").join("esp"));
    let boot = esp.join("EFI").join("BOOT");
    let cfg_dir = esp.join("EFI").join("MyBoot");
    std::fs::create_dir_all(&boot).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&cfg_dir).map_err(|e| e.to_string())?;

    // Removable-media path so firmware boots it automatically.
    std::fs::copy(efi_binary(&root, release), boot.join("BOOTX64.EFI"))
        .map_err(|e| format!("copy efi: {e}"))?;

    // A sample config so a fresh ESP is usable immediately.
    let sample = "\
# MyBoot configuration
default = \"nixos:gen-current\"
timeout_secs = 5
policy = \"last-good-then-default\"
max_tries = 3
";
    let cfg = cfg_dir.join("config.toml");
    if !cfg.exists() {
        std::fs::write(&cfg, sample).map_err(|e| e.to_string())?;
    }
    println!("assembled ESP at {}", esp.display());
    Ok(())
}

fn run_qemu(release: bool) -> Result<(), String> {
    let root = repo_root();
    esp(release, None)?;
    let esp_dir = root.join("target").join("esp");

    // OVMF firmware images — locations vary by distro; allow env overrides.
    let code = std::env::var("OVMF_CODE").unwrap_or_else(|_| "/usr/share/OVMF/OVMF_CODE.fd".into());
    let vars_src = std::env::var("OVMF_VARS").unwrap_or_else(|_| "/usr/share/OVMF/OVMF_VARS.fd".into());
    // Copy VARS to a writable per-run file so NVRAM writes persist for the session.
    let vars_rw = root.join("target").join("OVMF_VARS.rw.fd");
    std::fs::copy(&vars_src, &vars_rw)
        .map_err(|e| format!("copy OVMF_VARS ({vars_src}): {e} — set OVMF_CODE/OVMF_VARS"))?;

    let mut c = Command::new("qemu-system-x86_64");
    c.args(["-machine", "q35,accel=tcg", "-m", "512M"])
     .args(["-drive", &format!("if=pflash,format=raw,unit=0,readonly=on,file={code}")])
     .args(["-drive", &format!("if=pflash,format=raw,unit=1,file={}", vars_rw.display())])
     .args(["-drive", &format!("format=raw,file=fat:rw:{}", esp_dir.display())])
     .args(["-serial", "stdio"])
     .args(["-net", "none"]);
    println!("launching QEMU (Ctrl-A X to quit)...");
    run(&mut c)
}

fn test_host() -> Result<(), String> {
    // The repo default target is the UEFI triple, which has no test runner, so we
    // run each pure crate's tests against the host target explicitly.
    let root = repo_root();
    let host = host_triple();
    let mut failures = Vec::new();
    for krate in PURE_CRATES {
        let mut c = Command::new("cargo");
        c.current_dir(&root)
         .args(["test", "-p", krate, "--target", &host]);
        if run(&mut c).is_err() { failures.push(*krate); }
    }
    if failures.is_empty() { println!("all {} pure crates passed", PURE_CRATES.len()); Ok(()) }
    else { Err(format!("test failures in: {}", failures.join(", "))) }
}

fn kani() -> Result<(), String> {
    let root = repo_root();
    run(Command::new("cargo").current_dir(&root).args(["kani", "-p", "storage"]))
}

fn tlc() -> Result<(), String> {
    let root = repo_root();
    let spec = root.join("spec").join("tla").join("BootTransaction.tla");
    // Prefer a `tlc` wrapper if present; otherwise the java -jar invocation.
    if which("tlc") {
        run(Command::new("tlc").arg(&spec))
    } else {
        let jar = std::env::var("TLA2TOOLS_JAR")
            .map_err(|_| "set TLA2TOOLS_JAR to tla2tools.jar, or install a `tlc` wrapper".to_string())?;
        run(Command::new("java").args(["-cp", &jar, "tlc2.TLC"]).arg(&spec))
    }
}

fn host_triple() -> String {
    // Allow override; otherwise ask rustc for the host triple.
    if let Ok(t) = std::env::var("HOST_TRIPLE") { return t; }
    let out = Command::new("rustc").arg("-vV").output();
    if let Ok(o) = out {
        for line in String::from_utf8_lossy(&o.stdout).lines() {
            if let Some(h) = line.strip_prefix("host: ") { return h.trim().to_string(); }
        }
    }
    "x86_64-unknown-linux-gnu".to_string()
}

fn which(bin: &str) -> bool {
    Command::new("sh").args(["-c", &format!("command -v {bin}")])
        .status().map(|s| s.success()).unwrap_or(false)
}
