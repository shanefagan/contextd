fn main() {
    varlink_generator::cargo_build_tosource("src/contextd.varlink", true);
    varlink_generator::cargo_build_tosource("src/rgb_observer.varlink", true);
    varlink_generator::cargo_build_tosource("src/rgb_control.varlink", true);
}
