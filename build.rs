use std::env;

fn main() {
    let slint_theme = env::var("SLINT_THEME").unwrap_or("cosmic-dark".to_string());

    let config =
        slint_build::CompilerConfiguration::new()
        .with_style(slint_theme.into());
        
    slint_build::compile_with_config("ui/main.slint", config).unwrap();
}