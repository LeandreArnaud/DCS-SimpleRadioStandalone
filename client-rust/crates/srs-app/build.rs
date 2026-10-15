//! Produces the four-part version string the SRS server expects
//! (`UpdaterChecker.VERSION` is the entry assembly version, e.g. `2.4.1.0`).
//! Override with the `SRS_VERSION` environment variable in CI.

fn main() {
    println!("cargo:rerun-if-env-changed=SRS_VERSION");
    let version = std::env::var("SRS_VERSION").unwrap_or_else(|_| {
        let pkg = env!("CARGO_PKG_VERSION");
        let parts: Vec<&str> = pkg.split('.').collect();
        match parts.len() {
            3 => format!("{pkg}.0"),
            4 => pkg.to_string(),
            _ => panic!("CARGO_PKG_VERSION must have 3 or 4 parts, got {pkg}"),
        }
    });
    let parts = version.split('.').count();
    assert_eq!(parts, 4, "SRS_VERSION must be a four-part version, got {version}");
    println!("cargo:rustc-env=SRS_CLIENT_VERSION={version}");
}
