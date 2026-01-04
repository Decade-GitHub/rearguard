#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use sysinfo::{System, ProcessesToUpdate};
use std::thread;
use std::time::Duration as StdDuration;
use std::ffi::OsStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

fn main() {
    let mut system = System::new();
    let target_process = OsStr::new("VANGUARD-Win64-Shipping.exe");

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    }).expect("Error setting close handler.");

    while running.load(Ordering::SeqCst) {
        system.refresh_processes(ProcessesToUpdate::All);

        let found = system.processes()
            .iter()
            .find(|(_, process)| process.name() == target_process);

        if let Some((pid, _process)) = found {
            if let Some(proc) = system.process(*pid) {
                proc.kill();
            }   
        }
        thread::sleep(StdDuration::from_secs(2));
    }
}