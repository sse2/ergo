use std::fs;

use log::*;

pub static SERVICE_LABEL: &str = "cat.fennec.ergo";

pub fn is_service_installed() -> bool {
    let home_dir = std::env::var("HOME").unwrap();
    let plist_path = format!("{}/Library/LaunchAgents/{}.plist", home_dir, SERVICE_LABEL);
    fs::metadata(&plist_path).is_ok()
}

pub fn service_install() {
    let home_dir = std::env::var("HOME").unwrap();
    let plist_dir = format!("{}/Library/LaunchAgents", home_dir);
    let plist_path = format!("{}/{}.plist", plist_dir, SERVICE_LABEL);

    let executable_path = std::env::current_exe().unwrap().display().to_string();

    let plist_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <dict>
        <key>SuccesfulExit</key>
        <false/>
        <key>Crashed</key>
        <true/>
    </dict>
    <true/>
</dict>
</plist>"#,
        SERVICE_LABEL, executable_path
    );

    fs::create_dir_all(&plist_dir).unwrap();
    fs::write(&plist_path, plist_content).unwrap();

    info!("LaunchAgent installed at: {}", plist_path);
}

pub fn service_uninstall() {
    let home_dir = std::env::var("HOME").unwrap();
    let plist_path = format!(
        "{}/Library/LaunchAgents/{}.plist",
        home_dir, SERVICE_LABEL
    );

    if fs::metadata(&plist_path).is_ok() {
        fs::remove_file(&plist_path).unwrap();
        info!("LaunchAgent uninstalled from: {}", plist_path);
    } else {
        info!("LaunchAgent not installed, nothing to remove");
    }
}
