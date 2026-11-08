//! Windows: embeds the icon and the version details Explorer shows for CVBuilder.exe.
fn main() {
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../shared/assets/cvbuilder.ico")
            .set("ProductName", "CVBuilder")
            .set("FileDescription", "CVBuilder – CV-er som rekrutterere og søknadssystemer leser riktig")
            .set("CompanyName", "CVBuilder")
            .set("LegalCopyright", "MIT OR Apache-2.0");
        if let Err(e) = res.compile() {
            println!("cargo:warning=icon not embedded: {e}");
        }
    }
}
