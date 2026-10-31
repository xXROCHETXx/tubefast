fn main() {
    println!("cargo:rerun-if-env-changed=TUBEFAST_OAUTH_CLIENT_FILE");
    println!("cargo:rerun-if-changed=oauth-client.json");
    let config = std::env::var_os("TUBEFAST_OAUTH_CLIENT_FILE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("oauth-client.json"));
    println!("cargo:rerun-if-changed={}", config.display());
    let contents = if config.exists() {
        std::fs::read(config).expect("read the configured OAuth client")
    } else {
        b"{}".to_vec()
    };
    let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo output directory"));
    std::fs::write(output.join("oauth-client.json"), contents).expect("write OAuth configuration");
    println!("cargo:rerun-if-changed=assets/icon.ico");
    #[cfg(windows)]
    winresource::WindowsResource::new()
        .set_icon("assets/icon.ico")
        .compile()
        .expect("the application icon should embed");
}
