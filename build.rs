use std::process::Command;
use std::path::Path;

fn main() {
    let font_path = "fonts/Inter-Regular.ttf";

    if !Path::new(font_path).exists() {
        println!("cargo:warning=Font not found, fetching fonts...");

        #[cfg(unix)]
        let status = Command::new("bash")
            .arg("get-fonts.sh")
            .status()
            .expect("Failed to run get-fonts.sh");

        #[cfg(windows)]
        let status = Command::new("cmd")
            .args(["/C", "get-fonts.bat"])
            .status()
            .expect("Failed to run get-fonts.bat");

        if !status.success() {
            panic!("Font fetch script failed");
        }
    }

    println!("cargo:rerun-if-changed=fonts/Inter-Regular.ttf");
    println!("cargo:rerun-if-changed=get-fonts.sh");
    println!("cargo:rerun-if-changed=get-fonts.bat");
}
