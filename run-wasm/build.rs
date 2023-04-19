use anyhow::*;
use fs_extra::copy_items;
use fs_extra::dir::CopyOptions;
use std::env;
use std::path::Path;

fn main() -> Result<()> {
    // This tells cargo to rerun this script if something in /res/ changes.
    println!("cargo:rerun-if-changed=../wgpu-scene/res/*");

    let out_path = Path::new(&env::var("OUT_DIR")?)
        .join("..")
        .join("..")
        .join("..")
        .join("..")
        .join("wasm-examples")
        .join("wgpu-scene");

    println!("{:?}", out_path);
    let mut copy_options = CopyOptions::new();
    copy_options.overwrite = true;
    let paths_to_copy = vec!["../wgpu-scene/res/"];
    copy_items(&paths_to_copy, out_path, &copy_options)?;

    Ok(())
}
