use std::process::Command;

fn touch(file_name: &str) {
    if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/C", &format!("type nul >> {}", file_name)])
            .output()
            .expect("failed to execute touch");
    } else {
        Command::new("touch")
            .arg(file_name)
            .output()
            .expect("failed to execute touch");
    }
}
