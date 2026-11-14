// kernel/src/graphics/app.rs - Typed Application-to-Compositor Interface (UI-04.1)
// Provides a strict boundary between document/editor application state and window chrome/compositing.

pub const CLIPBOARD_SIZE: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCode {
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    Backspace,
    Delete,
    Enter,
    Tab,
    Escape,
    Char(u8),
    Ctrl(u8), // e.g. Ctrl+C (3), Ctrl+V (22), Ctrl+S (19), Ctrl+X (24), Ctrl+Z (26)
    ShiftArrow { left: bool, right: bool, up: bool, down: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppEvent {
    InputChar(u8),
    Key(KeyCode),
    PointerClick { x: i32, y: i32 },
    PointerDrag { cur_x: i32, cur_y: i32 },
    PointerRelease,
    Scroll { delta_lines: i32 },
    FocusGained,
    FocusLost,
    RequestSave,
    ClipboardCopy,
    ClipboardPaste,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppCommand {
    None,
    RequestRedraw,
    SetDirty(bool),
    SaveFile,
    SetClipboard { len: usize },
    CloseWindow,
}

pub trait WindowApp {
    fn app_id(&self) -> &'static str;
    fn handle_event(&mut self, event: AppEvent, clipboard: &mut [u8; CLIPBOARD_SIZE], clip_len: &mut usize) -> AppCommand;
    unsafe fn render_surface(
        &self,
        gop: &mut super::GopBuffer,
        content_rect: super::Rect,
        is_focused: bool,
    );
    fn is_dirty(&self) -> bool;
    fn title(&self) -> &str;
    fn subtitle(&self) -> &str;
    fn get_content_bytes(&self) -> &[u8];
}
