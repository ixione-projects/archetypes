use std::{env, path::PathBuf};

extern crate bindgen;

fn main() {
    println!("cargo:rerun-if-changed=bindings.h");
    println!("cargo:rustc-link-search=/usr/lib/x86_64-linux-gnu/");
    println!("cargo:rustc-link-lib=uv");

    let bindings = bindgen::Builder::default()
        .header("bindings.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("could not generate bindings");

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_dir.join("bindings.rs"))
        .expect("could not write bindings");
}
