//! Hold to talk on the mic: the pure gesture logic. The controller feeds it pointer events and the hold timer.
//!
//! A pointer press in `idle`, `nothingHeard` or `error` starts listening at once, so no speech is lost while the
//! gesture is still unknown. A press that outlives `holdMs` is a hold, and its release sends `STOP`. A shorter
//! press is a tap: the session it started goes on. The click that follows a handled press is swallowed, so it
//! does not toggle a second time. A click without a press (Space, Enter, assistive tech) toggles.

#[derive(Debug, Default)]
pub(crate) struct HoldGesture {
    press: u32,
    down: bool,
    held: bool,
    swallow_click: bool,
}

impl HoldGesture {
    /// A primary pointer went down. `can_start` is true in `idle` and `error`. Returns the press number for
    /// the hold timer when the press started listening; `None` leaves the press to the click (a plain tap).
    pub(crate) fn down(&mut self, can_start: bool) -> Option<u32> {
        if !can_start {
            return None;
        }
        self.press = self.press.wrapping_add(1);
        self.down = true;
        self.held = false;
        self.swallow_click = true;
        Some(self.press)
    }

    /// The hold timer of `press` fired. Returns true when the press became a hold (`data-hold="true"`).
    pub(crate) fn timer(&mut self, press: u32) -> bool {
        if self.down && press == self.press && !self.held {
            self.held = true;
            return true;
        }
        false
    }

    /// The pointer went up on the mic. Returns true when a hold ended, which sends `STOP`.
    pub(crate) fn up(&mut self) -> bool {
        if !self.down {
            return false;
        }
        self.down = false;
        std::mem::take(&mut self.held)
    }

    /// The pointer left the mic or the browser cancelled it while pressed. No click follows then.
    /// Returns true when a hold ended. After `up` it does nothing: touch fires `pointerleave` before `click`.
    pub(crate) fn cancel(&mut self) -> bool {
        if !self.down {
            return false;
        }
        self.swallow_click = false;
        self.up()
    }

    /// A click on the mic. Returns true when it should toggle.
    pub(crate) fn click(&mut self) -> bool {
        !std::mem::take(&mut self.swallow_click)
    }

    /// True while a hold lasts.
    #[cfg(test)]
    pub(crate) fn held(&self) -> bool {
        self.held
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_press_is_a_tap_that_keeps_listening() {
        let mut g = HoldGesture::default();
        let press = g.down(true).expect("starts listening");
        assert!(!g.up(), "a tap does not stop");
        assert!(!g.timer(press), "a late timer of a released press does nothing");
        assert!(!g.click(), "the click of the handled press is swallowed");
        assert!(g.click(), "the next click toggles");
    }

    #[test]
    fn a_long_press_is_a_hold_and_its_release_stops() {
        let mut g = HoldGesture::default();
        let press = g.down(true).unwrap();
        assert!(g.timer(press));
        assert!(g.held());
        assert!(g.up(), "releasing a hold stops");
        assert!(!g.held());
        assert!(!g.click());
    }

    #[test]
    fn a_press_while_busy_is_a_plain_tap() {
        let mut g = HoldGesture::default();
        assert_eq!(g.down(false), None);
        assert!(!g.up());
        assert!(g.click(), "the click toggles, which stops");
    }

    #[test]
    fn keyboard_clicks_toggle() {
        let mut g = HoldGesture::default();
        assert!(g.click());
        assert!(g.click());
    }

    #[test]
    fn leaving_while_held_stops_and_no_click_is_swallowed() {
        let mut g = HoldGesture::default();
        let press = g.down(true).unwrap();
        assert!(g.timer(press));
        assert!(g.cancel());
        assert!(g.click(), "the next real click is not swallowed");
    }

    #[test]
    fn leave_after_up_keeps_the_swallow() {
        let mut g = HoldGesture::default();
        g.down(true).unwrap();
        assert!(!g.up());
        assert!(!g.cancel(), "touch: pointerleave after pointerup");
        assert!(!g.click(), "the touch click is still swallowed");
    }

    #[test]
    fn a_timer_of_an_old_press_is_ignored() {
        let mut g = HoldGesture::default();
        let first = g.down(true).unwrap();
        g.up();
        g.click();
        let _second = g.down(true).unwrap();
        assert!(!g.timer(first));
        assert!(!g.held());
    }
}
