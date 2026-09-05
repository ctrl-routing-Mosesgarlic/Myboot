//! `myboot config` — show or validate the declarative MyBoot config. The
//! validation here is a light host-side mirror of the in-firmware schema checks
//! (the authoritative parser lives in the `config` crate); this keeps the CLI a
//! std binary with no no_std dependency, while still catching obvious mistakes.
use super::CmdResult;
use std::fs;

const DEFAULT_PATH: &str = "/boot/EFI/MyBoot/config.toml";

pub fn run(args: &[String]) -> CmdResult {
    match args.first().map(String::as_str) {
        Some("show") | None => show(),
        Some("path") => {
            let file = args.get(1).ok_or("usage: myboot config path <file>")?;
            validate(file)
        }
        Some(other) => Err(format!("unknown config subcommand: {other}").into()),
    }
}

fn show() -> CmdResult {
    match fs::read_to_string(DEFAULT_PATH) {
        Ok(text) => { print!("{text}"); Ok(()) }
        Err(_) => Err(format!("could not read {DEFAULT_PATH}").into()),
    }
}

fn validate(file: &str) -> CmdResult {
    let text = fs::read_to_string(file).map_err(|e| format!("read {file}: {e}"))?;
    let mut has_default = false;
    let mut entries = 0usize;
    for (n, raw) in text.lines().enumerate() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() { continue; }
        if let Some(rest) = line.strip_prefix("default") {
            if rest.trim_start().starts_with('=') { has_default = true; }
        }
        if line == "[[entry]]" { entries += 1; }
        // catch an obviously malformed key = value line
        if !line.starts_with('[') && !line.contains('=') {
            return Err(format!("{file}:{}: expected 'key = value'", n + 1).into());
        }
    }
    if !has_default { return Err(format!("{file}: missing required 'default' key").into()); }
    println!("{file}: ok ({entries} entr{})", if entries == 1 { "y" } else { "ies" });
    Ok(())
}
