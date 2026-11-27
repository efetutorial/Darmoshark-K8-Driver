fn main() {
    // Embed the application icon into the Windows executable.
    #[cfg(target_os = "windows")]
    {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/icons/icon.ico");
        resource.compile().expect("Could not embed the Windows icon");
    }
}
