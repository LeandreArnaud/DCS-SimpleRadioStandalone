//! Builds the speexdsp preprocessor (denoise + AGC) from the vendored
//! sources with the floating-point / KISS FFT configuration, the same one the
//! `speexdsp.dll` shipped with the C# client is built with.

fn main() {
    let vendor = std::path::Path::new("vendor");
    // mdf.c (echo canceller) is only there for the `speex_echo_get_residual`
    // symbol preprocess.c references; SRS never enables echo cancellation.
    let srcs = [
        "preprocess.c",
        "filterbank.c",
        "fftwrap.c",
        "kiss_fft.c",
        "kiss_fftr.c",
        "smallft.c",
        "mdf.c",
    ];
    let mut build = cc::Build::new();
    for s in srcs {
        let p = vendor.join("libspeexdsp").join(s);
        println!("cargo:rerun-if-changed={}", p.display());
        build.file(p);
    }
    build
        .include(vendor.join("include"))
        .include(vendor.join("libspeexdsp"))
        .define("FLOATING_POINT", None)
        .define("USE_KISS_FFT", None)
        .define("EXPORT", Some(""))
        .define("RANDOM_PREFIX", Some("srs"))
        .warnings(false)
        .opt_level(2);
    build.compile("speexdsp_preprocess");
    println!("cargo:rerun-if-changed=build.rs");
}
