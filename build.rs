use std::process::Command;
use std::{env, fs, path::Path};

fn main() {
    let pkg = env!("CARGO_PKG_VERSION");

    let on_tag = Command::new("git")
        .args(["describe", "--tags", "--exact-match", "HEAD"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    let sha = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let version = match (on_tag, sha) {
        (true, _) => pkg.to_string(),
        (false, Some(sha)) => format!("{pkg}-dev+{sha}"),
        (false, None) => format!("{pkg}-dev"),
    };

    let out = Path::new(&env::var("OUT_DIR").unwrap()).join("version.rs");
    fs::write(out, format!("pub const VERSION: &str = \"{version}\";\n")).unwrap();

    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/tags");
}
