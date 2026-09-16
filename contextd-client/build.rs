use std::fs;
use std::path::Path;

fn main() {
    // Copy varlink definitions into contextd-client/src/ during build
    copy_and_generate("../src/rgb/observer.varlink", "src/observer.varlink");
    copy_and_generate("../src/rgb/control.varlink", "src/control.varlink");
    copy_and_generate("../src/contextd.varlink", "src/contextd.varlink");
}

fn copy_and_generate(src: &str, dest: &str) {
    println!("cargo:rerun-if-changed={}", src);
    if Path::new(src).exists() {
        let _ = fs::copy(src, dest);
    }
    varlink_generator::cargo_build_tosource(dest, true);

    let rs_file = dest.replace(".varlink", ".rs");
    if let Ok(content) = fs::read_to_string(&rs_file)
        && !content.contains("#![allow(clippy::all)]")
    {
        let new_content = format!("#![allow(clippy::all)]\n{}", content);
        let _ = fs::write(&rs_file, new_content);
    }
}
