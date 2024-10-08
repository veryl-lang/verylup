use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("target.rs");
    fs::write(
        &dest_path,
        format!("const TARGET: &str = \"{}\";", env::var("TARGET").unwrap()),
    )
    .unwrap();
}
