include!("../../build-support/versioninfo.rs");

fn main() {
    // Tell Cargo to re-run if the .def file changes
    println!("cargo:rerun-if-changed=dplayx.def");
    println!("cargo:rerun-if-changed=unwind_stubs.c");
    println!("cargo:rerun-if-changed=../../build-support/versioninfo.rs");

    embed_version_info(WindowsBinary::Dll, "dplayx.dll", "datalink-mp DirectPlay DLL");

    // For Windows targets, use the .def file to control exports and ordinals
    let target = std::env::var("TARGET").unwrap_or_default();
    if target.contains("windows") {
        // Get the manifest directory where dplayx.def is located
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let def_path = format!("{}/dplayx.def", manifest_dir);

        // For MinGW/GCC, the syntax is different from MSVC
        // GCC uses the format: -Wl,path/to/file.def
        println!("cargo:rustc-cdylib-link-arg={}", def_path);

        // For GNU targets, compile unwind stubs to provide missing symbols
        // (the Rust std library references these even with panic=abort)
        if target.contains("gnu") {
            cc::Build::new()
                .file("unwind_stubs.c")
                .compile("unwind_stubs");
        }
    }
}
