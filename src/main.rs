#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use sysinfo::{System, ProcessesToUpdate};
use std::thread;
use std::time::Duration as StdDuration;
use std::ffi::OsStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

fn main() {
    let mut system = System::new();
    let target_processes = vec![
        "VALORANT-Win64-Shipping.exe",
        "vgc.exe",
        "vgtray.exe",
        "vgm.exe",
        "LeagueClient.exe",
        "GenshinImpact.exe"
        // Add more here
    ];
    
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
        println!("Shutting down gracefully.")
    }).expect("Error setting Ctrl-C handler");

    while running.load(Ordering::SeqCst) {
        system.refresh_processes(ProcessesToUpdate::All);
        
        for target in &target_processes {
            let target_osstr = OsStr::new(target);
            let mut processes = system.processes_by_exact_name(target_osstr);
            
            if let Some(process) = processes.next() {
                    process.kill();
            } 
        }
        thread::sleep(StdDuration::from_secs(2));
    }
}