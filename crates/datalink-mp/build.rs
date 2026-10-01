include!("../../build-support/versioninfo.rs");

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../build-support/versioninfo.rs");

    embed_version_info(WindowsBinary::Exe, "datalink-mp.exe", "datalink-mp Helper");
    println!("cargo:rustc-env=DATALINK_BUILD_ID={}", build_id());
}

/// The build's ID: the short hash of the commit it was built from, so that
/// two builds of one Release version (two RCs) can be told apart. CI names
/// the commit in `GITHUB_SHA`; elsewhere git is asked. "unknown" without either.
fn build_id() -> String {
    println!("cargo:rerun-if-env-changed=GITHUB_SHA");
    if let Ok(sha) = std::env::var("GITHUB_SHA") {
        if let Some(short) = sha.get(..7) {
            return short.to_string();
        }
    }
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .output()
            .ok()
            .filter(|out| out.status.success())
            .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
    };
    // Build again when another commit is checked out, or the branch moves.
    let mut watched = vec![git(&["rev-parse", "--git-path", "HEAD"])];
    watched.push(git(&["rev-parse", "--git-path", "packed-refs"]));
    if let Some(branch) = git(&["symbolic-ref", "-q", "HEAD"]) {
        watched.push(git(&["rev-parse", "--git-path", &branch]));
    }
    for path in watched.into_iter().flatten() {
        // A path that doesn't exist would make every build run this again.
        if std::path::Path::new(&path).exists() {
            println!("cargo:rerun-if-changed={path}");
        }
    }
    git(&["rev-parse", "--short=7", "HEAD"]).unwrap_or_else(|| "unknown".to_string())
}
