#![allow(dead_code)]
#[path = "../src/logic.rs"]
mod logic;
#[path = "../src/task.rs"]
mod task;
#[path = "../src/win32.rs"]
mod win32;

use logic::{
    AlertKind, CommandError, CommandMode, ServiceDecision, ServiceState, alert_for_scan,
    alert_kind_for_process, parse_command_line, select_alert, service_decision,
};

fn parse(text: &str) -> Result<CommandMode, CommandError> {
    parse_command_line(&text.encode_utf16().collect::<Vec<_>>())
}

#[test]
fn parses_supported_commands_and_windows_quoting() {
    assert_eq!(parse(r#""C:\Program Files\Rearguard\rearguard.exe""#), Ok(CommandMode::Run));
    assert_eq!(parse(r#""C:\Program Files\Rearguard\rearguard.exe" run"#), Ok(CommandMode::Run));
    assert_eq!(parse(r#"rearguard.exe "install""#), Ok(CommandMode::Install));
    assert_eq!(parse(r#"rearguard.exe uninstall"#), Ok(CommandMode::Uninstall));
    assert_eq!(parse("rearguard.exe \t run \t"), Ok(CommandMode::Run));
}

#[test]
fn rejects_unknown_or_extra_commands() {
    assert_eq!(parse(""), Err(CommandError::MissingExecutable));
    assert_eq!(parse("rearguard.exe status"), Err(CommandError::InvalidCommand));
    assert_eq!(parse("rearguard.exe run now"), Err(CommandError::ExtraArgument));
    assert_eq!(parse("rearguard.exe \"run"), Err(CommandError::InvalidCommand));
}

#[test]
fn preserves_process_alert_priority_and_copy() {
    let mut alert = None;
    for name in ["RobloxStudioBeta.exe", "GenshinImpact.exe", "vgc.exe"] {
        alert = select_alert(alert, alert_kind_for_process(name));
    }
    assert_eq!(alert, Some(AlertKind::BetterGames));
    assert_eq!(alert_for_scan(Some(AlertKind::StopSpending), true), alert);
    assert_eq!(alert_for_scan(None, false), None);
    assert_eq!(AlertKind::BetterGames.message(), "Play better games.");
    assert_eq!(AlertKind::StopSpending.message(), "Stop spending money you don't have.");
    assert_eq!(AlertKind::OriginalGames.message(), "Play origianal games.");
}

#[test]
fn preserves_service_decisions() {
    assert_eq!(service_decision(None), ServiceDecision::Missing);
    assert_eq!(service_decision(Some(ServiceState::Running)), ServiceDecision::DisableAndStop);
    assert_eq!(service_decision(Some(ServiceState::Stopped)), ServiceDecision::DisableOnly);
    assert_eq!(service_decision(Some(ServiceState::Pending)), ServiceDecision::Retry);
}

#[test]
fn compares_exact_utf16_process_names() {
    let mut name = [0u16; win32::MAX_PATH];
    let encoded: Vec<_> = "LeagueClient.exe".encode_utf16().collect();
    name[..encoded.len()].copy_from_slice(&encoded);
    assert!(win32::executable_name_matches(&name, "LeagueClient.exe"));
    assert!(!win32::executable_name_matches(&name, "leagueclient.exe"));
    assert!(!win32::executable_name_matches(&name, "LeagueClient"));
}

#[test]
fn checks_ascii_wide_buffers() {
    let mut output = [0u16; 4];
    assert_eq!(win32::encode_ascii_nul("vgc", &mut output).unwrap(), 4);
    assert_eq!(output, [118, 103, 99, 0]);
    assert!(win32::encode_ascii_nul("vgtray", &mut output).is_err());
    assert!(win32::encode_ascii_nul("bad\0", &mut output).is_err());
    assert!(win32::encode_ascii_nul("é", &mut output).is_err());
}

#[test]
fn builds_schtasks_arguments_with_an_embedded_quoted_path() {
    let path: Vec<_> = r"C:\Program Files\Rearguard\rearguard.exe".encode_utf16().collect();
    let mut output = [0u16; 256];
    let length = task::build_schtasks_command_line(true, &path, &mut output).unwrap();
    assert_eq!(
        String::from_utf16(&output[..length]).unwrap(),
        r#"schtasks.exe /create /tn "Windows Host Manager" /tr "\"C:\Program Files\Rearguard\rearguard.exe\" run" /sc onlogon /rl highest /f"#
    );
    assert_eq!(output[length], 0);
    let length = task::build_schtasks_command_line(false, &[], &mut output).unwrap();
    assert_eq!(
        String::from_utf16(&output[..length]).unwrap(),
        r#"schtasks.exe /delete /tn "Windows Host Manager" /f"#
    );
    assert!(task::build_schtasks_command_line(true, &path, &mut [0u16; 16]).is_err());
}

#[test]
fn win32_structs_match_x64_abi() {
    use core::mem::{offset_of, size_of};
    assert_eq!(size_of::<win32::ProcessEntry32W>(), 568);
    assert_eq!(offset_of!(win32::ProcessEntry32W, default_heap_id), 16);
    assert_eq!(offset_of!(win32::ProcessEntry32W, executable_file), 44);
    assert_eq!(size_of::<win32::ServiceStatus>(), 28);
    assert_eq!(size_of::<task::StartupInfoW>(), 104);
    assert_eq!(size_of::<task::ProcessInformation>(), 24);
}

#[test]
fn harmless_win32_queries_succeed() {
    let result = win32::terminate_processes_by_exact_name(&[]);
    assert_eq!(result.alert, None);
    assert!(result.first_error.is_none());
    assert!(matches!(win32::Service::open("__rearguard_test_service_does_not_exist__"), Ok(None)));
}
