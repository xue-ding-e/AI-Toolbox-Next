use std::env;
use std::fs;

fn main() {
    embed_windows_test_manifest();
    tauri_build::build();
    link_windows_unit_test_resource();
}

fn embed_windows_test_manifest() {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR is set by Cargo");
    let manifest_path = std::path::Path::new(&out_dir).join("ai-toolbox-test.manifest");
    fs::write(
        &manifest_path,
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency>
    <dependentAssembly>
      <assemblyIdentity
        type="win32"
        name="Microsoft.Windows.Common-Controls"
        version="6.0.0.0"
        processorArchitecture="*"
        publicKeyToken="6595b64144ccf1df"
        language="*"
      />
    </dependentAssembly>
  </dependency>
</assembly>
"#,
    )
    .expect("write Windows test manifest");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-link-arg-tests=/MANIFEST:EMBED");
    println!(
        "cargo:rustc-link-arg-tests=/MANIFESTINPUT:{}",
        manifest_path.display()
    );
}

fn link_windows_unit_test_resource() {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR is set by Cargo");
    println!("cargo:rustc-link-search=native={out_dir}");
}
