#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use sysinfo::{System, ProcessesToUpdate};
use std::thread;
use std::time::Duration as StdDuration;
use std::ffi::OsStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::collections::HashMap;
use windows::core::*;
use windows::Win32::System::Services::*;

fn stop_windows_service(service_name: &str) -> Result<bool> {
    unsafe {
        let scm = OpenSCManagerW(
            None,
            None,
            SC_MANAGER_CONNECT,
        )?;
        
        let service = match OpenServiceW(
            scm,
            &HSTRING::from(service_name),
            SERVICE_STOP | SERVICE_QUERY_STATUS,
        ) {
            Ok(svc) => svc,
            Err(_) => {
                let _ = CloseServiceHandle(scm);
                return Ok(false); // Service doesn't exist
            }
        };

        let mut status = SERVICE_STATUS::default();
        let _ = QueryServiceStatus(service, &mut status);

        let was_stopped = if status.dwCurrentState == SERVICE_RUNNING {
            match ControlService(service, SERVICE_CONTROL_STOP, &mut status) {
                Ok(_) => true,
                Err(_) => false, // Already stopped or fatal error
            }
        }
        else {
            false // Was never running
        };

        let _ = CloseServiceHandle(service);
        let _ = CloseServiceHandle(scm);

        Ok(was_stopped)
    }
}

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
    
    let target_services = vec![
        "vgc",
        "vgk"
    ];

    let mut services_stopped: HashMap<String, bool> = HashMap::new();

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

        for service_name in &target_services {
            if !services_stopped.contains_key(*service_name) {
                match stop_windows_service(service_name) {
                    Ok(stopped) => {
                        services_stopped.insert(service_name.to_string(), stopped);
                    }
                    Err(_) => {
                        // Something went wrong, retry on next loop
                        println!("Fatal error. Retrying...");
                    }
                }
            }
        }
        thread::sleep(StdDuration::from_secs(2));
    }
}