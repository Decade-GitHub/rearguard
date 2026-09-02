#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(not(all(target_os = "windows", target_arch = "x86_64", target_env = "msvc")))]
compile_error!("Rearguard supports only the x86_64-pc-windows-msvc target");

mod win32;

use std::env;
use std::path::Path;
use std::process::Command as ProcessCommand;
use std::thread;
use std::time::Duration;

use win32::{Service, ServiceState};

const PROCESS_TARGETS: &[&str] = &[
    "VALORANT-Win64-Shipping.exe",
    "vgc.exe",
    "vgtray.exe",
    "vgm.exe",
    "LeagueClient.exe",
    "GenshinImpact.exe",
    "RobloxPlayerBeta.exe",
    "RobloxPlayerLauncher.exe",
    "RobloxStudioBeta.exe",
    "UmamusumePrettyDerby.exe",
    "Client-Win64-Shipping.exe",
];

const SERVICE_TARGETS: &[&str] = &["vgc", "vgk"];
const SCAN_INTERVAL: Duration = Duration::from_secs(2);
const TASK_NAME: &str = "Windows Host Manager";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommandMode {
    Run,
    Install,
    Uninstall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ServiceDecision {
    Missing,
    DisableOnly,
    DisableAndStop,
    Retry,
}

fn main() {
    if let Err(error) = try_main() {
        report_error(&error);
        std::process::exit(1);
    }
}

fn try_main() -> Result<(), String> {
    match parse_command(env::args().skip(1))? {
        CommandMode::Run => run_forever(),
        CommandMode::Install => configure_startup_task(true),
        CommandMode::Uninstall => configure_startup_task(false),
    }
}

fn parse_command(arguments: impl IntoIterator<Item = String>) -> Result<CommandMode, String> {
    let mut arguments = arguments.into_iter();
    let command = match arguments.next().as_deref() {
        None | Some("run") => CommandMode::Run,
        Some("install") => CommandMode::Install,
        Some("uninstall") => CommandMode::Uninstall,
        Some(command) => return Err(format!("unknown command: {command}")),
    };

    if arguments.next().is_some() {
        return Err("expected at most one command argument".to_string());
    }

    Ok(command)
}

fn run_forever() -> ! {
    loop {
        enforce_processes();
        enforce_services();
        thread::sleep(SCAN_INTERVAL);
    }
}

fn enforce_processes() {
    if let Err(error) = win32::terminate_processes_by_exact_name(PROCESS_TARGETS) {
        report_error(&format!("could not complete process enforcement: {error}"));
    }
}

fn enforce_services() {
    for service_name in SERVICE_TARGETS {
        match enforce_service(service_name) {
            ServiceDecision::Retry => {
                report_error(&format!("will retry service enforcement: {service_name}"))
            }
            ServiceDecision::Missing
            | ServiceDecision::DisableOnly
            | ServiceDecision::DisableAndStop => {}
        }
    }
}

fn enforce_service(service_name: &str) -> ServiceDecision {
    match Service::open(service_name) {
        Ok(Some(service)) => disable_and_stop_service(&service),
        Ok(None) => ServiceDecision::Missing,
        Err(error) => {
            report_error(&format!("could not open {service_name}: {error}"));
            ServiceDecision::Retry
        }
    }
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

    if decision == ServiceDecision::DisableAndStop && service.stop().is_err() {
        return ServiceDecision::Retry;
    }

    decision
}

fn service_decision(state: Option<ServiceState>) -> ServiceDecision {
    match state {
        None => ServiceDecision::Missing,
        Some(ServiceState::Running) => ServiceDecision::DisableAndStop,
        Some(ServiceState::Stopped) => ServiceDecision::DisableOnly,
        Some(ServiceState::Pending) => ServiceDecision::Retry,
    }
}

fn configure_startup_task(install: bool) -> Result<(), String> {
    let arguments = if install {
        let executable = env::current_exe()
            .map_err(|error| format!("could not determine executable path: {error}"))?;
        startup_task_arguments(&executable)
    } else {
        uninstall_task_arguments()
    };

    let status = ProcessCommand::new("schtasks.exe")
        .args(arguments)
        .status()
        .map_err(|error| format!("could not start schtasks.exe: {error}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("schtasks.exe exited with {status}"))
    }
}

fn startup_task_arguments(executable: &Path) -> Vec<String> {
    let task_command = format!(r#""{}" run"#, executable.display());
    vec![
        "/create".to_string(),
        "/tn".to_string(),
        TASK_NAME.to_string(),
        "/tr".to_string(),
        task_command,
        "/sc".to_string(),
        "onlogon".to_string(),
        "/rl".to_string(),
        "highest".to_string(),
        "/f".to_string(),
    ]
}

fn uninstall_task_arguments() -> Vec<String> {
    vec![
        "/delete".to_string(),
        "/tn".to_string(),
        TASK_NAME.to_string(),
        "/f".to_string(),
    ]
}

#[cfg(debug_assertions)]
fn report_error(message: &str) {
    eprintln!("rearguard: {message}");
}

#[cfg(not(debug_assertions))]
fn report_error(_: &str) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_default_and_explicit_run_modes() {
        assert_eq!(parse_command(Vec::new()), Ok(CommandMode::Run));
        assert_eq!(parse_command(vec!["run".to_string()]), Ok(CommandMode::Run));
    }

    #[test]
    fn parses_setup_modes() {
        assert_eq!(
            parse_command(vec!["install".to_string()]),
            Ok(CommandMode::Install)
        );
        assert_eq!(
            parse_command(vec!["uninstall".to_string()]),
            Ok(CommandMode::Uninstall)
        );
    }

    #[test]
    fn rejects_unknown_or_extra_arguments() {
        assert!(parse_command(vec!["status".to_string()]).is_err());
        assert!(parse_command(vec!["run".to_string(), "now".to_string()]).is_err());
    }

    #[test]
    fn decides_how_to_enforce_each_service_state() {
        assert_eq!(service_decision(None), ServiceDecision::Missing);
        assert_eq!(
            service_decision(Some(ServiceState::Running)),
            ServiceDecision::DisableAndStop
        );
        assert_eq!(
            service_decision(Some(ServiceState::Stopped)),
            ServiceDecision::DisableOnly
        );
        assert_eq!(
            service_decision(Some(ServiceState::Pending)),
            ServiceDecision::Retry
        );
    }

    #[test]
    fn includes_the_expected_process_targets() {
        assert!(PROCESS_TARGETS.contains(&"VALORANT-Win64-Shipping.exe"));
        assert!(PROCESS_TARGETS.contains(&"LeagueClient.exe"));
        assert!(PROCESS_TARGETS.contains(&"GenshinImpact.exe"));
    }

    #[test]
    fn builds_the_expected_scheduled_task_arguments() {
        assert_eq!(
            startup_task_arguments(Path::new(r"C:\Program Files\Rearguard\rearguard.exe")),
            vec![
                "/create",
                "/tn",
                "Windows Host Manager",
                "/tr",
                r#""C:\Program Files\Rearguard\rearguard.exe" run"#,
                "/sc",
                "onlogon",
                "/rl",
                "highest",
                "/f",
            ]
        );
        assert_eq!(
            uninstall_task_arguments(),
            vec!["/delete", "/tn", "Windows Host Manager", "/f"]
        );
    }
}
