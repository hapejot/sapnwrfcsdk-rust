use std::{
    env,
    path::{Path, PathBuf},
};

fn main() {
    let sap_dir =
        PathBuf::from(env::var("SAPNWRFCSDK").expect("SAPNWRFCSDK environment variable not set"));

    // Tell Cargo that if the given file changes, to rerun this build script.
    // println!("cargo:rerun-if-changed=src/hello.c");
    // println!("cargo:rustc-link-lib=libsapucum");
    println!("cargo:rustc-link-lib=sapnwrfc");
    // Use the `cc` crate to build a C file and statically link it.
    //     cc::Build::new()
    //         .file("src/hello.c")
    //         .compile("hello");

    let bindings = plattform_defines(&sap_dir)
        .clang_arg("-DSAPwithUNICODE")
        // Tell cargo to invalidate the built crate whenever any of the
        // included header files changed.
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        // Finish the builder and generate the bindings.
        .generate()
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}

#[cfg(target_os = "linux")]
fn plattform_defines(sap_dir: &str) -> bindgen::Builder {
    println!("cargo:rustc-link-search=native={}/lib", sap_dir);
    bindgen::Builder::default().header("{}/include/sapnwrfc.h")
}

#[cfg(target_os = "windows")]
fn plattform_defines(sap_dir: &PathBuf) -> bindgen::Builder {
    println!(
        "cargo:rustc-link-search=native={}",
        sap_dir.join("lib").to_str().unwrap()
    );
    plattform_copy(sap_dir);
    bindgen::Builder::default()
        // The input header we would like to generate
        // bindings for.
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
        .header(sap_dir.join("include").join("sapnwrfc.h").to_str().unwrap())
}

#[cfg(target_os = "linux")]
fn plattform_copy(sap_dir: &str) {}

#[cfg(target_os = "windows")]
fn plattform_copy(sap_dir: &PathBuf) {
    let output_path = get_output_path();
    let dest_path = sap_dir.join("lib");

    copy_dll(&dest_path, "sapnwrfc.dll", &output_path);
    copy_dll(&dest_path, "icudt57.dll", &output_path);
    copy_dll(&dest_path, "icuin57.dll", &output_path);
    copy_dll(&dest_path, "icuuc57.dll", &output_path);
    copy_dll(&dest_path, "libsapucum.dll", &output_path);
}

fn copy_dll(dest_path: &PathBuf, name: &str, output_path: &PathBuf) {
    let src1 = dest_path.join(name);
    let dst1 = output_path.join(name);
    let _ = std::fs::copy(&src1, &dst1).expect(format!("copy {:?} to {:?}", &src1, &dst1).as_str());
}

fn get_output_path() -> PathBuf {
    //<root or manifest path>/target/<profile>/
    // let manifest_dir_string = env::var("CARGO_MANIFEST_DIR").unwrap();
    let build_type = env::var("PROFILE").unwrap();
    let path = Path::new(".")
        .join("target")
        .join(build_type);
    return PathBuf::from(path);
}
