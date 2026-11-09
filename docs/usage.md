# Using Gavnel

[← Back to the README](../README.md)

## Capture and recording modes

Open the recorder dropdown in the top bar to choose a game, window, or display. Its source card shows a live preview while capture is active. The preview refreshes twice per second while the card is open.

Under **Settings → Recording**, choose:

- **Manual capture:** keep recent gameplay in the replay buffer; save a clip with your hotkey. Use the long-recording button or shortcut to record a session yourself.
- **Automatically record entire game session:** detect a supported game, record while it is open, save when it closes, and wait for the next one.
- **Recording off:** disable capture and recording shortcuts.

The built-in profiles cover Fortnite, Valorant, Rust, and Counter-Strike 2. Add other executable names under **Settings → Game detection**. This is process detection, not automatic identification of kills, deaths, or other game events.

The capture on/off state is saved on normal exit and restored at the next launch. A previous long recording finishes on exit; reopening starts a new capture using the saved mode, rather than continuing that file.

## Clips, sessions, and bookmarks

The default clip key is **F8** and the default replay duration is **60 seconds**. Click the combined keybind/duration control in the toolbar to change it. Additional clip hotkeys can have their own replay lengths while sharing one buffer.

**Alt+F7** starts or stops a long recording. While a long recording is active, **F8** and the dedicated **F4** bookmark shortcut mark timestamps instead of saving replay clips. Use those timestamps and timeline markers in the built-in player to revisit a moment.

Long recordings stream compressed video and audio to disk. They have no configured duration cap, but storage capacity and configured limits still apply. At 20 Mbps, video alone uses about 9 GB per hour. Replay retention can shorten at high bitrates to stay within its memory budget.

Changing the source, quality, or recording audio settings during capture starts a fresh replay buffer. For a long recording, it saves the current file and starts another.

## Screenshots

**F7** saves a full-resolution PNG of the selected source. Capture can already be active, or Gavnel briefly opens the source to take the screenshot. A selected game or window must be in the foreground. PNG files appear in the library and open in your default image viewer.

## Hotkeys

Manage recording hotkeys under **Settings → Recording**. Bindings can use Tap or Long press; a long press fires once after 500 ms. A tap and a long press can share a key, but duplicate key/trigger combinations are rejected. Escape cancels shortcut entry.

Each action retains at least one row. **Unset hotkey** clears a binding; **Remove hotkey** removes an extra row for that action. Existing saved bindings are preserved across updates.

## Audio

PC audio is enabled by default; the microphone is disabled. Choose all playback devices or selected outputs, adjust their volume, select a microphone, and optionally enable separate audio tracks.

The microphone controls include a local recording/playback test, mono input, input sensitivity, push to talk, and noise suppression. Standard suppression targets steady noise; the AI option runs locally and can affect music or singing. Changing microphone settings clears the previous test sample.

Audio devices are retried after disconnection. Desktop and microphone streams have independent timing anchors and are aligned to the recording clock.

## Your library and files

Choose a save folder under **Settings → Storage**. Recordings go into game/app subfolders; monitor recordings use a Desktop folder. MP4 videos contain H.264 video and AAC audio. Screenshots use PNG. Long-recording bookmarks are stored in a companion `.bookmarks` file.

The library reads both source folders and older loose clips. Search, group by game, sort, and change card sizes; these view preferences are saved. Clicking a video opens the built-in player. You can also reveal it in File Explorer or open it with another player.

Changing the capture folder affects new clips and future recordings without clearing the replay buffer. A queued save finishes in its original folder, and an active long recording finishes in the folder where it started. Existing files are not moved automatically.

**Organize existing clips** is an explicit action in Storage settings. It moves recognized older recordings into source folders, without replacing existing destinations. Files already organized, unrelated filenames, and locked files are left alone.

The storage limit blocks further writes when reached; it does not automatically delete older clips. Deletion from the library asks for confirmation. Keep your own backups of important recordings.

## Exit, tray, and saved settings

By default, closing Gavnel exits. Under **Settings → General → Window & startup**, you can opt into closing to the system tray or starting with Windows.

Closing to the tray keeps capture active. Click the tray icon to reopen the window, or right-click for recording controls and Exit. Launching Gavnel again restores the existing window instead of opening another recorder.

Normal Exit waits for pending saves and finalizes long recordings. Avoid force-closing the process while a recording is saving. A crash or power loss can leave an unplayable `.mp4.partial` file.

Settings are shared between app versions in `%APPDATA%\Gavnel\Gavnel\config`. The save folder has a separate recovery file, and previous settings are backed up when saving.

## Current capture limits

- Windows 11 x64, SDR, and a compatible NVIDIA GPU/driver are the current target. There is no CPU, AMD, or Intel video-encoding fallback.
- One source is captured at a time. A single replay export can be active with one more queued.
- The retained compressed-packet budget is 512 MiB; total RAM and GPU memory can be higher.
- Game/window capture becomes black when its target loses focus; audio continues. A monitor source captures everything visible on that display.
- Source loss, device recovery, or a capture-backend change may reset the replay buffer. Automatic sessions wait or retry; manual long recordings stop and attempt to save received frames.
