#![no_std]
#![no_main]

#[cfg(not(all(target_os = "windows", target_arch = "x86_64", target_env = "msvc")))]
compile_error!("Rearguard supports only the x86_64-pc-windows-msvc target");

mod logic;
mod runtime;
mod task;
mod win32;

use core::panic::PanicInfo;
use logic::{
    AlertKind, BLOCK_DIALOG_TITLE, CommandMode, PROCESS_TARGETS, SERVICE_TARGETS, ServiceDecision,
    ServiceState, alert_for_scan, service_decision,
};
use win32::Service;

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    win32::exit(101)
}

#[unsafe(no_mangle)]
pub extern "system" fn rearguard_entry() -> ! {
    let command_line = match win32::command_line() {
        Ok(command_line) => command_line,
        Err(error) => {
            win32::report_error(error);
            win32::exit(1);
        }
    };
    let command = match logic::parse_command_line(command_line) {
        Ok(command) => command,
        Err(_) => {
            win32::report_message("invalid command line");
            win32::exit(1);
        }
    };

    match command {
        CommandMode::Run => run_forever(),
        CommandMode::Install | CommandMode::Uninstall => {
            if let Err(error) = task::configure_startup_task(command == CommandMode::Install) {
                win32::report_error(error);
                win32::exit(1);
            }
            win32::exit(0)
        }
    }
}

fn run_forever() -> ! {
    loop {
        let process_alert = enforce_processes();
        let service_blocked = enforce_services();
        if let Some(alert) = alert_for_scan(process_alert, service_blocked) {
            if let Err(error) =
                win32::show_error_message_box(alert.message(), BLOCK_DIALOG_TITLE)
            {
                win32::report_error(error);
            }
        }
        win32::sleep(2_000);
    }
}

fn enforce_processes() -> Option<AlertKind> {
    let result = win32::terminate_processes_by_exact_name(PROCESS_TARGETS);
    if let Some(error) = result.first_error {
        win32::report_error(error);
    }
    result.alert
}

fn enforce_services() -> bool {
    let mut blocked_any = false;
    for &name in SERVICE_TARGETS {
        let decision = match Service::open(name) {
            Ok(Some(service)) => disable_and_stop_service(&service),
            Ok(None) => ServiceDecision::Missing,
            Err(error) => {
                win32::report_error(error);
                ServiceDecision::Retry
            }
        };
        if decision == ServiceDecision::Retry {
            win32::report_message("will retry service enforcement");
        }
        blocked_any |= decision == ServiceDecision::DisableAndStop;
    }
    blocked_any
}

fn disable_and_stop_service(service: &Service) -> ServiceDecision {
    let state = match service.state() {
        Ok(state) => state,
        Err(_) => return ServiceDecision::Retry,
    };
    let decision = service_decision(Some(state));
    if service.disable().is_err() {
        return ServiceDecision::Retry;
    }
    if state == ServiceState::Running && service.stop().is_err() {
        return ServiceDecision::Retry;
    }
    decision
}
