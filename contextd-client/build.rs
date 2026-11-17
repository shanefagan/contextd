// Generates Rust bindings for the contextd Varlink interfaces. The daemon
// depends on this crate for the same generated types, so the .varlink files
// in `interfaces/` are the single source of truth for both sides.
fn main() {
    for file in [
        "interfaces/contextd.varlink",
        "interfaces/observer.varlink",
        "interfaces/control.varlink",
    ] {
        println!("cargo:rerun-if-changed={file}");
        varlink_generator::cargo_build(file);
    }
}
