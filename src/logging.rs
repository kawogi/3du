// Initialize logging in platform dependant ways.
pub(crate) fn init_logger() {
    // parse_default_env will read the RUST_LOG environment variable and apply it on top
    // of these default filters.
    env_logger::builder()
        .filter_level(log::LevelFilter::Trace)
        // We keep wgpu at Error level, as it's very noisy.
        .filter_module("wgpu_core", log::LevelFilter::Info)
        .filter_module("wgpu_hal", log::LevelFilter::Info)
        .filter_module("naga", log::LevelFilter::Info)
        .filter_module("calloop", log::LevelFilter::Info)
        .parse_default_env()
        .init();
}