//! Embeds the app icon and version information in `parley.exe` on Windows (R-NF-1),
//! so Explorer, the taskbar and the Start menu show Parley's icon.

fn main() {
    println!("cargo::rerun-if-changed=icons/parley.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut resource = winresource::WindowsResource::new();
    resource.set_icon("icons/parley.ico");
    resource.set("ProductName", "Parley");
    resource.set("FileDescription", "Parley TN5250 terminal");
    if let Err(e) = resource.compile() {
        // Without the Windows resource compiler the app still builds, just without its icon.
        println!("cargo::warning=could not embed the Windows icon: {e}");
    }
}
