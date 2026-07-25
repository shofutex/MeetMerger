use std::env;
use std::path::PathBuf;
use std::process::Command;

use sha2::{Digest, Sha256};

// Pinned against https://github.com/rsms/inter/releases/download/v4.1/Inter-4.1.zip's
// extras/ttf/Inter-Regular.ttf, so a compromised release asset (or a tampered
// cache) is caught instead of silently getting embedded into the binary.
const EXPECTED_SHA256: &str = "40d692fce188e4471e2b3cba937be967878f631ad3ebbbdcd587687c7ebe0c82";

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let font_path = out_dir.join("Inter-Regular.ttf");

    if !font_path.exists() {
        println!("cargo:warning=Font not found, fetching fonts...");

        #[cfg(unix)]
        let status = Command::new("bash")
            .arg("get-fonts.sh")
            .arg(&out_dir)
            .status()
            .expect("Failed to run get-fonts.sh");

        #[cfg(windows)]
        let status = Command::new("cmd")
            .args(["/C", "get-fonts.bat", out_dir.to_str().unwrap()])
            .status()
            .expect("Failed to run get-fonts.bat");

        if !status.success() {
            panic!("Font fetch script failed");
        }
    }

    let bytes = std::fs::read(&font_path).expect("Failed to read fetched font");
    let actual_sha256 = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    if actual_sha256 != EXPECTED_SHA256 {
        std::fs::remove_file(&font_path).ok();
        panic!(
            "Inter-Regular.ttf sha256 mismatch: expected {EXPECTED_SHA256}, got {actual_sha256}. \
             Refusing to build with an unverified font file."
        );
    }

    println!("cargo:rerun-if-changed=get-fonts.sh");
    println!("cargo:rerun-if-changed=get-fonts.bat");
}
