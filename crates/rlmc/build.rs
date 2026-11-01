//! Puts the logo into the Windows executable as its icon.

fn main() {
    println!("cargo:rerun-if-changed=../../assets/logo.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../assets/logo.ico");
        res.set("ProductName", "RustLauncherMC");
        res.set("FileDescription", "RustLauncherMC");
        if let Err(e) = res.compile() {
            // A missing resource compiler only costs the icon.
            println!("cargo:warning=could not add the icon: {e}");
        }
    }
}
