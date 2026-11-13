//! On Windows, puts the icon and the version information into `tapiti-desktop.exe`; elsewhere there is nothing to do.

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "windows") {
        winresource::WindowsResource::new()
            .set_icon("assets/icon.ico")
            .set("ProductName", "Tapiti")
            .set("FileDescription", "Tapiti")
            .compile()
            .expect("could not add the icon to the executable");
    }
}
