fn main() {
    varlink_generator::cargo_build_tosource("src/contextd.varlink", true);
    varlink_generator::cargo_build_tosource("src/rgb/observer.varlink", true);
    varlink_generator::cargo_build_tosource("src/rgb/control.varlink", true);
}
