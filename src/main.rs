fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--headless-demo") {
        let dir = args.get(i + 1).cloned().unwrap_or_else(|| "demo-out".to_string());
        krabba_spelet::game::headless_demo(&dir);
        return;
    }
    krabba_spelet::game::run();
}
