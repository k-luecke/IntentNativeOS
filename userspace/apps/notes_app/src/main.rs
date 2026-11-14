// userspace/apps/notes_app/src/main.rs - Native Ring 3 Notes Application (APP-CAP-00 & Interactive Editor)

#![no_std]
#![no_main]
#![allow(dead_code, unused_variables)]

use core::panic::PanicInfo;
use inos_app_sdk::syscall::SyscallError;
use inos_app_sdk::{
    compositor::{
        draw_text, fill_rect, set_clipboard, AppShell, InputEventType, RawInputEvent,
        KEY_BACKSPACE, KEY_ENTER, KEY_F2, KEY_F3, KEY_F4,
    },
    debug_print, exit, file_create, file_read, file_write, get_cpl, get_pid, ipc_recv, ipc_send,
    request_capability, rights, yield_now,
};

pub mod engine;

#[no_mangle]
#[link_section = ".text._start"]
pub extern "C" fn _start() -> ! {
    debug_print("[NOTES_APP] Native Ring 3 Notes Application started.\n");

    // 1. Hardware Privilege Level Verification
    let cpl = get_cpl();
    if cpl == 3 {
        debug_print("[NOTES_APP] Hardware CPL=3 isolation confirmed.\n");
    } else {
        debug_print("[NOTES_APP] ERROR: Not running at CPL=3!\n");
        exit(1);
    }

    let pid = get_pid();
    if pid != 0 {
        debug_print("[NOTES_APP] Registered Process ID confirmed.\n");
    }

    // 1b. Memory Isolation & State Preservation Test
    let mem_ptr = 0x0040_8000 as *mut u64;
    unsafe {
        core::ptr::write_volatile(mem_ptr, 0x1111_2222_3333_4444);
    }
    debug_print("[NOTES_APP] Initialized private memory signature at 0x00408000.\n");

    // Cooperative Yield to allow sibling (calc_app) to execute
    debug_print("[NOTES_APP] Yielding execution to sibling process...\n");
    yield_now();
    debug_print("[NOTES_APP] Resumed execution after cooperative yield.\n");

    // Verify memory signature is preserved (sibling write did not clobber it)
    let preserved_val = unsafe { core::ptr::read_volatile(mem_ptr) };
    if preserved_val == 0x1111_2222_3333_4444 {
        debug_print("[NOTES_APP] Memory signature preserved across context switch.\n");
    } else {
        debug_print("[NOTES_APP] ERROR: Memory signature corrupted across context switch!\n");
        exit(15);
    }

    // 1c. Compositor IPC Integration
    let shell = match AppShell::new(620, 400, "Notes - INOS Text Studio") {
        Ok(s) => {
            let _ = draw_text(s.surface_id, 14, 38, 0x00FFFFFF, "Notes: Milestone APP-CAP-01");
            s.present();
            debug_print("[NOTES_APP] Compositor IPC surface registered and rendered.\n");
            s
        }
        Err(_) => exit(2),
    };

    // Check if running in post-reboot verification mode
    let mut check_buf = [0u8; 256];
    let mut initial_read_len = 0usize;
    let is_post_reboot = if let Ok(tok) = request_capability("notes_output.txt", rights::READ) {
        if let Ok(n) = file_read("notes_output.txt", &mut check_buf, tok) {
            initial_read_len = n;
            true
        } else {
            false
        }
    } else {
        false
    };

    if is_post_reboot {
        debug_print("[NOTES_APP] Cold reboot detected: 'notes_output.txt' found on disk. Initiating Post-Reboot Replay Suppression Protocol...\n");
        let cmd_replay = b"REQ:1001|OP:calc.eval|EXPR:42*1024|CALLER:100";
        let _ = ipc_send(200, 5, cmd_replay);
        debug_print("[NOTES_APP] Dispatched replayed REQ:1001 to PID 200.\n");
        yield_now();

        let mut resp_buf = [0u8; 128];
        if let Ok((sender, _mtype, rlen)) = ipc_recv(&mut resp_buf) {
            let resp_str = core::str::from_utf8(&resp_buf[..rlen]).unwrap_or("");
            if sender == 200
                && resp_str.contains("RESP:1001")
                && resp_str.contains("STATUS:DUPLICATE_CACHED")
                && resp_str.contains("RESULT:43008")
            {
                debug_print("[NOTES_APP] Post-Reboot Replay Protection Verified: Received DUPLICATE_CACHED for replayed REQ:1001.\n");
                debug_print("[POST_REBOOT_REPLAY_SUPPRESSION_VERIFIED_MARKER_v1.0]\n");
                debug_print("[NOTES_APP] Post-reboot replay verification finished cleanly. Exiting.\n");
                exit(0);
            } else {
                debug_print("[NOTES_APP_ERROR] Post-reboot duplicate response mismatch!\n");
                exit(90);
            }
        }
    }

    // =========================================================================
    // TYPED IPC APPLICATION EXECUTION & CORRELATED LEDGER PROTOCOL
    // =========================================================================
    debug_print("[NOTES_APP] === Initiating Typed IPC Application Execution Protocol ===\n");

    let mut empty_check_buf = [0u8; 128];
    let empty_res = ipc_recv(&mut empty_check_buf);
    if empty_res == Err(SyscallError::QueueEmpty) {
        debug_print("[NOTES_APP] IPC Negative 1: QueueEmpty returned cleanly on empty inbox.\n");
    } else {
        debug_print("[NOTES_APP_ERROR] IPC Negative 1 failed!\n");
        exit(20);
    }

    let unroutable_res = ipc_send(999, 5, b"REQ:999|OP:ping");
    if unroutable_res == Err(SyscallError::ObjectNotFound) {
        debug_print("[NOTES_APP] IPC Negative 2: Unroutable PID 999 rejected with ObjectNotFound.\n");
    } else {
        debug_print("[NOTES_APP_ERROR] IPC Negative 2 failed!\n");
        exit(21);
    }

    let cmd1 = b"REQ:1001|OP:calc.eval|EXPR:42*1024|CALLER:spoofed_admin_0";
    if ipc_send(200, 5, cmd1).is_err() {
        debug_print("[NOTES_APP_ERROR] Failed to send REQ:1001 to PID 200!\n");
        exit(22);
    }
    debug_print("[NOTES_APP] IPC Step 3: Dispatched correlated command REQ:1001 to PID 200.\n");
    yield_now();

    let mut resp_buf = [0u8; 128];
    if let Ok((sender, mtype, rlen)) = ipc_recv(&mut resp_buf) {
        let resp_str = core::str::from_utf8(&resp_buf[..rlen]).unwrap_or("");
        if sender == 200
            && mtype == 6
            && resp_str.contains("RESP:1001")
            && resp_str.contains("STATUS:OK")
            && resp_str.contains("RESULT:43008")
        {
            debug_print("[NOTES_APP] IPC Step 3: Correlated response RESP:1001 verified (result=43008).\n");
            debug_print("[NOTES_APP] Ledger: Committed verified action receipt for REQ:1001.\n");
        } else {
            debug_print("[NOTES_APP_ERROR] Correlated response verification failed!\n");
            exit(23);
        }

        // Negative Test 4: Caller Authorization / Permissions Rejection (REQ:1002)
        let cmd_unauth = b"REQ:1002|OP:calc.admin_reset|EXPR:0|CALLER:100";
        let _ = ipc_send(200, 5, cmd_unauth);
        debug_print("[NOTES_APP] IPC Step 4: Dispatched unauthorized operation REQ:1002.\n");
        yield_now();

        match ipc_recv(&mut resp_buf) {
            Ok((sender, _mtype, rlen)) => {
                let resp_str = core::str::from_utf8(&resp_buf[..rlen]).unwrap_or("");
                if sender == 200 && resp_str.contains("RESP:1002") && resp_str.contains("STATUS:AUTH_DENIED") {
                    debug_print("[NOTES_APP] IPC Negative 4: Caller authorization enforcement confirmed (AUTH_DENIED).\n");
                } else {
                    debug_print("[NOTES_APP_ERROR] Caller authorization check failed!\n");
                    exit(25);
                }
            }
            Err(_) => exit(26),
        }

        // Negative Test 5: Malformed Message Handling (REQ:1003)
        let cmd_malformed = b"MALFORMED_GARBAGE_WITHOUT_DELIMITERS";
        let _ = ipc_send(200, 5, cmd_malformed);
        debug_print("[NOTES_APP] IPC Step 5: Dispatched malformed payload to PID 200.\n");
        yield_now();

        match ipc_recv(&mut resp_buf) {
            Ok((sender, _mtype, rlen)) => {
                let resp_str = core::str::from_utf8(&resp_buf[..rlen]).unwrap_or("");
                if sender == 200 && resp_str.contains("STATUS:MALFORMED") {
                    debug_print("[NOTES_APP] IPC Negative 5: Malformed payload handled gracefully by target application.\n");
                } else {
                    debug_print("[NOTES_APP_ERROR] Malformed payload check failed!\n");
                    exit(27);
                }
            }
            Err(_) => exit(28),
        }

        // Negative Test: Conflicting Payload Detection on duplicate REQ:1001
        let cmd_conflict = b"REQ:1001|OP:calc.eval|EXPR:999*999|CALLER:100";
        let _ = ipc_send(200, 5, cmd_conflict);
        debug_print("[NOTES_APP] IPC Step: Dispatched conflicting payload REQ:1001 (EXPR:999*999) to PID 200.\n");
        yield_now();

        match ipc_recv(&mut resp_buf) {
            Ok((sender, _mtype, rlen)) => {
                let resp_str = core::str::from_utf8(&resp_buf[..rlen]).unwrap_or("");
                if sender == 200 && resp_str.contains("RESP:1001") && resp_str.contains("STATUS:PAYLOAD_CONFLICT") {
                    debug_print("[NOTES_APP] IPC Negative: Conflicting payload detected and rejected with PAYLOAD_CONFLICT.\n");
                } else {
                    debug_print("[NOTES_APP_ERROR] Conflicting payload check failed!\n");
                    exit(28);
                }
            }
            Err(_) => exit(29),
        }

        // Negative Test 6: Duplicate Request / Idempotency Check
        let cmd_dup = b"REQ:1001|OP:calc.eval|EXPR:42*1024|CALLER:100";
        let _ = ipc_send(200, 5, cmd_dup);
        debug_print("[NOTES_APP] IPC Step 6: Dispatched duplicate request REQ:1001 to test idempotency.\n");
        yield_now();

        match ipc_recv(&mut resp_buf) {
            Ok((sender, _mtype, rlen)) => {
                let resp_str = core::str::from_utf8(&resp_buf[..rlen]).unwrap_or("");
                if sender == 200 && resp_str.contains("RESP:1001") && resp_str.contains("STATUS:DUPLICATE_CACHED") {
                    debug_print("[NOTES_APP] IPC Negative 6: Idempotent duplicate request recognized and suppressed.\n");
                } else {
                    debug_print("[NOTES_APP_ERROR] Duplicate request check failed!\n");
                    exit(29);
                }
            }
            Err(_) => exit(30),
        }

        // Positive Test 7: Scoped Operation Idempotency Check
        let cmd_distinct_op = b"REQ:1001|OP:calc.verify|EXPR:42*1024";
        let _ = ipc_send(200, 5, cmd_distinct_op);
        debug_print("[NOTES_APP] IPC Step 7: Dispatched REQ:1001 with distinct operation 'calc.verify'.\n");
        yield_now();

        match ipc_recv(&mut resp_buf) {
            Ok((sender, _mtype, rlen)) => {
                let resp_str = core::str::from_utf8(&resp_buf[..rlen]).unwrap_or("");
                if sender == 200 && resp_str.contains("RESP:1001") && resp_str.contains("STATUS:OK") && resp_str.contains("RESULT:VERIFIED") {
                    debug_print("[NOTES_APP] IPC Step 7: Scoped operation check confirmed: distinct operation 'calc.verify' executed under same req_id 1001.\n");
                } else {
                    debug_print("[NOTES_APP_ERROR] Scoped operation check failed!\n");
                    exit(31);
                }
            }
            Err(_) => exit(32),
        }

        debug_print("[IPC_FULL_ROUNDTRIP_EXECUTION_VERIFIED_MARKER_v1.0]\n");
    }

    // 2. Negative Security Test: Write without capability token
    let denied1 = file_write("research.txt", b"unauthorized_payload", 0);
    if denied1.is_err() {
        debug_print("[NOTES_APP] Security Check 1: Unauthorized write without token rejected.\n");
    } else {
        debug_print("[NOTES_APP] ERROR: Unauthorized write succeeded!\n");
        exit(2);
    }

    // 3. Negative Security Test: Write with forged capability token (0xDEADBEEF)
    let denied2 = file_write("notes_output.txt", b"forged_payload", 0xDEADBEEF);
    if denied2.is_err() {
        debug_print("[NOTES_APP] Security Check 2: Forged token 0xDEADBEEF rejected.\n");
    } else {
        debug_print("[NOTES_APP] ERROR: Forged token write succeeded!\n");
        exit(3);
    }

    // 4. Scoped Capability Request: Request write permission for notes_output.txt
    debug_print("[NOTES_APP] Requesting scoped capability grant for 'notes_output.txt'...\n");
    let token = match request_capability("notes_output.txt", rights::READ | rights::WRITE | rights::CREATE) {
        Ok(t) => {
            debug_print("[NOTES_APP] Capability grant issued by trusted OS policy.\n");
            t
        }
        Err(_) => {
            debug_print("[NOTES_APP] ERROR: OS policy denied capability request!\n");
            exit(4);
        }
    };

    // 5. Negative Security Test: Cross-path tampering with valid token
    let denied3 = file_write(".workspace", b"malicious_overwrite", token);
    if denied3.is_err() {
        debug_print("[NOTES_APP] Security Check 3: Scoped token rejected for cross-path target '.workspace'.\n");
    } else {
        debug_print("[NOTES_APP] ERROR: Cross-path token tampering succeeded!\n");
        exit(5);
    }

    // 6. Authorized Operation: Create file 'notes_output.txt'
    let default_payload = b"INOS Native Application Foundry v1.0 - NotesApp Persistent Payload - Milestone APP-CAP-00";
    let mut editor_buf = [0u8; 256];
    let mut editor_len: usize;

    if initial_read_len > 0 {
        editor_buf[..initial_read_len].copy_from_slice(&check_buf[..initial_read_len]);
        editor_len = initial_read_len;
    } else {
        if file_create("notes_output.txt", token).is_err() {
            debug_print("[NOTES_APP] ERROR: Failed to create authorized file!\n");
            exit(6);
        }
        debug_print("[NOTES_APP] Authorized file 'notes_output.txt' created.\n");

        if file_write("notes_output.txt", default_payload, token).is_err() {
            debug_print("[NOTES_APP] ERROR: Failed to write payload to authorized file!\n");
            exit(7);
        }
        debug_print("[NOTES_APP] Committed payload to INOS-FS with persistent WAL.\n");
        debug_print("[APP_CAP_00_FILE_COMMITTED_MARKER_v1.0]\n");
        editor_buf[..default_payload.len()].copy_from_slice(default_payload);
        editor_len = default_payload.len();
    }

    let mut status_msg = "Type text | [F2] Save to Disk | [F3] Reload | [F4] Copy Clipboard | [Esc] Exit";

    // 7. Interactive Notes Editor Loop
    shell.run_split_loop(|sh, ev_opt: Option<RawInputEvent>| -> bool {
        if let Some(ev) = ev_opt {
            match ev.event_type {
                InputEventType::KeyDown => {
                    let ch = (ev.arg1 & 0xFF) as u8;
                    let sc = ev.arg2;
                    if sc == KEY_F2 {
                        let _ = file_write("notes_output.txt", &editor_buf[..editor_len], token);
                        status_msg = "Saved notes_output.txt to persistent INOS-FS.";
                    } else if sc == KEY_F3 {
                        if let Ok(n) = file_read("notes_output.txt", &mut editor_buf, token) {
                            editor_len = core::cmp::min(n, 256);
                            status_msg = "Reloaded notes_output.txt from INOS-FS.";
                        }
                    } else if sc == KEY_F4 {
                        if let Ok(clip_tok) = request_capability("clipboard", rights::CLIPBOARD_WRITE) {
                            let clip_len = core::cmp::min(editor_len, 64);
                            let _ = set_clipboard(sh.surface_id, clip_tok, &editor_buf[..clip_len]);
                            status_msg = "Copied note selection to system clipboard.";
                        }
                    } else if sc == KEY_BACKSPACE {
                        if editor_len > 0 {
                            editor_len -= 1;
                            status_msg = "Modified (Press F2 to Save)";
                        }
                    } else if sc == KEY_ENTER {
                        if editor_len < 255 {
                            editor_buf[editor_len] = b'\n';
                            editor_len += 1;
                            status_msg = "Modified (Press F2 to Save)";
                        }
                    } else if ch >= 0x20 && ch <= 0x7E && editor_len < 255 {
                        editor_buf[editor_len] = ch;
                        editor_len += 1;
                        status_msg = "Modified (Press F2 to Save)";
                    }
                }
                InputEventType::MouseDown => {
                    let mx = ev.arg1;
                    let my = ev.arg2;
                    if my >= 36 && my <= 62 {
                        if mx >= 14 && mx <= 124 {
                            let _ = file_write("notes_output.txt", &editor_buf[..editor_len], token);
                            status_msg = "Saved notes_output.txt to INOS-FS.";
                        } else if mx >= 132 && mx <= 242 {
                            if let Ok(n) = file_read("notes_output.txt", &mut editor_buf, token) {
                                editor_len = core::cmp::min(n, 256);
                                status_msg = "Reloaded notes_output.txt from disk.";
                            }
                        } else if mx >= 250 && mx <= 360 {
                            editor_len = 0;
                            status_msg = "Buffer cleared (Press F2 to Save, F3 to Undo).";
                        }
                    }
                }
                _ => {}
            }
        }
        if let Some((_sender, _op, _wire)) = sh.poll_ambient_directive() {
            status_msg = "Synced live evaluation receipt from bound Calculator/Web surface (BoundLiveSync).";
        }

        let _ = sh.clear(0x0F172A);
        let _ = sh.draw_button(14, 36, 110, 26, "[F2] Save", false);
        let _ = sh.draw_button(132, 36, 110, 26, "[F3] Reload", false);
        let _ = sh.draw_button(250, 36, 110, 26, "Clear Buffer", false);
        let _ = draw_text(sh.surface_id, 380, 42, 0x94A3B8, "File: notes_output.txt");

        // Editor Document Surface
        let _ = fill_rect(sh.surface_id, 14, 72, 592, 290, 0x1E293B);
        let text_str = core::str::from_utf8(&editor_buf[..editor_len]).unwrap_or("");
        let mut row = 0u32;
        for line in text_str.split('\n') {
            let mut pos = 0;
            if line.is_empty() {
                row += 1;
                continue;
            }
            while pos < line.len() && row < 11 {
                let end = core::cmp::min(pos + 62, line.len());
                let _ = draw_text(sh.surface_id, 24, 84 + row * 22, 0xF8FAFC, &line[pos..end]);
                pos = end;
                row += 1;
            }
        }
        let _ = sh.draw_status_bar(status_msg);
        false
    });

    debug_print("[NOTES_APP] Workflow completed successfully. Exiting cleanly.\n");
    debug_print("[APP_CAP_00_APP_CLEAN_EXIT_MARKER_v1.0]\n");
    exit(0);
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    debug_print("[NOTES_APP] Panic occurred in Ring 3 application!\n");
    exit(101);
}
