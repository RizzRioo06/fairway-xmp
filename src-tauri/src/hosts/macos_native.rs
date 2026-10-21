//! Experimental SMAppService registration. Approval belongs to macOS, not Fairway.
use crate::config::Result;
use objc2::{class, msg_send, rc::Retained, runtime::AnyObject};
use objc2_foundation::{NSError, NSString};
use std::{
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

const LABEL: &str = "dev.fairway.environment.hosts-helper";
pub const APPROVAL_MESSAGE: &str = "Allow Fairway XMP in System Settings → General → Login Items & Extensions, then try address setup again.";
#[link(name = "ServiceManagement", kind = "framework")]
unsafe extern "C" {}

fn service() -> Retained<AnyObject> {
    let plist = NSString::from_str(&format!("{LABEL}.plist"));
    unsafe { msg_send![class!(SMAppService), daemonServiceWithPlistName: &*plist] }
}

fn helper() -> Result<PathBuf> {
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let contents = executable
        .parent()
        .and_then(|p| p.parent())
        .ok_or("Cannot locate app bundle.")?;
    let helper = contents.join("Library/HelperTools").join(LABEL);
    if !helper.is_file()
        || !contents
            .join("Library/LaunchDaemons")
            .join(format!("{LABEL}.plist"))
            .is_file()
    {
        return Err("This prototype needs its bundled signed address helper.".into());
    }
    Ok(helper)
}

// A fresh service can report NotFound before its first registration (observed
// on macOS 27). The bundle payload is checked separately; this status must not
// prevent registration. Enabled/RequiresApproval must retain the user's choice.
fn registration_status(
    before: isize,
    recheck: bool,
    register: impl FnOnce() -> (bool, isize, String),
) -> Result<isize> {
    if !recheck && before != 0 && before != 3 {
        return Ok(before);
    }
    let (success, status, detail) = register();
    // An explicit recheck may return AlreadyRegistered after successfully
    // refreshing an enabled job. Approval still comes exclusively from macOS.
    if !success && status != 2 && !(recheck && status == 1) {
        return Err(format!(
            "Cannot register address helper (status {status}): {detail}"
        ));
    }
    Ok(status)
}

fn register_if_needed(service: &AnyObject, recheck: bool) -> Result<isize> {
    let status: isize = unsafe { msg_send![service, status] };
    registration_status(status, recheck, || {
        let mut error: Option<Retained<NSError>> = None;
        let success: bool = unsafe { msg_send![service, registerAndReturnError: &mut error] };
        // register can return EPERM while successfully recording a daemon
        // that awaits approval. Consult the resulting status even on failure.
        let status = unsafe { msg_send![service, status] };
        let detail = error.map_or_else(
            || "No system error detail.".to_owned(),
            |error| {
                format!(
                    "{} / {}: {} ({})",
                    error.domain(),
                    error.code(),
                    error.localizedDescription(),
                    error
                        .localizedFailureReason()
                        .map_or_else(String::new, |s| s.to_string())
                )
            },
        );
        (success, status, detail)
    })
}

/// Fresh, signed/notarized app smoke check. Never approves the daemon, opens
/// Settings, connects to XPC or writes hosts. Refuses an existing registration.
pub fn test_registration() -> Result<()> {
    helper()?;
    let service = service();
    let before: isize = unsafe { msg_send![&service, status] };
    if before != 0 && before != 3 {
        return Err(format!(
            "Registration smoke check requires a fresh service (status {before})."
        ));
    }
    let outcome = register_if_needed(&service, false).and_then(|status| {
        println!("Address helper registration: {before} -> {status}");
        if status == 1 || status == 2 {
            Ok(())
        } else {
            Err(format!(
                "Address helper registration returned unexpected status {status}."
            ))
        }
    });
    // Even a failed status check must clean up the registration it attempted.
    let mut error: Option<Retained<NSError>> = None;
    let removed: bool = unsafe { msg_send![&service, unregisterAndReturnError: &mut error] };
    if !removed {
        return Err(format!(
            "Registration smoke cleanup failed: {error:?}; result: {outcome:?}"
        ));
    }
    outcome
}

pub fn unregister() -> Result<()> {
    let service = service();
    let status: isize = unsafe { msg_send![&service, status] };
    if status == 0 || status == 3 {
        return Ok(());
    }
    let mut error: Option<Retained<NSError>> = None;
    let success: bool = unsafe { msg_send![&service, unregisterAndReturnError: &mut error] };
    if success {
        Ok(())
    } else {
        Err(format!("Cannot remove address helper: {error:?}"))
    }
}

/// Read-only status check. Never registers the helper or opens Settings.
pub fn approval_enabled() -> bool {
    let service = service();
    let status: isize = unsafe { msg_send![&service, status] };
    status == 1
}

pub fn ensure(names: &[String]) -> Result<super::SetupStatus> {
    ensure_internal(names, true, false)
}

/// Explicit user retry: refresh this app's registration without reopening
/// Settings. Re-registering can restore an approved daemon that macOS has not
/// bootstrapped yet; it never grants approval or unregisters an existing job.
pub fn check_approval(names: &[String]) -> Result<super::SetupStatus> {
    ensure_internal(names, false, true)
}

/// Resume a requested update without registering, reopening Settings or prompting.
pub fn finish_approved(names: &[String]) -> Result<super::SetupStatus> {
    ensure_internal(names, false, false)
}

fn finish_registration(
    status: isize,
    interactive: bool,
    open_settings: impl FnOnce() -> Result<()>,
    apply: impl FnOnce() -> Result<()>,
) -> Result<super::SetupStatus> {
    match status {
        1 => {
            apply()?;
            Ok(super::SetupStatus::Complete)
        }
        2 => {
            if interactive {
                open_settings()?;
            }
            Ok(super::SetupStatus::ApprovalRequired)
        }
        // Approval can be revoked between the monitor check and this call.
        // Background continuation must never register the service again.
        0 | 3 if !interactive => Ok(super::SetupStatus::ApprovalRequired),
        _ => Err(format!(
            "macOS could not enable the address helper (status {status}). Try Set up addresses again."
        )),
    }
}

fn ensure_internal(
    names: &[String],
    interactive: bool,
    recheck: bool,
) -> Result<super::SetupStatus> {
    if names.len() > 1024
        || names
            .iter()
            .any(|name| !crate::config::valid_project_name(name))
    {
        return Err("Invalid site domains or too many sites (maximum 1024).".into());
    }
    if super::current(names)? {
        if names.is_empty() {
            unregister()?;
        }
        return Ok(super::SetupStatus::Complete);
    }
    let helper = helper()?;
    let service = service();
    let status = if interactive || recheck {
        register_if_needed(&service, recheck)?
    } else {
        unsafe { msg_send![&service, status] }
    };
    finish_registration(
        status,
        interactive,
        crate::desktop::authorization::open_login_items,
        || apply(names, helper),
    )
}

fn apply(names: &[String], helper: PathBuf) -> Result<()> {
    let mut child = Command::new(helper)
        .arg("--client")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    child
        .stdin
        .take()
        .ok_or("Cannot send addresses to helper.")?
        .write_all(&serde_json::to_vec(names).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "Address change failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    if !super::current(names)? {
        return Err("The address helper did not update the hosts file.".into());
    }
    if names.is_empty() {
        unregister()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hosts::SetupStatus;
    use std::cell::Cell;

    #[test]
    fn explicit_check_refreshes_registration_and_applies_an_approved_job_without_settings() {
        let attempts = Cell::new(0);
        let writes = Cell::new(0);
        let status = registration_status(2, true, || {
            attempts.set(attempts.get() + 1);
            (true, 1, String::new())
        })
        .unwrap();
        assert_eq!(
            finish_registration(
                status,
                false,
                || panic!("Checking approval reopened Settings"),
                || {
                    writes.set(writes.get() + 1);
                    Ok(())
                },
            )
            .unwrap(),
            SetupStatus::Complete
        );
        assert_eq!(attempts.get(), 1);
        assert_eq!(writes.get(), 1);
        // An existing approved job may return AlreadyRegistered during refresh.
        assert_eq!(
            registration_status(1, true, || (false, 1, "Already registered".into())).unwrap(),
            1
        );
    }

    #[test]
    fn check_never_claims_approval_or_hides_a_real_registration_failure() {
        assert_eq!(
            registration_status(2, false, || panic!("Background observation re-registered"))
                .unwrap(),
            2
        );
        assert_eq!(
            registration_status(2, true, || (false, 2, "Approval required".into())).unwrap(),
            2
        );
        assert!(
            registration_status(3, true, || (false, 0, "Invalid signature".into()))
                .unwrap_err()
                .contains("Invalid signature")
        );
    }

    #[test]
    fn approval_is_pending_then_continues_without_another_settings_handoff() {
        let opened = Cell::new(0);
        let written = Cell::new(0);
        let open = || {
            opened.set(opened.get() + 1);
            Ok(())
        };
        let write = || {
            written.set(written.get() + 1);
            Ok(())
        };
        assert_eq!(
            finish_registration(2, true, open, write).unwrap(),
            SetupStatus::ApprovalRequired
        );
        for _ in 0..4 {
            assert_eq!(
                finish_registration(2, false, open, write).unwrap(),
                SetupStatus::ApprovalRequired
            );
        }
        assert_eq!(opened.get(), 1);
        assert_eq!(written.get(), 0);
        assert_eq!(
            finish_registration(1, false, open, write).unwrap(),
            SetupStatus::Complete
        );
        assert_eq!(opened.get(), 1);
        assert_eq!(written.get(), 1);
    }

    #[test]
    fn revoked_approval_never_registers_or_opens_settings_from_the_background() {
        for status in [0, 2, 3] {
            assert_eq!(
                finish_registration(
                    status,
                    false,
                    || panic!("background task opened Settings"),
                    || panic!("unapproved task wrote hosts"),
                )
                .unwrap(),
                SetupStatus::ApprovalRequired
            );
        }
    }

    #[test]
    fn settings_and_write_failures_remain_errors_rather_than_success_or_pending() {
        assert_eq!(
            finish_registration(2, true, || Err("Settings unavailable".into()), || Ok(())),
            Err("Settings unavailable".into())
        );
        assert_eq!(
            finish_registration(1, false, || Ok(()), || Err("XPC write failed".into())),
            Err("XPC write failed".into())
        );
        assert!(finish_registration(9, false, || Ok(()), || Ok(())).is_err());
    }
}
