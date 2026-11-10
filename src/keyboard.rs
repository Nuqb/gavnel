//! User-approved passive keyboard listener. Events are forwarded, never stored.
mod mapping;
use anyhow::{Result, anyhow};
use global_hotkey::{GlobalHotKeyEvent, HotKeyState, hotkey::HotKey};
use std::{
    cell::RefCell,
    sync::{Arc, Mutex, mpsc},
    thread,
};
use windows::Win32::{
    Foundation::*,
    System::{LibraryLoader::GetModuleHandleW, Threading::GetCurrentThreadId},
    UI::WindowsAndMessaging::*,
};
type Handler = Box<dyn Fn(GlobalHotKeyEvent) + Send + Sync>;
thread_local! { static EVENTS: RefCell<Option<mpsc::Sender<(u32, bool)>>> = const { RefCell::new(None) }; }
pub struct Manager {
    keys: Arc<Mutex<Vec<HotKey>>>,
    handler: Arc<Mutex<Option<Handler>>>,
    thread_id: u32,
    hook: Option<thread::JoinHandle<()>>,
    dispatch: Option<thread::JoinHandle<()>>,
}
impl Manager {
    pub fn new() -> Result<Self> {
        let (events, receiver) = mpsc::channel();
        let (ready, startup) = mpsc::sync_channel(1);
        let hook = thread::spawn(move || run_hook(events, ready));
        let thread_id = startup.recv()?.map_err(|error| anyhow!(error))?;
        let keys = Arc::new(Mutex::new(Vec::new()));
        let handler = Arc::new(Mutex::new(None));
        let (dispatch_keys, dispatch_handler) = (keys.clone(), handler.clone());
        let dispatch = thread::spawn(move || dispatch(receiver, dispatch_keys, dispatch_handler));
        Ok(Self {
            keys,
            handler,
            thread_id,
            hook: Some(hook),
            dispatch: Some(dispatch),
        })
    }
    pub fn set_event_handler(&self, handler: impl Fn(GlobalHotKeyEvent) + Send + Sync + 'static) {
        *self.handler.lock().unwrap() = Some(Box::new(handler));
    }
    pub fn register(&self, key: HotKey) -> Result<()> {
        anyhow::ensure!(
            mapping::key_to_vk(&key.key).is_some(),
            "Unsupported shortcut key"
        );
        self.keys.lock().unwrap().push(key);
        Ok(())
    }
    pub fn unregister(&self, key: HotKey) -> Result<()> {
        self.keys.lock().unwrap().retain(|old| *old != key);
        Ok(())
    }
}
impl Drop for Manager {
    fn drop(&mut self) {
        unsafe {
            let _ = PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
        }
        if let Some(hook) = self.hook.take() {
            let _ = hook.join();
        }
        if let Some(dispatch) = self.dispatch.take() {
            let _ = dispatch.join();
        }
    }
}
unsafe extern "system" fn observe(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let message = wparam.0 as u32;
        if matches!(message, WM_KEYDOWN | WM_SYSKEYDOWN | WM_KEYUP | WM_SYSKEYUP) {
            let key = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
            EVENTS.with(|slot| {
                if let Some(sender) = &*slot.borrow() {
                    let _ =
                        sender.send((key.vkCode, matches!(message, WM_KEYDOWN | WM_SYSKEYDOWN)));
                }
            });
        }
    }
    // Never consume input, even when it matches a shortcut.
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}
fn dispatch(
    receiver: mpsc::Receiver<(u32, bool)>,
    keys: Arc<Mutex<Vec<HotKey>>>,
    handler: Arc<Mutex<Option<Handler>>>,
) {
    let mut down = std::collections::HashSet::new();
    let mut held = std::collections::HashMap::<u32, Vec<u32>>::new();
    while let Ok((vk, pressed)) = receiver.recv() {
        if pressed && !down.insert(vk) {
            continue;
        }
        if !pressed {
            down.remove(&vk);
        }
        let ids = if pressed {
            let mods = mapping::modifiers(&down);
            let ids: Vec<_> = keys
                .lock()
                .unwrap()
                .iter()
                .filter(|key| {
                    mapping::key_to_vk(&key.key).is_some_and(|code| code.0 as u32 == vk)
                        && key.mods == mods
                })
                .map(HotKey::id)
                .collect();
            held.insert(vk, ids.clone());
            ids
        } else {
            held.remove(&vk).unwrap_or_default()
        };
        for id in ids {
            if let Some(callback) = &*handler.lock().unwrap() {
                callback(GlobalHotKeyEvent {
                    id,
                    state: if pressed {
                        HotKeyState::Pressed
                    } else {
                        HotKeyState::Released
                    },
                });
            }
        }
    }
}

fn run_hook(
    events: mpsc::Sender<(u32, bool)>,
    ready: mpsc::SyncSender<std::result::Result<u32, String>>,
) {
    unsafe {
        let mut message = MSG::default();
        let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
        EVENTS.with(|slot| *slot.borrow_mut() = Some(events));
        let result = GetModuleHandleW(None).and_then(|module| {
            SetWindowsHookExW(WH_KEYBOARD_LL, Some(observe), Some(module.into()), 0)
        });
        match result {
            Ok(handle) => {
                let _ = ready.send(Ok(GetCurrentThreadId()));
                while GetMessageW(&mut message, None, 0, 0).0 > 0 {}
                let _ = UnhookWindowsHookEx(handle);
            }
            Err(error) => {
                let _ = ready.send(Err(error.to_string()));
            }
        }
        EVENTS.with(|slot| *slot.borrow_mut() = None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn listener_installs_and_shuts_down_cleanly() {
        drop(Manager::new().unwrap());
    }
    #[test]
    fn shortcuts_ignore_repeat_and_release_even_after_modifier_is_released() {
        let plain: HotKey = "A".parse().unwrap();
        let modified: HotKey = "Control+A".parse().unwrap();
        let keys = Arc::new(Mutex::new(vec![plain, modified]));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let collected = seen.clone();
        let handler: Arc<Mutex<Option<Handler>>> = Arc::new(Mutex::new(Some(Box::new(move |e| {
            collected.lock().unwrap().push((e.id, e.state))
        }))));
        let (sender, receiver) = mpsc::channel();
        for input in [
            (0x41, true),
            (0x41, true),
            (0x41, false),
            (0xA2, true),
            (0x41, true),
            (0xA2, false),
            (0x41, false),
        ] {
            sender.send(input).unwrap();
        }
        drop(sender);
        dispatch(receiver, keys, handler);
        assert_eq!(
            *seen.lock().unwrap(),
            vec![
                (plain.id(), HotKeyState::Pressed),
                (plain.id(), HotKeyState::Released),
                (modified.id(), HotKeyState::Pressed),
                (modified.id(), HotKeyState::Released)
            ]
        );
    }
}
