use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let rust_uno_dir = manifest_dir.join("../../rust_uno");

    if !rust_uno_dir.join("Cargo.toml").exists() {
        panic!(
            "rust_uno not found at {}.\n\
             Enter the dev shell (`nix develop`) — it sets up rust_uno automatically.",
            rust_uno_dir.display(),
        );
    }

    println!("cargo:rerun-if-env-changed=RUST_UNO_PATH");
}
