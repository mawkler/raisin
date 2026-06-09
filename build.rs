const COMPOSITORS: &[&str] = &["hyprland", "niri"];

fn main() {
    let compositors_features: Vec<_> = COMPOSITORS
        .iter()
        .map(|compositor| format!("compositor-{compositor}"))
        .collect();

    let enabled: Vec<_> = compositors_features
        .iter()
        .filter(|c| {
            let key = format!("CARGO_FEATURE_{}", c.to_uppercase().replace('-', "_"));
            std::env::var(key).is_ok()
        })
        .collect();

    match enabled.len() {
        0 => {
            let compositors = compositors_features.join(", ");
            eprintln!(
                "error: no compositor selected — enable one of the following features: {compositors}"
            );
            std::process::exit(1);
        }
        1 => {}
        _ => {
            eprintln!(
                "error: more than one compositors selected — features prefixed by `compositor-` are mutually exclusive"
            );
            eprintln!(
                "help: pass '--no-default-features --features <compositor-name>' to select a single compositor"
            );
            std::process::exit(1);
        }
    }
}
