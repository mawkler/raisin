fn main() {
    const COMPOSITORS: &[&str] = &["hyprland", "niri"];

    let enabled: Vec<&&str> = COMPOSITORS
        .iter()
        .filter(|c| {
            let key = format!("CARGO_FEATURE_{}", c.to_uppercase().replace('-', "_"));
            std::env::var(key).is_ok()
        })
        .collect();

    match enabled.len() {
        0 => {
            eprintln!("error: no compositor selected — enable one of {COMPOSITORS:?}");
            std::process::exit(1);
        }
        1 => {}
        n => {
            eprintln!(
                "error: {n} compositors selected ({enabled:?}) — features are mutually exclusive"
            );
            eprintln!(
                "help: pass '--no-default-features --features <name>' to select a single compositor"
            );
            std::process::exit(1);
        }
    }
}
