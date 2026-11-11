fn main() {
    println!("cargo:rerun-if-changed=assets/gavnel.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("assets/gavnel.ico")
            .set("ProductName", "Gavnel")
            .set("CompanyName", "Software by Brox")
            .set("FileDescription", "Gavnel Gameplay Clipper")
            .compile()
            .expect("Could not embed Gavnel's Windows icon");
    }
}
