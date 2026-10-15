//! Builds libopus from the vendored sources (`vendor/`, opus 1.5.2) with
//! `cc`, portable C only: no runtime CPU detection, no SSE/AVX/NEON files,
//! float build (`FLOATING_POINT` is the default when `FIXED_POINT` is unset).
//! This keeps cross-compilation (macOS → Windows) free of cmake/ninja and of
//! clang-cl target-feature issues.

use std::path::{Path, PathBuf};

/// Parse `VAR = a \ b \ c` lists from an automake `.mk` file.
fn mk_sources(path: &Path, var: &str) -> Vec<String> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let joined = text.replace("\\\n", " ");
    for line in joined.lines() {
        if let Some(rest) = line.trim().strip_prefix(var) {
            let rest = rest.trim_start();
            if let Some(list) = rest.strip_prefix('=') {
                return list.split_whitespace().map(str::to_string).collect();
            }
        }
    }
    panic!("{var} not found in {}", path.display());
}

fn main() {
    let vendor = PathBuf::from("vendor");
    let mut sources: Vec<String> = Vec::new();
    sources.extend(mk_sources(&vendor.join("celt_sources.mk"), "CELT_SOURCES"));
    sources.extend(mk_sources(&vendor.join("silk_sources.mk"), "SILK_SOURCES"));
    sources.extend(mk_sources(&vendor.join("silk_sources.mk"), "SILK_SOURCES_FLOAT"));
    sources.extend(mk_sources(&vendor.join("opus_sources.mk"), "OPUS_SOURCES"));
    sources.extend(mk_sources(&vendor.join("opus_sources.mk"), "OPUS_SOURCES_FLOAT"));
    // Architecture specific files (cpu detection, SIMD maps) are not vendored.
    sources.retain(|s| !s.contains("/x86/") && !s.contains("/arm/") && !s.contains("/mips/"));

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let mut build = cc::Build::new();
    for s in &sources {
        let p = vendor.join(s);
        assert!(p.exists(), "missing vendored source {}", p.display());
        build.file(p);
    }
    build
        .include(vendor.join("include"))
        .include(vendor.join("celt"))
        .include(vendor.join("silk"))
        .include(vendor.join("silk").join("float"))
        .include(vendor.join("src"))
        .define("OPUS_BUILD", None)
        .define("USE_ALLOCA", None)
        .define("HAVE_LRINT", None)
        .define("HAVE_LRINTF", None)
        .define("PACKAGE_VERSION", Some("\"1.5.2\""))
        .warnings(false)
        .opt_level(3);
    if target_os != "windows" {
        build.define("HAVE_ALLOCA_H", None);
    }
    build.compile("opus");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=vendor/celt_sources.mk");
    println!("cargo:rerun-if-changed=vendor/silk_sources.mk");
    println!("cargo:rerun-if-changed=vendor/opus_sources.mk");
}
