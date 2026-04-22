// Build script to generate Rust code from Varlink interface definitions.
// Generated files are placed in OUT_DIR and included in the source via include!.
fn main() {
    varlink_generator::cargo_build("src/contextd.varlink");
    varlink_generator::cargo_build("src/rgb/observer.varlink");
    varlink_generator::cargo_build("src/rgb/control.varlink");
}
