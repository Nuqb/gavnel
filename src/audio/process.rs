//! Process-tree loopback. No endpoint fallback: missing apps produce silence.
use super::*;
use std::mem::ManuallyDrop;
use windows::{
    Win32::{
        Foundation::CloseHandle,
        System::{Com::StructuredStorage::*, Diagnostics::ToolHelp::*, Variant::VT_BLOB},
    },
    core::{HRESULT, Interface, Ref, implement},
};

#[derive(Clone, Debug)]
pub struct AppProcess {
    pub pid: u32,
    pub parent: u32,
    pub executable: String,
}
pub fn processes() -> Vec<AppProcess> {
    unsafe {
        let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return vec![];
        };
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut apps = vec![];
        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let end = entry
                    .szExeFile
                    .iter()
                    .position(|c| *c == 0)
                    .unwrap_or(entry.szExeFile.len());
                apps.push(AppProcess {
                    pid: entry.th32ProcessID,
                    parent: entry.th32ParentProcessID,
                    executable: String::from_utf16_lossy(&entry.szExeFile[..end]),
                });
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snapshot);
        apps
    }
}
pub fn roots(apps: &[AppProcess], names: &[String]) -> Vec<AppProcess> {
    let selected: std::collections::HashSet<u32> = apps
        .iter()
        .filter(|a| {
            names
                .iter()
                .any(|name| name.eq_ignore_ascii_case(&a.executable))
        })
        .map(|a| a.pid)
        .collect();
    apps.iter()
        .filter(|a| {
            if !selected.contains(&a.pid) {
                return false;
            }
            let mut parent = a.parent;
            for _ in 0..64 {
                if parent == 0 || parent == a.pid {
                    break;
                }
                if selected.contains(&parent) {
                    return false;
                }
                match apps.iter().find(|p| p.pid == parent) {
                    Some(p) => parent = p.parent,
                    None => break,
                }
            }
            true
        })
        .cloned()
        .collect()
}
#[implement(IActivateAudioInterfaceCompletionHandler)]
struct Activation {
    ready: crossbeam_channel::Sender<()>,
}
impl IActivateAudioInterfaceCompletionHandler_Impl for Activation_Impl {
    fn ActivateCompleted(
        &self,
        _operation: Ref<IActivateAudioInterfaceAsyncOperation>,
    ) -> windows::core::Result<()> {
        let _ = self.ready.try_send(());
        Ok(())
    }
}
impl Endpoint {
    pub(super) fn open_process(pid: u32) -> Result<Self> {
        unsafe {
            let mut params = AUDIOCLIENT_ACTIVATION_PARAMS {
                ActivationType: AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK,
                Anonymous: AUDIOCLIENT_ACTIVATION_PARAMS_0 {
                    ProcessLoopbackParams: AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS {
                        TargetProcessId: pid,
                        ProcessLoopbackMode: PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE,
                    },
                },
            };
            // The blob borrows stack storage. PROPVARIANT's generated Drop would
            // otherwise call PropVariantClear and free that borrowed pointer.
            let variant = ManuallyDrop::new(PROPVARIANT {
                Anonymous: PROPVARIANT_0 {
                    Anonymous: ManuallyDrop::new(PROPVARIANT_0_0 {
                        vt: VT_BLOB,
                        Anonymous: PROPVARIANT_0_0_0 {
                            blob: BLOB {
                                cbSize: std::mem::size_of_val(&params) as u32,
                                pBlobData: (&mut params as *mut AUDIOCLIENT_ACTIVATION_PARAMS)
                                    .cast(),
                            },
                        },
                        ..Default::default()
                    }),
                },
            });
            let (tx, rx) = crossbeam_channel::bounded(1);
            let completion: IActivateAudioInterfaceCompletionHandler =
                Activation { ready: tx }.into();
            let operation = ActivateAudioInterfaceAsync(
                VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK,
                &IAudioClient::IID,
                Some(&*variant),
                &completion,
            )?;
            rx.recv_timeout(Duration::from_secs(3))
                .context("Application audio activation timed out")?;
            let mut status = HRESULT(0);
            let mut unknown = None;
            operation.GetActivateResult(&mut status, &mut unknown)?;
            status.ok()?;
            let client: IAudioClient = unknown
                .context("Application audio activation returned no client")?
                .cast()?;
            let format = WAVEFORMATEX {
                wFormatTag: 3,
                nChannels: 2,
                nSamplesPerSec: 48_000,
                nAvgBytesPerSec: 384_000,
                nBlockAlign: 8,
                wBitsPerSample: 32,
                cbSize: 0,
            };
            client.Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_LOOPBACK | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM,
                1_000_000,
                0,
                &format,
                None,
            )?;
            let capture: IAudioCaptureClient = client.GetService()?;
            client.Start()?;
            Ok(Self {
                client,
                capture,
                segments: VecDeque::new(),
                timeline: CaptureTimeline::new(48_000),
                trace: None,
            })
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selection_is_case_insensitive_and_never_duplicates_children() {
        let apps = vec![
            AppProcess {
                pid: 1,
                parent: 0,
                executable: "Discord.exe".into(),
            },
            AppProcess {
                pid: 2,
                parent: 1,
                executable: "Discord.exe".into(),
            },
            AppProcess {
                pid: 3,
                parent: 0,
                executable: "Game.exe".into(),
            },
        ];
        assert_eq!(
            roots(&apps, &["discord.EXE".into()])
                .iter()
                .map(|a| a.pid)
                .collect::<Vec<_>>(),
            vec![1]
        );
        assert!(roots(&apps, &[]).is_empty());
    }
}

#[cfg(test)]
mod integration {
    use super::*;
    use std::os::windows::process::CommandExt;
    #[test]
    #[ignore = "plays two quiet synthetic tones and captures only the selected test process"]
    fn process_loopback_isolates_selected_application() {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).unwrap();
        }
        let binary =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("work/media/bin/ffplay.exe");
        let spawn = |frequency| {
            std::process::Command::new(&binary)
                .creation_flags(0x08000000)
                .args([
                    "-nodisp",
                    "-autoexit",
                    "-loglevel",
                    "error",
                    "-volume",
                    "1",
                    "-f",
                    "lavfi",
                    "-i",
                    &format!("sine=frequency={frequency}:sample_rate=48000:duration=3"),
                ])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .unwrap()
        };
        let mut selected = spawn(440);
        let mut excluded = spawn(880);
        let clock = Clock::new().unwrap();
        let mut endpoint = Endpoint::open_process(selected.id()).unwrap();
        let started = Instant::now();
        let mut samples = Vec::new();
        while started.elapsed() < Duration::from_millis(2400) {
            endpoint.read(&clock).unwrap();
            for segment in endpoint.segments.drain(..) {
                samples.extend(segment.frames.into_iter().map(|s| s[0] as f64));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        drop(endpoint);
        selected.wait().unwrap();
        excluded.wait().unwrap();
        let amplitude = |frequency: f64| {
            let (mut re, mut im) = (0.0, 0.0);
            for (i, sample) in samples.iter().enumerate() {
                let phase = std::f64::consts::TAU * frequency * i as f64 / 48000.0;
                re += sample * phase.cos();
                im += sample * phase.sin();
            }
            (re * re + im * im).sqrt()
        };
        assert!(samples.len() > 48000, "No process audio arrived");
        assert!(
            amplitude(440.0) > amplitude(880.0) * 10.0,
            "Excluded application leaked into capture"
        );
        unsafe {
            CoUninitialize();
        }
    }
}
