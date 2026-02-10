use std::fs;
use std::path::PathBuf;

fn state_file_path() -> Result<PathBuf, String> {
    let home = std::env::var("HOME").map_err(|e| format!("failed to read HOME: {e}"))?;
    Ok(PathBuf::from(home)
        .join("Library")
        .join("Application Support")
        .join("MacNetConfig")
        .join("last_ip.txt"))
}

pub fn load_last_ip() -> Result<Option<String>, String> {
    let path = state_file_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(path).map_err(|e| format!("read last_ip: {e}"))?;
    let ip = content.trim().to_string();
    if ip.is_empty() {
        Ok(None)
    } else {
        Ok(Some(ip))
    }
}

pub fn save_last_ip(ip: &str) -> Result<(), String> {
    let path = state_file_path()?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("create state dir: {e}"))?;
    }
    fs::write(path, ip).map_err(|e| format!("write last_ip: {e}"))?;
    Ok(())
}
