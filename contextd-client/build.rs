fn main() {
    println!("cargo:rerun-if-changed=../src/rgb/observer.varlink");
    varlink_generator::cargo_build("../src/rgb/observer.varlink");

    println!("cargo:rerun-if-changed=../src/rgb/control.varlink");
    varlink_generator::cargo_build("../src/rgb/control.varlink");

    println!("cargo:rerun-if-changed=../src/contextd.varlink");
    varlink_generator::cargo_build("../src/contextd.varlink");
}
