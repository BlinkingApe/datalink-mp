// Shared by the Helper's and the DLL's build scripts through `include!`, so
// both Windows binaries carry the same VERSIONINFO: the workspace version,
// which a player reads in the file's Properties → Details.

/// The kind of Windows binary the resource is embedded in.
#[allow(dead_code)] // each build script uses one kind
enum WindowsBinary {
    Exe,
    /// A `cdylib`. `embed_resource::compile` links resources into binaries
    /// only, so a DLL needs the variant that reaches every artifact.
    Dll,
}

/// Embeds a VERSIONINFO resource when the target is Windows; does nothing
/// otherwise. The resource is written to `OUT_DIR` from the package version,
/// so it can't drift from the Release version.
fn embed_version_info(binary: WindowsBinary, original_filename: &str, description: &str) {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let env = |key: &str| std::env::var(key).unwrap();
    let version = env("CARGO_PKG_VERSION");
    let numeric = format!(
        "{},{},{},0",
        env("CARGO_PKG_VERSION_MAJOR"),
        env("CARGO_PKG_VERSION_MINOR"),
        env("CARGO_PKG_VERSION_PATCH"),
    );
    // VFT_APP / VFT_DLL; numeric so the script needs no `winver.h`.
    let file_type = match binary {
        WindowsBinary::Exe => 1,
        WindowsBinary::Dll => 2,
    };
    // FILEOS 0x40004 is VOS_NT_WINDOWS32. Translation 0x409, 1200: US
    // English, Unicode, matching the "040904B0" block.
    let rc = format!(
        r#"1 VERSIONINFO
FILEVERSION {numeric}
PRODUCTVERSION {numeric}
FILEOS 0x40004
FILETYPE {file_type}
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904B0"
    BEGIN
      VALUE "FileDescription", "{description}"
      VALUE "FileVersion", "{version}"
      VALUE "OriginalFilename", "{original_filename}"
      VALUE "ProductName", "datalink-mp"
      VALUE "ProductVersion", "{version}"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#
    );
    let rc_path = std::path::Path::new(&env("OUT_DIR")).join("versioninfo.rc");
    std::fs::write(&rc_path, rc).unwrap();

    let result = match binary {
        WindowsBinary::Exe => embed_resource::compile(&rc_path, embed_resource::NONE),
        WindowsBinary::Dll => embed_resource::compile_for_everything(&rc_path, embed_resource::NONE),
    };
    // Required: a release binary without its version is a build failure,
    // not something to discover from a player's Properties dialog.
    result.manifest_required().unwrap();
}
