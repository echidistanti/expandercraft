// Generates the Windows icon resource from the procedural icon in src/icon.rs.

#[path = "src/icon.rs"]
#[allow(dead_code)]
mod icon;

fn main() {
    println!("cargo:rerun-if-changed=src/icon.rs");
    println!("cargo:rerun-if-changed=assets/expandercraft.rc");
    println!("cargo:rerun-if-changed=assets/expandercraft.manifest");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    std::fs::write(out.join("expandercraft.ico"), icon::build_ico(&[16, 24, 32, 48, 64, 256])).unwrap();
    let manifest_dir = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let manifest = std::fs::read_to_string(manifest_dir.join("assets/expandercraft.manifest"))
        .unwrap()
        .replace("@VERSION@", env!("CARGO_PKG_VERSION"));
    std::fs::write(out.join("expandercraft.manifest"), manifest).unwrap();
    let rc = std::fs::read_to_string(manifest_dir.join("assets/expandercraft.rc"))
        .unwrap()
        .replace("@ICON@", &out.join("expandercraft.ico").display().to_string().replace('\\', "/"))
        .replace("@MANIFEST@", &out.join("expandercraft.manifest").display().to_string().replace('\\', "/"))
        .replace("@VERSION@", env!("CARGO_PKG_VERSION"))
        .replace("@VERSION_COMMAS@", &env!("CARGO_PKG_VERSION").replace('.', ","));
    let rc_path = out.join("expandercraft.rc");
    std::fs::write(&rc_path, rc).unwrap();
    embed_resource::compile(&rc_path, embed_resource::NONE).manifest_optional().unwrap();
}
