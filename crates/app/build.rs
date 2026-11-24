fn main() {
    println!("cargo:rerun-if-changed=../../packaging/windows/Ivy.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        windows_resources();
    }
}

fn windows_resources() {
    let version = std::env::var("CARGO_PKG_VERSION").expect("package version");
    let part = |name| -> u64 {
        std::env::var(name)
            .expect("version component")
            .parse::<u16>()
            .expect("Windows version components must fit in 16 bits") as u64
    };
    let numeric = (part("CARGO_PKG_VERSION_MAJOR") << 48)
        | (part("CARGO_PKG_VERSION_MINOR") << 32)
        | (part("CARGO_PKG_VERSION_PATCH") << 16);
    let mut resource = winresource::WindowsResource::new();
    resource
        .set_icon("../../packaging/windows/Ivy.ico")
        .set("ProductName", "Ivy")
        .set("FileDescription", "Ivy")
        .set("FileVersion", &version)
        .set("ProductVersion", &version)
        .set_version_info(winresource::VersionInfo::FILEVERSION, numeric)
        .set_version_info(winresource::VersionInfo::PRODUCTVERSION, numeric);
    resource.compile().expect("compile Ivy Windows resources");
}
