use std::ffi::OsString;
use std::fs;

use log::*;

use crate::conditions::{ConditionNode, parse_rule};

#[derive(Debug, Clone, PartialEq)]
pub enum ServiceMode {
    YesService,
    NoService,
}

#[derive(Debug)]
pub struct AppConfig {
    pub verbose: bool,
    pub firstrun: bool,
    pub service_mode: Option<ServiceMode>,
    pub rules: Vec<(ConditionNode, String)>,
}

// returns the path to the ergorc config directory
pub fn get_config_dir() -> String {
    std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| {
        let home: String = std::env::var("HOME").unwrap_or_else(|_| {
            format!(
                "/Users/{}",
                users::get_current_username()
                    .unwrap_or(OsString::from("defaultuser"))
                    .to_str()
                    .unwrap()
            )
            .to_string()
        });
        format!("{}/.config", home).to_string()
    })
}

pub fn get_config_path() -> String {
    format!("{}/ergorc", get_config_dir())
}

// gets config file from paths, returns None if no config file exists
pub fn try_get_config_from_common_paths() -> Option<String> {
    let ergorc = get_config_path();
    info!("looking for ergorc at: {}", ergorc);

    if fs::metadata(&ergorc).is_ok() {
        info!("found ergorc at: {}", ergorc);
        Some(fs::read_to_string(ergorc).unwrap_or_else(|_| "verbose".to_string()))
    } else {
        info!("no ergorc found, first run");
        None
    }
}

// writes a config file with the given service mode
pub fn write_config(service_mode: &ServiceMode) {
    let config_dir = get_config_dir();
    let config_path = get_config_path();

    fs::create_dir_all(&config_dir).unwrap();

    let service_cmd = match service_mode {
        ServiceMode::YesService => "yesservice",
        ServiceMode::NoService => "noservice",
    };

    let content = format!(
        "# ergo config\n# please see https://github.com/sse2/ergo for documentation\n{}\n",
        service_cmd
    );

    fs::write(&config_path, content).unwrap();
    info!("wrote config to: {}", config_path);
}

// either commands or rules, separated by newlines
// rules are handled in conditions.rs, commands are as follows
// verbose - extra logging
// firstrun - show the first run dialog again
// yesservice - always tries to install as service
// noservice - removes service if possible
pub fn parse_config(raw: &str) -> AppConfig {
    let mut config = AppConfig {
        verbose: false,
        firstrun: false,
        service_mode: None,
        rules: vec![],
    };

    for line in raw.lines() {
        let trimmed = line.trim();

        // skip empty lines and comments
        if trimmed.is_empty() || trimmed.starts_with("#") {
            continue;
        }

        // handle commands
        match trimmed {
            "verbose" => {
                config.verbose = true;
                info!("verbose mode enabled");
                continue;
            }
            "firstrun" => {
                config.firstrun = true;
                info!("firstrun flag set");
                continue;
            }
            "yesservice" => {
                config.service_mode = Some(ServiceMode::YesService);
                info!("service mode: yesservice");
                continue;
            }
            "noservice" => {
                config.service_mode = Some(ServiceMode::NoService);
                info!("service mode: noservice");
                continue;
            }
            _ => {}
        }

        // try to parse as a rule
        match parse_rule(trimmed.to_string()) {
            Ok((node, cmd)) => {
                if !cmd.is_empty() {
                    config.rules.push((node, cmd));
                }
            }
            Err(e) => {
                warn!("skipping invalid rule '{}': {}", trimmed, e);
            }
        }
    }

    info!(
        "config loaded: verbose={}, firstrun={}, service_mode={:?}, {} rule(s)",
        config.verbose,
        config.firstrun,
        config.service_mode,
        config.rules.len()
    );

    config
}
