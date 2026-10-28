#[path = "build_support/ssl_error_tables.rs"]
mod ssl_error_tables;
#[path = "build_support/ssl_version.rs"]
mod ssl_version;

fn main() {
    // openssl-sys publishes the exact include paths used for the linked library.
    // Preprocess protocol flags against those headers, never the build host's
    // unrelated system OpenSSL (also important when cross compiling).
    let includes = std::env::var_os("DEP_OPENSSL_INCLUDE")
        .expect("openssl-sys must publish its include directories");
    cc::Build::new()
        .includes(std::env::split_paths(&includes))
        .file("src/openssl_dynamic.c")
        .flag("-fvisibility=hidden")
        .flag_if_supported("-Wno-deprecated-declarations")
        .compile("twr_openssl_dynamic");

    // Resolve openssl-sys' native link names to forwarding code, not OpenSSL.
    // This directory precedes openssl-sys' library directories at the final link.
    let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    for library in ["ssl", "crypto"] {
        std::fs::copy(
            output.join("libtwr_openssl_dynamic.a"),
            output.join(format!("lib{library}.a")),
        )
        .expect("create OpenSSL forwarding archive");
    }
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        println!("cargo:rustc-link-lib=dl");
    }

    let expanded = cc::Build::new()
        .includes(std::env::split_paths(&includes))
        .file("src/tls_protocols.c")
        .expand();
    let expanded = String::from_utf8(expanded).expect("OpenSSL preprocessor output must be UTF-8");
    let api_version = ssl_version::parse(&expanded).expect("parse compiled OpenSSL API version");
    let versions: Vec<u16> = expanded
        .lines()
        .filter_map(|line| line.trim().strip_prefix("TARANTOOL_TLS_PROTOCOL_"))
        .map(|number| {
            number
                .parse()
                .expect("invalid compiled TLS protocol marker")
        })
        .collect();
    assert!(
        !versions.is_empty(),
        "OpenSSL must enable at least one TLS protocol"
    );
    std::fs::write(
        output.join("ssl_protocols.rs"),
        format!(
            "const COMPILED_PROTOCOLS: &[u16] = &{versions:?};\nconst COMPILED_API_VERSION: u64 = {api_version};\n"
        ),
    )
    .expect("write compiled TLS protocol list");
    error_names(&includes, &output);
    println!("cargo:rerun-if-changed=src/tls_protocols.c");
    println!("cargo:rerun-if-changed=src/openssl_dynamic.c");
    println!("cargo:rerun-if-changed=src/openssl_functions.h");
    println!("cargo:rerun-if-changed=build_support/ssl_version.rs");
    println!("cargo:rerun-if-env-changed=DEP_OPENSSL_INCLUDE");
}

fn error_names(includes: &std::ffi::OsStr, output: &std::path::Path) {
    let expanded = cc::Build::new()
        .includes(std::env::split_paths(includes))
        .file("src/tls_errors.c")
        .expand();
    let expanded = String::from_utf8(expanded).expect("OpenSSL error tables must be UTF-8");
    let ssl_error_tables::ErrorNames { libraries, reasons } =
        ssl_error_tables::parse(&expanded).expect("parse compiled CPython error tables");
    assert!(
        libraries.len() > 20 && reasons.len() > 500,
        "incomplete CPython error tables"
    );
    std::fs::write(
        output.join("ssl_errors.rs"),
        format!(
        "const LIBRARIES: &[(i32, &str)] = &[{}];\nconst REASONS: &[(i32, i32, &str)] = &[{}];\n",
        libraries.join(","), reasons.join(",")
    ),
    )
    .expect("write SSL error names");
    println!("cargo:rerun-if-changed=src/tls_errors.c");
    println!("cargo:rerun-if-changed=build_support/ssl_error_tables.rs");
    println!("cargo:rerun-if-changed=src/ssl_error_data");
}
