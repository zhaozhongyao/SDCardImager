#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    sdcard_image_flasher_lib::run()
}
