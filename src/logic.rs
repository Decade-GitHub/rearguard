//! Platform-independent decisions. This module uses only `core`.

pub(crate) const PROCESS_TARGETS: &[&str] = &[
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

pub(crate) const SERVICE_TARGETS: &[&str] = &["vgc", "vgk"];
pub(crate) const TASK_NAME: &str = "Windows Host Manager";
pub(crate) const BLOCK_DIALOG_TITLE: &str = "No.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommandMode {
    Run,
    Install,
    Uninstall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommandError {
    MissingExecutable,
    InvalidCommand,
    ExtraArgument,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ServiceState {
    Running,
    Stopped,
    Pending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ServiceDecision {
    Missing,
    DisableOnly,
    DisableAndStop,
    Retry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum AlertKind {
    BetterGames,
    StopSpending,
    OriginalGames,
}

impl AlertKind {
    pub(crate) fn message(self) -> &'static str {
        match self {
            Self::BetterGames => "Play better games.",
            Self::StopSpending => "Stop spending money you don't have.",
            Self::OriginalGames => "Play origianal games.",
        }
    }
}

pub(crate) fn service_decision(state: Option<ServiceState>) -> ServiceDecision {
    match state {
        None => ServiceDecision::Missing,
        Some(ServiceState::Running) => ServiceDecision::DisableAndStop,
        Some(ServiceState::Stopped) => ServiceDecision::DisableOnly,
        Some(ServiceState::Pending) => ServiceDecision::Retry,
    }
}

pub(crate) fn alert_kind_for_process(name: &str) -> AlertKind {
    match name {
        "GenshinImpact.exe" | "UmamusumePrettyDerby.exe" | "Client-Win64-Shipping.exe" => {
            AlertKind::StopSpending
        }
        "RobloxPlayerBeta.exe" | "RobloxPlayerLauncher.exe" | "RobloxStudioBeta.exe" => {
            AlertKind::OriginalGames
        }
        _ => AlertKind::BetterGames,
    }
}

pub(crate) fn select_alert(current: Option<AlertKind>, next: AlertKind) -> Option<AlertKind> {
    Some(match current {
        Some(current) => core::cmp::min(current, next),
        None => next,
    })
}

pub(crate) fn alert_for_scan(process_alert: Option<AlertKind>, service_blocked: bool) -> Option<AlertKind> {
    if service_blocked {
        Some(AlertKind::BetterGames)
    } else {
        process_alert
    }
}

// GetCommandLineW includes the executable name. Its first token follows the
// special Windows argv[0] quoting rule; the only accepted later tokens are
// ASCII command names, optionally surrounded by quotes.
pub(crate) fn parse_command_line(command_line: &[u16]) -> Result<CommandMode, CommandError> {
    let mut index = 0;
    skip_space(command_line, &mut index);
    if index == command_line.len() {
        return Err(CommandError::MissingExecutable);
    }

    let mut quoted = false;
    while index < command_line.len() {
        let character = command_line[index];
        if character == b'"' as u16 {
            quoted = !quoted;
        } else if !quoted && is_space(character) {
            break;
        }
        index += 1;
    }
    if quoted {
        return Err(CommandError::InvalidCommand);
    }
    skip_space(command_line, &mut index);
    if index == command_line.len() {
        return Ok(CommandMode::Run);
    }

    let mut argument = [0u16; 9];
    let mut length = 0;
    while index < command_line.len() {
        let character = command_line[index];
        if character == b'"' as u16 {
            quoted = !quoted;
        } else if !quoted && is_space(character) {
            break;
        } else {
            if length == argument.len() {
                return Err(CommandError::InvalidCommand);
            }
            argument[length] = character;
            length += 1;
        }
        index += 1;
    }
    if quoted {
        return Err(CommandError::InvalidCommand);
    }
    skip_space(command_line, &mut index);
    if index != command_line.len() {
        return Err(CommandError::ExtraArgument);
    }

    match &argument[..length] {
        value if value == ascii_wide(b"run") => Ok(CommandMode::Run),
        value if value == ascii_wide(b"install") => Ok(CommandMode::Install),
        value if value == ascii_wide(b"uninstall") => Ok(CommandMode::Uninstall),
        _ => Err(CommandError::InvalidCommand),
    }
}

fn is_space(character: u16) -> bool {
    character == b' ' as u16 || character == b'\t' as u16
}

fn skip_space(command_line: &[u16], index: &mut usize) {
    while *index < command_line.len() && is_space(command_line[*index]) {
        *index += 1;
    }
}

fn ascii_wide<const N: usize>(bytes: &[u8; N]) -> [u16; N] {
    let mut result = [0; N];
    let mut index = 0;
    while index < N {
        result[index] = bytes[index] as u16;
        index += 1;
    }
    result
}
