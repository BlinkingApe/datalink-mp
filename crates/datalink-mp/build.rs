include!("../../build-support/versioninfo.rs");

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../build-support/versioninfo.rs");

    embed_version_info(WindowsBinary::Exe, "datalink-mp.exe", "datalink-mp Helper");
}
