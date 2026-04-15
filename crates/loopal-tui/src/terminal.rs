use std::io;

use crossterm::{
    event::{
        DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
        KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{
        EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
        supports_keyboard_enhancement,
    },
};

/// RAII guard that ensures raw mode and alternate screen are cleaned up on drop,
/// even if the TUI panics or returns early via `?`.
pub struct TerminalGuard {
    keyboard_enhanced: bool,
}

impl TerminalGuard {
    pub fn new() -> io::Result<Self> {
        enable_raw_mode()?;
        let keyboard_enhanced = supports_keyboard_enhancement().unwrap_or(false);
        let mut stdout = io::stdout();
        execute!(
            stdout,
            EnterAlternateScreen,
            EnableMouseCapture,
            EnableBracketedPaste
        )?;
        if keyboard_enhanced {
            let _ = execute!(
                stdout,
                PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
            );
        }
        Ok(Self { keyboard_enhanced })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        if self.keyboard_enhanced {
            let _ = execute!(io::stdout(), PopKeyboardEnhancementFlags);
        }
        let _ = disable_raw_mode();
        let _ = execute!(
            io::stdout(),
            DisableBracketedPaste,
            DisableMouseCapture,
            LeaveAlternateScreen
        );
    }
}

/// Restore terminal before panic output so backtraces are readable.
///
/// Must be called BEFORE `TerminalGuard::new()`.
pub fn install_panic_hook() {
    let original = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(
            io::stdout(),
            PopKeyboardEnhancementFlags,
            DisableBracketedPaste,
            DisableMouseCapture,
            LeaveAlternateScreen,
        );
        original(info);
    }));
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn install_panic_hook_replaces_default() {
        static CUSTOM_CALLED: AtomicBool = AtomicBool::new(false);

        // Install a marker hook, then our hook on top
        std::panic::set_hook(Box::new(|_| {
            CUSTOM_CALLED.store(true, Ordering::SeqCst);
        }));
        super::install_panic_hook();

        // Trigger panic in catch_unwind — our hook should chain to the marker
        let _ = std::panic::catch_unwind(|| panic!("test"));
        assert!(
            CUSTOM_CALLED.load(Ordering::SeqCst),
            "install_panic_hook must chain to the previous hook"
        );

        // Restore default to avoid poisoning other tests
        let _ = std::panic::take_hook();
    }
}
