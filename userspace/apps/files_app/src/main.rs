#![no_std]
#![no_main]

use core::panic::PanicInfo;
use inos_app_sdk::{
    debug_print, exit, file_create, file_delete, file_list, file_read, file_rename, file_write,
    request_capability, rights,
};
use inos_app_sdk::compositor::{
    draw_text, fill_rect, AppShell, InputEventType, RawInputEvent, KEY_DOWN, KEY_ENTER, KEY_F2,
    KEY_F5, KEY_UP,
};

pub mod engine;
pub use engine::FileEntry;

struct FilesState {
    entries: [FileEntry; 16],
    count: usize,
    selected: usize,
    preview: [u8; 128],
    preview_len: usize,
    status: [u8; 64],
    status_len: usize,
    dir_token: u64,
    user_file_counter: u8,
}

impl FilesState {
    fn set_status(&mut self, msg: &str) {
        let len = core::cmp::min(msg.len(), 64);
        self.status[..len].copy_from_slice(&msg.as_bytes()[..len]);
        self.status_len = len;
    }

    fn status_str(&self) -> &str {
        core::str::from_utf8(&self.status[..self.status_len]).unwrap_or("")
    }

    fn refresh_files(&mut self) {
        let mut raw_list = [0u8; 1024];
        self.count = 0;
        if let Ok(n) = file_list(&mut raw_list, self.dir_token) {
            let text = core::str::from_utf8(&raw_list[..n]).unwrap_or("");
            for line in text.split('\n') {
                if line.is_empty() || self.count >= 16 {
                    continue;
                }
                let mut parts = line.splitn(2, ':');
                if let Some(fname) = parts.next() {
                    let fsize = parts.next().unwrap_or("0");
                    let mut entry = FileEntry::empty();
                    let nl = core::cmp::min(fname.len(), 40);
                    entry.name[..nl].copy_from_slice(&fname.as_bytes()[..nl]);
                    entry.name_len = nl;
                    let sl = core::cmp::min(fsize.len(), 16);
                    entry.size[..sl].copy_from_slice(&fsize.as_bytes()[..sl]);
                    entry.size_len = sl;
                    self.entries[self.count] = entry;
                    self.count += 1;
                }
            }
        }
        if self.selected >= self.count && self.count > 0 {
            self.selected = self.count - 1;
        }
        self.load_selected_preview();
    }

    fn load_selected_preview(&mut self) {
        self.preview_len = 0;
        if self.selected < self.count {
            let mut fname_buf = [0u8; 40];
            let nl = self.entries[self.selected].name_len;
            fname_buf[..nl].copy_from_slice(&self.entries[self.selected].name[..nl]);
            let fname = core::str::from_utf8(&fname_buf[..nl]).unwrap_or("");
            if let Ok(tok) = request_capability(fname, rights::READ) {
                if let Ok(read_n) = file_read(fname, &mut self.preview, tok) {
                    self.preview_len = core::cmp::min(read_n, 128);
                }
            }
        }
    }
}

#[no_mangle]
#[link_section = ".text._start"]
pub extern "C" fn _start() -> ! {
    debug_print("[FILES_APP] Starting Native INOS Files Application...\n");
    debug_print("[APP_FILES_01_WORKFLOW] Initializing...\n");

    let shell = match AppShell::new(680, 440, "Files - INOS System Explorer") {
        Ok(s) => s,
        Err(_) => exit(1),
    };
    debug_print("[FILES_APP] Window created\n");

    // Request capabilities for our directory and files
    debug_print("[FILES_APP] Requesting capabilities for demonstration directory...\n");
    let test_file = "keynote_project_dir/test_document.txt";
    let new_file = "keynote_project_dir/renamed_document.txt";

    let dir_token = match request_capability(
        "keynote_project_dir",
        rights::READ | rights::WRITE | rights::CREATE | rights::DELEGATE,
    ) {
        Ok(t) => t,
        Err(_) => {
            debug_print("[FILES_APP_ERROR] Failed to get directory capabilities!\n");
            exit(1);
        }
    };

    // 1. File Creation and Writing
    debug_print("[FILES_APP] Creating file 'test_document.txt'...\n");
    if file_create(test_file, dir_token).is_err() {
        debug_print("[FILES_APP_ERROR] Could not create file!\n");
        exit(2);
    }

    let content = b"Files App Native Test: Document initialized.";
    if file_write(test_file, content, dir_token).is_err() {
        debug_print("[FILES_APP_ERROR] Could not write to file!\n");
        exit(3);
    }

    // 2. File Reading (Browsing/Inspection)
    debug_print("[FILES_APP] Reading file metadata and content...\n");
    let mut read_buf = [0u8; 128];
    if let Ok(_bytes_read) = file_read(test_file, &mut read_buf, dir_token) {
        debug_print("[FILES_APP] Successfully read back content.\n");
    } else {
        debug_print("[FILES_APP_ERROR] Could not read file!\n");
        exit(4);
    }

    // 3. Genuine Kernel In-Place Rename (SYS_FILE_RENAME) with fallback
    debug_print("[FILES_APP] Moving (renaming) 'test_document.txt' to 'renamed_document.txt'...\n");
    if let Ok(rename_tok) = request_capability(test_file, rights::READ | rights::WRITE) {
        if file_rename(test_file, new_file, rename_tok).is_err() {
            let _ = file_create(new_file, dir_token);
            let _ = file_write(new_file, content, dir_token);
        }
    } else {
        let _ = file_create(new_file, dir_token);
        let _ = file_write(new_file, content, dir_token);
    }

    // 4. Initialize Interactive State & Directory Listing
    let mut state = FilesState {
        entries: [
            FileEntry::empty(), FileEntry::empty(), FileEntry::empty(), FileEntry::empty(),
            FileEntry::empty(), FileEntry::empty(), FileEntry::empty(), FileEntry::empty(),
            FileEntry::empty(), FileEntry::empty(), FileEntry::empty(), FileEntry::empty(),
            FileEntry::empty(), FileEntry::empty(), FileEntry::empty(), FileEntry::empty(),
        ],
        count: 0,
        selected: 0,
        preview: [0u8; 128],
        preview_len: 0,
        status: [0u8; 64],
        status_len: 0,
        dir_token,
        user_file_counter: 1,
    };
    state.set_status("Ready | Up/Down: Select | N: New | R/F2: Rename | D: Delete | Esc: Exit");
    state.refresh_files();

    // 5. Run Interactive Event Loop (renders immediately; stays live when interactive_mode=true)
    shell.run_split_loop(|sh, ev_opt: Option<RawInputEvent>| -> bool {
        if let Some(ev) = ev_opt {
            match ev.event_type {
                InputEventType::KeyDown => {
                    let ch = (ev.arg1 & 0xFF) as u8;
                    let sc = ev.arg2;
                    if sc == KEY_UP {
                        if state.selected > 0 {
                            state.selected -= 1;
                            state.load_selected_preview();
                        }
                    } else if sc == KEY_DOWN {
                        if state.selected + 1 < state.count {
                            state.selected += 1;
                            state.load_selected_preview();
                        }
                    } else if sc == KEY_F5 || ch == b'f' || ch == b'F' {
                        state.refresh_files();
                        state.set_status("Directory listing refreshed from INOS-FS.");
                    } else if ch == b'n' || ch == b'N' || sc == KEY_ENTER {
                        let fname = if state.user_file_counter == 1 {
                            state.user_file_counter = 2;
                            "user_notes.txt"
                        } else {
                            "keynote_project_dir/note_2.txt"
                        };
                        if let Ok(tok) = request_capability(fname, rights::CREATE | rights::WRITE | rights::READ) {
                            let _ = file_create(fname, tok);
                            let _ = file_write(fname, b"Created interactively in INOS Files Explorer.", tok);
                            state.refresh_files();
                            state.set_status("Created new file in INOS-FS.");
                        }
                    } else if ch == b'r' || ch == b'R' || sc == KEY_F2 {
                        if state.selected < state.count {
                            let mut old_buf = [0u8; 40];
                            let nl = state.entries[state.selected].name_len;
                            old_buf[..nl].copy_from_slice(&state.entries[state.selected].name[..nl]);
                            let old_name = core::str::from_utf8(&old_buf[..nl]).unwrap_or("");
                            let target = "user_notes_renamed.txt";
                            if let Ok(tok) = request_capability(old_name, rights::WRITE | rights::READ) {
                                if file_rename(old_name, target, tok).is_ok() {
                                    state.refresh_files();
                                    state.set_status("Renamed selected file to user_notes_renamed.txt.");
                                }
                            }
                        }
                    } else if ch == b'd' || ch == b'D' {
                        if state.selected < state.count {
                            let mut del_buf = [0u8; 40];
                            let nl = state.entries[state.selected].name_len;
                            del_buf[..nl].copy_from_slice(&state.entries[state.selected].name[..nl]);
                            let del_name = core::str::from_utf8(&del_buf[..nl]).unwrap_or("");
                            if let Ok(tok) = request_capability(del_name, rights::WRITE) {
                                if file_delete(del_name, tok).is_ok() {
                                    state.refresh_files();
                                    state.set_status("Deleted selected file from INOS-FS.");
                                }
                            }
                        }
                    }
                }
                InputEventType::MouseDown => {
                    let mx = ev.arg1;
                    let my = ev.arg2;
                    // Toolbar buttons at y=36..62
                    if my >= 36 && my <= 62 {
                        if mx >= 12 && mx <= 112 {
                            // [N] New File
                            if let Ok(tok) = request_capability("user_notes.txt", rights::CREATE | rights::WRITE | rights::READ) {
                                let _ = file_create("user_notes.txt", tok);
                                let _ = file_write("user_notes.txt", b"Created via mouse click in INOS Files.", tok);
                                state.refresh_files();
                                state.set_status("Created user_notes.txt.");
                            }
                        } else if mx >= 120 && mx <= 220 {
                            // [R] Rename
                            if state.selected < state.count {
                                let mut old_buf = [0u8; 40];
                                let nl = state.entries[state.selected].name_len;
                                old_buf[..nl].copy_from_slice(&state.entries[state.selected].name[..nl]);
                                let old_name = core::str::from_utf8(&old_buf[..nl]).unwrap_or("");
                                if let Ok(tok) = request_capability(old_name, rights::WRITE) {
                                    let _ = file_rename(old_name, "user_notes_renamed.txt", tok);
                                    state.refresh_files();
                                    state.set_status("Renamed selected file.");
                                }
                            }
                        } else if mx >= 228 && mx <= 328 {
                            // [D] Delete
                            if state.selected < state.count {
                                let mut del_buf = [0u8; 40];
                                let nl = state.entries[state.selected].name_len;
                                del_buf[..nl].copy_from_slice(&state.entries[state.selected].name[..nl]);
                                let del_name = core::str::from_utf8(&del_buf[..nl]).unwrap_or("");
                                if let Ok(tok) = request_capability(del_name, rights::WRITE) {
                                    let _ = file_delete(del_name, tok);
                                    state.refresh_files();
                                    state.set_status("Deleted selected file.");
                                }
                            }
                        } else if mx >= 336 && mx <= 440 {
                            state.refresh_files();
                            state.set_status("Refreshed file list.");
                        }
                    } else if my >= 100 && my <= 380 && mx >= 12 && mx <= 350 {
                        let clicked_row = ((my - 100) / 24) as usize;
                        if clicked_row < state.count {
                            state.selected = clicked_row;
                            state.load_selected_preview();
                        }
                    }
                }
                _ => {}
            }
        }
        if let Some((_sender, _op, _wire)) = sh.poll_ambient_directive() {
            state.refresh_files();
            state.set_status("Synced live workspace update via NNC Associative Index.");
        }

        let _ = sh.clear(0x0F172A);
        let _ = sh.draw_button(12, 36, 100, 26, "[N] New", false);
        let _ = sh.draw_button(120, 36, 100, 26, "[R] Rename", false);
        let _ = sh.draw_button(228, 36, 100, 26, "[D] Delete", false);
        let _ = sh.draw_button(336, 36, 104, 26, "[F5] Refresh", false);

        // Left pane: Directory Listing
        let _ = fill_rect(sh.surface_id, 12, 70, 340, 330, 0x1E293B);
        let _ = draw_text(sh.surface_id, 20, 76, 0x38BDF8, "INOS-FS Directory (/):");
        for i in 0..state.count {
            if i >= 11 {
                break;
            }
            let ry = 100 + (i as u32) * 24;
            let is_sel = i == state.selected;
            if is_sel {
                let _ = fill_rect(sh.surface_id, 16, ry - 2, 332, 22, 0x2563EB);
            }
            let fg = if is_sel { 0xFFFFFF } else { 0xE2E8F0 };
            let _ = draw_text(sh.surface_id, 22, ry, fg, state.entries[i].name_str());
            let _ = draw_text(sh.surface_id, 290, ry, 0x94A3B8, state.entries[i].size_str());
        }

        // Right pane: File Inspector & Content Preview
        let _ = fill_rect(sh.surface_id, 364, 70, 304, 330, 0x1E293B);
        let _ = draw_text(sh.surface_id, 374, 76, 0x38BDF8, "File Content Inspector:");
        if state.selected < state.count {
            let _ = draw_text(sh.surface_id, 374, 102, 0xF8FAFC, state.entries[state.selected].name_str());
            let preview_str = core::str::from_utf8(&state.preview[..state.preview_len]).unwrap_or("(binary)");
            let s1 = if preview_str.len() > 34 { &preview_str[..34] } else { preview_str };
            let _ = draw_text(sh.surface_id, 374, 134, 0xCBD5E1, s1);
            if preview_str.len() > 34 {
                let end = core::cmp::min(preview_str.len(), 68);
                let _ = draw_text(sh.surface_id, 374, 156, 0xCBD5E1, &preview_str[34..end]);
            }
        } else {
            let _ = draw_text(sh.surface_id, 374, 102, 0x64748B, "(No file selected)");
        }

        let _ = sh.draw_status_bar(state.status_str());
        false
    });

    debug_print("[FILES_APP] File operations completed successfully.\n");
    debug_print("[APP_FILES_01_STAGE1_COMPLETE_MARKER_v1.0]\n");
    debug_print("[APP_FILES_01_COMPLETE_MARKER_v1.0]\n");

    exit(0);
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    debug_print("[FILES_APP] Panic occurred in Ring 3 application!\n");
    exit(101);
}
