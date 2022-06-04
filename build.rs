use std::env;

fn main() {
    if env::var_os("PROFILE").unwrap() == "release" {
        // Reduce byte alignment for less padding between sections
        println!("cargo:rustc-link-arg-bins=/FILEALIGN:0x200");

        // Prevents unused code from being included
        println!("cargo:rustc-link-arg-bins=/OPT:REF");

        // Drop debug information in PE header
        println!("cargo:rustc-link-arg-bins=/EMITPOGOPHASEINFO");
        println!("cargo:rustc-link-arg-bins=/DEBUG:NONE");

        // Use custom stub
        println!("cargo:rustc-link-arg-bins=/STUB:stub.exe");
    }
}
