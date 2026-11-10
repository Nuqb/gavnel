//! A separate copy waits for graceful shutdown before running Inno Setup.
use super::*;
use serde::{Deserialize, Serialize};
use std::os::windows::process::CommandExt;
use std::process::Command;
use windows::Win32::System::Threading::*;

#[derive(Serialize, Deserialize)]
struct Plan {
    parent: u32,
    target: PathBuf,
    installer: PathBuf,
    digest: String,
    size: u64,
    version: String,
}

pub(super) fn start(prepared: &Prepared) -> Result<()> {
    let target = std::env::current_exe()?.canonicalize()?;
    ensure!(
        managed_executable(&target),
        "In-app installation is available in installed release builds. Use the source launcher for this checkout."
    );
    download::verify(
        &prepared.path,
        &prepared.release.digest,
        prepared.release.size,
    )?;
    let directory = prepared.path.parent().context("Missing update directory")?;
    let helper = directory.join("Gavnel-Update-Helper.exe");
    std::fs::copy(&target, &helper)?;
    let plan = Plan {
        parent: std::process::id(),
        target,
        installer: prepared.path.clone(),
        digest: prepared.release.digest.clone(),
        size: prepared.release.size,
        version: prepared.release.version.to_string(),
    };
    let path = directory.join("install.json");
    std::fs::write(&path, serde_json::to_vec(&plan)?)?;
    Command::new(helper)
        .arg("--apply-release-update")
        .arg(path)
        .creation_flags(CREATE_NO_WINDOW.0)
        .spawn()
        .context("Could not start the update helper")?;
    Ok(())
}

pub fn early(args: &[String]) -> Result<bool> {
    if args
        .get(1)
        .is_none_or(|arg| arg != "--apply-release-update")
    {
        return Ok(false);
    }
    let result = (|| -> Result<()> {
        ensure!(args.len() == 3, "Invalid update helper arguments");
        let bytes = std::fs::read(&args[2])?;
        ensure!(bytes.len() < 65536, "Invalid update plan");
        let plan: Plan = serde_json::from_slice(&bytes)?;
        apply(&plan)
    })();
    if let Err(error) = result {
        report(&format!(
            "Gavnel could not finish updating. Your recordings and preferences were not removed.\n\n{error:#}"
        ));
    }
    Ok(true)
}

fn apply(plan: &Plan) -> Result<()> {
    if let Err(error) = install_after_exit(plan) {
        if managed_executable(&plan.target) {
            let _ = Command::new(&plan.target).spawn();
        }
        return Err(error);
    }
    Command::new(&plan.target).spawn()?;
    let _ = std::fs::remove_file(&plan.installer);
    Ok(())
}

fn install_after_exit(plan: &Plan) -> Result<()> {
    ensure!(
        managed_executable(&plan.target),
        "The update target is not a Gavnel installation"
    );
    wait_for_parent(plan)?;
    download::verify(&plan.installer, &plan.digest, plan.size)?;
    let directory = plan
        .target
        .parent()
        .context("Missing installation directory")?;
    let status = installer_command(&plan.installer, directory)
        .arg(format!(
            "/LOG={}",
            plan.installer.with_extension("log").display()
        ))
        .status()
        .context("Could not run the update installer")?;
    ensure!(
        status.success(),
        "The installer did not complete (exit {:?}). The previous installation was kept where possible.",
        status.code()
    );
    let output = Command::new(&plan.target)
        .arg("--version")
        .creation_flags(CREATE_NO_WINDOW.0)
        .output()?;
    ensure!(
        output.status.success()
            && String::from_utf8_lossy(&output.stdout).trim() == format!("Gavnel {}", plan.version),
        "Installed version verification failed; open Gavnel from the Start menu and check its version"
    );
    Ok(())
}

fn installer_command(installer: &Path, directory: &Path) -> Command {
    let mut command = Command::new(installer);
    command
        .args([
            "/VERYSILENT",
            "/SUPPRESSMSGBOXES",
            "/NORESTART",
            "/CURRENTUSER",
            "/NOCLOSEAPPLICATIONS",
            "/NOFORCECLOSEAPPLICATIONS",
            "/NOLAUNCH=1",
            "/NOSHORTCUTS=1",
            "/NOREGISTRY=1",
        ])
        .arg(format!("/DIR={}", installer_path(directory)))
        .creation_flags(CREATE_NO_WINDOW.0);
    command
}

// Inno Setup rejects the extended-length prefix returned by canonicalize().
fn installer_path(path: &Path) -> String {
    let text = path.to_string_lossy();
    if let Some(unc) = text.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{unc}")
    } else {
        text.strip_prefix(r"\\?\").unwrap_or(&text).to_owned()
    }
}

fn wait_for_parent(plan: &Plan) -> Result<()> {
    use windows::Win32::Foundation::{CloseHandle, ERROR_INVALID_PARAMETER, WAIT_OBJECT_0};
    unsafe {
        let process = match OpenProcess(
            PROCESS_ACCESS_RIGHTS(SYNCHRONIZATION_SYNCHRONIZE.0)
                | PROCESS_QUERY_LIMITED_INFORMATION,
            false,
            plan.parent,
        ) {
            Ok(process) => process,
            Err(error)
                if error.code()
                    == windows::core::HRESULT::from_win32(ERROR_INVALID_PARAMETER.0) =>
            {
                return Ok(());
            }
            Err(error) => return Err(error.into()),
        };
        let result = (|| -> Result<()> {
            let mut buffer = vec![0_u16; 32768];
            let mut length = buffer.len() as u32;
            QueryFullProcessImageNameW(
                process,
                PROCESS_NAME_WIN32,
                windows::core::PWSTR(buffer.as_mut_ptr()),
                &mut length,
            )?;
            let path =
                PathBuf::from(String::from_utf16(&buffer[..length as usize])?).canonicalize()?;
            ensure!(
                path == plan.target,
                "The waiting process does not match the installation"
            );
            ensure!(
                WaitForSingleObject(process, 10 * 60 * 1000) == WAIT_OBJECT_0,
                "Gavnel is still saving or another exit error occurred. No files were replaced; try updating later."
            );
            Ok(())
        })();
        let _ = CloseHandle(process);
        result
    }
}

fn report(message: &str) {
    let text: Vec<u16> = message.encode_utf16().chain(Some(0)).collect();
    unsafe {
        windows::Win32::UI::WindowsAndMessaging::MessageBoxW(
            None,
            windows::core::PCWSTR(text.as_ptr()),
            windows::core::w!("Gavnel update"),
            windows::Win32::UI::WindowsAndMessaging::MB_OK
                | windows::Win32::UI::WindowsAndMessaging::MB_ICONERROR,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    #[test]
    fn installer_accepts_canonical_windows_paths_with_spaces() {
        assert_eq!(
            installer_path(Path::new(r"\\?\C:\Users\A User\Programs\Gavnel")),
            r"C:\Users\A User\Programs\Gavnel"
        );
        assert_eq!(
            installer_path(Path::new(r"\\?\UNC\server\share\Gavnel")),
            r"\\server\share\Gavnel"
        );
        assert_eq!(
            installer_path(Path::new(r"C:\Apps\Gavnel")),
            r"C:\Apps\Gavnel"
        );
    }

    #[test]
    #[ignore = "child process fixture for the installer integration test"]
    fn update_parent_fixture() {
        std::thread::sleep(std::time::Duration::from_millis(900));
    }

    #[test]
    #[ignore = "requires GAVNEL_TEST_INSTALLER; runs a real installer in an isolated workspace folder"]
    fn installer_replaces_a_closed_process_and_preserves_user_data() {
        let installer = PathBuf::from(
            std::env::var_os("GAVNEL_TEST_INSTALLER").expect("Provide the built installer"),
        );
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("work/update-smoke");
        std::fs::create_dir_all(&root).unwrap();
        let fixture = tempfile::tempdir_in(&root).unwrap();
        let application = fixture.path().join("app");
        assert!(
            installer_command(&installer, &application)
                .status()
                .unwrap()
                .success()
        );
        let target = application.join("Gavnel.exe").canonicalize().unwrap();
        assert!(managed_executable(&target));
        let original = std::fs::read(&target).unwrap();
        let data = fixture.path().join("recordings-and-settings.keep");
        std::fs::write(&data, b"recordings and settings stay separate").unwrap();
        // A test-runner copy provides a real, short-lived parent at the installed path
        // without opening the UI, reading real preferences, or starting any capture.
        std::fs::copy(std::env::current_exe().unwrap(), &target).unwrap();
        let mut parent = Command::new(&target)
            .args([
                "--exact",
                "update::install::tests::update_parent_fixture",
                "--ignored",
            ])
            .creation_flags(CREATE_NO_WINDOW.0)
            .spawn()
            .unwrap();
        let bytes = std::fs::read(&installer).unwrap();
        let plan = Plan {
            parent: parent.id(),
            target: target.clone(),
            installer,
            digest: format!("{:x}", Sha256::digest(&bytes)),
            size: bytes.len() as u64,
            version: env!("CARGO_PKG_VERSION").into(),
        };
        let started = std::time::Instant::now();
        let result = install_after_exit(&plan);
        assert!(parent.wait().unwrap().success());
        assert!(started.elapsed() >= std::time::Duration::from_millis(800));
        let replaced = std::fs::read(&target).unwrap();
        let uninstall = Command::new(application.join("unins000.exe"))
            .args(["/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART"])
            .creation_flags(CREATE_NO_WINDOW.0)
            .status()
            .unwrap();
        assert!(uninstall.success());
        result.unwrap();
        assert_eq!(Sha256::digest(&original), Sha256::digest(&replaced));
        assert_eq!(
            std::fs::read(data).unwrap(),
            b"recordings and settings stay separate"
        );
    }
}
