// Windows'ta sürüm derlemesinde arkada konsol penceresi açılmasını engeller.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    openclass_teacher_lib::run()
}
