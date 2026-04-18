pub fn open_url(url: &str, target: &str) -> Result<(), String> {
    webbrowser::open(url).map_err(|error| format!("Could not open {}: {}", target, error))?;
    Ok(())
}
