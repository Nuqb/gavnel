use anyhow::Result;
use std::time::{Duration, Instant};
use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0},
        System::Threading::*,
    },
    core::PCWSTR,
};

/// A private high-resolution timer; no process-wide timer-resolution change or spin loop.
pub struct FrameTimer(HANDLE);
impl FrameTimer {
    pub fn new() -> Result<Self> {
        Ok(Self(unsafe {
            CreateWaitableTimerExW(
                None,
                PCWSTR::null(),
                CREATE_WAITABLE_TIMER_HIGH_RESOLUTION,
                TIMER_ALL_ACCESS.0,
            )?
        }))
    }
    pub fn until(&self, deadline: Instant) -> Result<()> {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Ok(());
        }
        let due = -(remaining.as_nanos().div_ceil(100).min(i64::MAX as u128) as i64);
        unsafe {
            SetWaitableTimerEx(self.0, &due, 0, None, None, None, 0)?;
            anyhow::ensure!(
                WaitForSingleObject(self.0, 1000) == WAIT_OBJECT_0,
                "Capture timer failed to wake"
            );
        }
        Ok(())
    }
}
impl Drop for FrameTimer {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

/// Skip missed slots instead of sending a burst of stale frames to the encoder.
pub fn advance(deadline: &mut Instant, now: Instant, interval: Duration) -> u64 {
    *deadline += interval;
    if *deadline > now {
        return 0;
    }
    let skipped = now.duration_since(*deadline).as_nanos() / interval.as_nanos() + 1;
    *deadline += interval * skipped.min(u32::MAX as u128) as u32;
    skipped as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn schedule_keeps_phase_and_skips_backlogs() {
        let origin = Instant::now();
        let mut next = origin;
        let frame = Duration::from_millis(10);
        assert_eq!(
            advance(&mut next, origin + Duration::from_millis(2), frame),
            0
        );
        assert_eq!(next, origin + frame);
        assert_eq!(
            advance(&mut next, origin + Duration::from_millis(47), frame),
            3
        );
        assert_eq!(next, origin + Duration::from_millis(50));
    }
}
