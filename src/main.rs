mod conditions;
mod config;
mod display;
mod service;

use std::ffi::{OsString, c_void};
use std::fs::File;
use std::sync::{Arc, Mutex};

use log::*;
use simplelog::*;

use objc2::MainThreadMarker;
use objc2_app_kit::{NSAlert, NSAlertStyle, NSApplication};
use objc2_core_foundation::{
    CFAbsoluteTime, CFAbsoluteTimeGetCurrent, CFIndex, CFRetained, CFRunLoop, CFRunLoopTimer,
    CFRunLoopTimerContext, CFTimeInterval, kCFAllocatorDefault, kCFRunLoopCommonModes,
};
use objc2_foundation::ns_string;

use crate::conditions::{DisplayState, try_execute_command};
use crate::config::{AppConfig, ServiceMode, parse_config, try_get_config_from_common_paths, write_config};
use crate::display::get_display_names;
use crate::service::{is_service_installed, service_install, service_uninstall};

struct AppState {
    previous_displays: Vec<String>,
    config: AppConfig,
    first_timer_iteration: bool,
}

// @src: https://github.com/madsmtm/objc2/blob/master/examples/corefoundation/runloop.rs
unsafe fn create_timer_unchecked<F: Fn(&CFRunLoopTimer) + 'static>(
    fire_date: CFAbsoluteTime,
    interval: CFTimeInterval,
    order: CFIndex,
    callback: F,
) -> CFRetained<CFRunLoopTimer> {
    // We use an `Arc` here to make sure that the reference-counting of the
    // signal container is atomic (`Retained`/`CFRetained` would be valid
    // alternatives too).
    let callback = Arc::new(callback);

    unsafe extern "C-unwind" fn retain<F>(info: *const c_void) -> *const c_void {
        // SAFETY: The pointer was passed to `CFRunLoopTimerContext.info` below.
        unsafe { Arc::increment_strong_count(info.cast::<F>()) };
        info
    }
    unsafe extern "C-unwind" fn release<F>(info: *const c_void) {
        // SAFETY: The pointer was passed to `CFRunLoopTimerContext.info` below.
        unsafe { Arc::decrement_strong_count(info.cast::<F>()) };
    }

    unsafe extern "C-unwind" fn callout<F: Fn(&CFRunLoopTimer)>(
        timer: *mut CFRunLoopTimer,
        info: *mut c_void,
    ) {
        // SAFETY: The timer is valid for at least the duration of the callback.
        let timer = unsafe { &*timer };

        // SAFETY: The pointer was passed to `CFRunLoopTimerContext.info` below.
        let callback = unsafe { &*info.cast::<F>() };

        // Call the provided closure.
        callback(timer);
    }

    // This is marked `mut` to match the signature of `CFRunLoopTimer::new`,
    // but the information is copied, and not actually mutated.
    let mut context = CFRunLoopTimerContext {
        version: 0,
        // This pointer is retained by CF on creation.
        info: Arc::as_ptr(&callback) as *mut c_void,
        retain: Some(retain::<F>),
        release: Some(release::<F>),
        copyDescription: None,
    };

    // SAFETY: The retain/release callbacks are thread-safe, and caller
    // upholds that the main callback is used in a thread-safe manner.
    //
    // `F: 'static`, so extending the lifetime of the closure is fine.
    unsafe {
        CFRunLoopTimer::new(
            kCFAllocatorDefault,
            fire_date,
            interval,
            0, // Documentation says to pass 0 for future compat.
            order,
            Some(callout::<F>),
            &mut context,
        )
    }
    .unwrap()
}

// shows the first run dialog, returns the user's choice as a ServiceMode
fn show_firstrun_dialog() -> ServiceMode {
    unsafe {
        let alert = NSAlert::new(MainThreadMarker::new().unwrap());
        alert.setMessageText(ns_string!("Ergo - First Run"));
        alert.setInformativeText(ns_string!(
            "Would you like to install Ergo as a background service (LaunchAgent)? It will start automatically when you log in."
        ));
        alert.addButtonWithTitle(ns_string!("Yes"));
        alert.addButtonWithTitle(ns_string!("No"));
        alert.setAlertStyle(NSAlertStyle::Informational);
        let response = alert.runModal();
        if response == 1000 {
            info!("user chose to install as service");
            ServiceMode::YesService
        } else {
            info!("user chose not to install as service");
            ServiceMode::NoService
        }
    }
}

fn handle_service_mode(config: &mut AppConfig, no_config_existed: bool) {
    // firstrun: show dialog if no config existed or if firstrun flag is set in config
    if config.firstrun || no_config_existed {
        let choice = show_firstrun_dialog();

        // write the config so we don't ask again
        write_config(&choice);
        config.firstrun = false;

        config.service_mode = Some(choice);
    }

    // now handle the actual service mode
    match &config.service_mode {
        Some(ServiceMode::YesService) => {
            if !is_service_installed() {
                info!("yesservice: installing LaunchAgent");
                service_install();
            } else {
                info!("yesservice: LaunchAgent already installed");
            }
        }
        Some(ServiceMode::NoService) => {
            if is_service_installed() {
                info!("noservice: removing LaunchAgent");
                service_uninstall();
            } else {
                info!("noservice: LaunchAgent not installed, nothing to do");
            }
        }
        None => {}
    }
}

fn main() {
    let _app = NSApplication::sharedApplication(MainThreadMarker::new().unwrap());

    // get config early so we can set log level
    let raw_config = try_get_config_from_common_paths();
    let no_config_existed = raw_config.is_none();
    let mut config = parse_config(&raw_config.unwrap_or_default());

    let log_level = if config.verbose {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };

    CombinedLogger::init(vec![
        TermLogger::new(
            log_level,
            Config::default(),
            TerminalMode::Mixed,
            ColorChoice::Auto,
        ),
        WriteLogger::new(
            log_level,
            Config::default(),
            File::create(format!(
                "/tmp/ergo-log-{}.log",
                users::get_current_username()
                    .unwrap_or(OsString::from("defaultuser"))
                    .to_str()
                    .unwrap()
            ))
            .unwrap(),
        ),
    ])
    .unwrap();

    // handle service install/uninstall/ask
    handle_service_mode(&mut config, no_config_existed);

    let initial_displays = get_display_names();
    info!("initial displays: {:?}", initial_displays);

    let state = Arc::new(Mutex::new(AppState {
        previous_displays: initial_displays,
        config,
        first_timer_iteration: true,
    }));

    let interval = 0.5;
    let timer = unsafe {
        let state = Arc::clone(&state);
        create_timer_unchecked(
            CFAbsoluteTimeGetCurrent() + interval,
            interval,
            0,
            move |_timer| {
                let mut state = state.lock().unwrap();
                let displays = get_display_names();

                // try reparse config on every iteration, in case it changed. This is a bit inefficient but ensures we pick up changes to the config without needing to restart the app
                state.config = parse_config(&try_get_config_from_common_paths().unwrap_or_default());

                let mut added = false;
                let mut removed = false;

                // check for added displays
                for name in displays.iter() {
                    if !state.previous_displays.contains(name) {
                        info!("display added: {}", name);
                        added = true;
                    }
                }

                // check for removed displays
                for name in state.previous_displays.iter() {
                    if !displays.contains(name) {
                        info!("display removed: {}", name);
                        removed = true;
                    }
                }

                // build display state and evaluate rules
                let display_state = DisplayState {
                    current_displays: displays.clone(),
                    added,
                    removed,
                };

                // only execute rules if something changed, otherwise we might execute the same command repeatedly if it fails to change the display state
                if added || removed || state.first_timer_iteration {
                    for (tree, command) in &state.config.rules {
                        try_execute_command(tree, command, &display_state);
                    }
                }

                state.previous_displays = displays;
                state.first_timer_iteration = false;
            },
        )
    };

    let rl: CFRetained<CFRunLoop> = CFRunLoop::main().unwrap();
    rl.add_timer(Some(&timer), unsafe { kCFRunLoopCommonModes });

    CFRunLoop::run();
}
