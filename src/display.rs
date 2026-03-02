use objc2::MainThreadMarker;
use objc2_app_kit::NSScreen;

#[allow(unexpected_cfgs)]
pub fn get_display_names() -> Vec<String> {
    let mut display_names = Vec::new();

    let screens = NSScreen::screens(MainThreadMarker::new().unwrap());
    for screen in screens.iter() {
        unsafe {
            display_names.push(screen.localizedName().to_string());
        }
    }

    display_names
}
