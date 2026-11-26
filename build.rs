use std::{
    env,
    path::{Path, PathBuf},
};

fn main() {
    let sap_dir =
        PathBuf::from(env::var("SAPNWRFCSDK").expect("SAPNWRFCSDK environment variable not set"));

    println!("cargo:rustc-link-lib=sapnwrfc");

    let bindings = plattform_defines(&sap_dir)
        .clang_arg("-DSAPwithUNICODE")
        // Libc-Funktionen (memcmp, strlen, ...) aus den Headern werden nicht gebraucht und
        // erzeugen Warnungen ("suspicious runtime symbol"); daher ausschließen.
        .blocklist_function("^(memcmp|memcpy|memmove|memset|strlen)$")
        // Tell cargo to invalidate the built crate whenever any of the
        // included header files changed.
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}

#[cfg(target_os = "linux")]
fn plattform_defines(sap_dir: &Path) -> bindgen::Builder {
    println!(
        "cargo:rustc-link-search=native={}",
        sap_dir.join("lib").display()
    );
    bindgen::Builder::default().header(sap_dir.join("include").join("sapnwrfc.h").to_string_lossy())
}

#[cfg(target_os = "windows")]
fn plattform_defines(sap_dir: &Path) -> bindgen::Builder {
    println!(
        "cargo:rustc-link-search=native={}",
        sap_dir.join("lib").display()
    );
    plattform_copy(sap_dir);
    bindgen::Builder::default()
        .clang_arg("-DSAPonNT")
        .clang_arg("-D_CRT_NON_CONFORMING_SWPRINTFS")
        .clang_arg("-D_CRT_SECURE_NO_DEPRECATE")
        .clang_arg("-D_CRT_NONSTDC_NO_DEPRECATE")
        .clang_arg("-D_AFXDLL")
        .clang_arg("-DWIN32")
        .clang_arg("-D_WIN32_WINNT=0x0502")
        .clang_arg("-D_X86_")
        .clang_arg("-DBCDASM")
        .clang_arg("-DNDEBUG")
        .clang_arg("-DUNICODE")
        .clang_arg("-D_UNICODE")
        .clang_arg("-DSAPwithTHREADS")
        .clang_arg("-D_ATL_ALLOW_CHAR_UNSIGNED")
        .header(sap_dir.join("include").join("sapnwrfc.h").to_string_lossy())
}

#[cfg(target_os = "linux")]
fn plattform_copy(_sap_dir: &Path) {}

#[cfg(target_os = "windows")]
fn plattform_copy(sap_dir: &Path) {
    let output_path = get_output_path();
    let dest_path = sap_dir.join("lib");

    copy_dll(&dest_path, "sapnwrfc.dll", &output_path);
    copy_dll(&dest_path, "icudt57.dll", &output_path);
    copy_dll(&dest_path, "icuin57.dll", &output_path);
    copy_dll(&dest_path, "icuuc57.dll", &output_path);
    copy_dll(&dest_path, "libsapucum.dll", &output_path);
}

#[cfg(target_os = "windows")]
fn copy_dll(dest_path: &Path, name: &str, output_path: &Path) {
    let src = dest_path.join(name);
    assert!(src.exists(), "file {} does not exist", src.display());
    let dst = output_path.join(name);
    assert!(
        output_path.exists(),
        "path {} doesnt exist",
        output_path.display()
    );
    std::fs::copy(&src, &dst)
        .unwrap_or_else(|e| panic!("copy {} to {} failed: {e}", src.display(), dst.display()));
}

#[cfg(target_os = "windows")]
fn get_output_path() -> PathBuf {
    // OUT_DIR is something like: target/<profile>/build/<crate>-<hash>/out
    // We need to go up to target/<profile>
    let out_dir = env::var("OUT_DIR").unwrap();
    Path::new(&out_dir)
        .ancestors()
        .nth(3)
        .expect("Could not find target/<profile> directory")
        .to_path_buf()
}
