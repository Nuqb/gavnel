# Local test checklist

Use the desktop source launcher to open the completed build. Use these checks when preparing an editor release.

Run `cargo test --locked media::tests` for automated native-media checks. For the
packaged-app smoke check, run:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/test-media-runtime.ps1 -App PATH_TO_GAVNEL_EXE
```

This creates temporary synthetic media and exercises inspection, thumbnails,
audio mixing, and export without capturing or playing sound.

For a release, also test a fresh installation without a `media` directory or
bundled codec DLLs. With Windows Smart App Control enabled, open clips, seek,
change the audio mix, and export. Include filenames containing spaces and Unicode,
and portrait or custom crops with dimensions that are not multiples of 16.
Run installation checks in a clean session; do not stop an active recorder to test
an installer. Automated media tests do not establish Smart App Control acceptance
on every machine.

1. Open a clip → Edit. Drag both trim handles, enter exact times, undo/redo, and export. Confirm the source is unchanged.
2. Crop to 9:16 and Square, drag the borders to switch to Custom, and use Play or Space. Confirm the exported framing matches.
3. Open a separate-track recording. Mute the microphone, solo game audio, preview and export; check there is no doubled audio.
4. Select clips in the Library → Create montage. Reorder/remove segments, mix silent and differently sized clips, and export.
5. Try a small file-size limit and Cancel an export. Check the result size, absence of partial outputs, and an existing filename error.
6. Use Save video to save a new MP4 and replace the current video. Check resolution/file-size settings, background progress, cancellation, completion View, and source preservation on failure. Change a trim, crop, or audio setting, then leave using Library, Settings, or Game detection: check Save, Discard, and Cancel. Confirm Cancel keeps edits, Discard leaves the saved video unchanged, and Save opens the video-saving dialog. Check that app exit, renaming, deleting, and organizing cannot interrupt an active save. Rename a capture and verify its companion metadata follows it.
7. Select Game only or Selected apps audio. Record your game plus Discord while another app plays audio; only selected apps should appear in the recording. Restart a selected app and check reconnection.
8. Enable CS2 highlights, connect its cfg folder, restart CS2, and test the selected events in both replay and long-recording modes.
9. For Fortnite, Valorant, and Rust, enable experimental highlights and inspect HUD calibration. Test actual gameplay at your HUD scale/language. Record misses/false events; Rust kill detection is intentionally absent.
10. Check minimum window size, Windows display scaling, a right-to-left locale, keyboard navigation, and normal shutdown while recording/exporting. Check integrated window buttons, header dragging/double-clicking, edge resizing, and attached playback controls during minimize/maximize/restore and fullscreen transitions.

Synthetic verification covers media transformations, timing, mute isolation, source preservation, real Windows OCR, and real Windows process-audio isolation. It does not establish detection accuracy for live games. New UI messages currently use English fallback outside English pending translation review.
