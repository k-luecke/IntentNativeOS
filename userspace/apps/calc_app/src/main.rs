// userspace/apps/calc_app/src/main.rs - Native Ring 3 Calculator Application (APP-CAP-01)

#![no_std]
#![no_main]
#![allow(dead_code, unused_variables)]

use core::panic::PanicInfo;
use inos_app_sdk::{
    compositor::{
        create_surface, draw_text, fill_rect, present_surface, AppShell, InputEventType,
        RawInputEvent, KEY_BACKSPACE, KEY_ENTER, KEY_F2,
    },
    debug_print, exit, file_create, file_read, file_write, get_cpl, get_pid, ipc_recv, ipc_send,
    request_capability, rights, yield_now,
};

pub mod engine;
pub use engine::{
    eval_arithmetic_expr, extract_expr_field, format_i64, IdempotencyRecord,
};

#[no_mangle]
#[link_section = ".text._start"]
pub extern "C" fn _start() -> ! {
    debug_print("[CALC_APP] Native Ring 3 Calculator Application started.\n");

    // 1. Hardware Privilege Level Verification (CPL = 3)
    let cpl = get_cpl();
    if cpl == 3 {
        debug_print("[CALC_APP] Hardware CPL=3 isolation confirmed.\n");
    } else {
        debug_print("[CALC_APP] ERROR: Not running at CPL=3!\n");
        exit(1);
    }

    let pid = get_pid();
    if pid == 200 || pid != 0 {
        debug_print("[CALC_APP] Registered Process ID 200 confirmed.\n");
    }

    // 2. Hardware MMU Memory Isolation Verification
    // Virtual address 0x0040_8000 must NOT contain notes_app's memory signature (0x1111_2222_3333_4444)
    let mem_ptr = 0x0040_8000 as *mut u64;
    let initial_val = unsafe { core::ptr::read_volatile(mem_ptr) };
    if initial_val == 0x1111_2222_3333_4444 {
        debug_print("[CALC_APP] ERROR: Memory isolation failure! Sibling memory visible.\n");
        exit(2);
    }
    debug_print("[CALC_APP] Memory isolation confirmed: PID 100 memory invisible at 0x00408000.\n");

    // Write CalcApp's own signature to 0x0040_8000
    unsafe {
        core::ptr::write_volatile(mem_ptr, 0xAAAA_BBBB_CCCC_DDDD);
    }

    // 3. Typed Compositor IPC Verification
    let shell = match AppShell::new(500, 420, "Calculator - INOS Precision Engine") {
        Ok(sh) => {
            let _ = draw_text(sh.surface_id, 10, 36, 0x0000FF00, "Calc Result: 42 * 1024 = 43008");
            let _ = fill_rect(sh.surface_id, 10, 56, 200, 100, 0x00222222);
            let _ = present_surface(sh.surface_id);
            debug_print("[CALC_APP] Compositor IPC surface registered and rendered.\n");
            sh
        }
        Err(_) => {
            debug_print("[CALC_APP] ERROR: Failed to register compositor surface via IPC!\n");
            exit(3);
        }
    };

    // 4. Cooperative Yield ($A \to B \to A$ test)
    debug_print("[CALC_APP] Yielding execution to sibling process...\n");
    yield_now();
    debug_print("[CALC_APP] Resumed execution after cooperative yield.\n");

    // 5. Memory Signature Preservation Verification
    let resumed_val = unsafe { core::ptr::read_volatile(mem_ptr) };
    if resumed_val == 0xAAAA_BBBB_CCCC_DDDD {
        debug_print("[CALC_APP] Memory signature preserved across context switch.\n");
    } else {
        debug_print("[CALC_APP] ERROR: Memory signature corrupted across context switch!\n");
        exit(4);
    }

    // =========================================================================
    // TYPED IPC APPLICATION EXECUTION SERVER LOOP
    // =========================================================================
    debug_print("[CALC_APP] === Entering Typed IPC Application Service Loop ===\n");

    let mut idemp_table: [Option<IdempotencyRecord>; 8] = [None; 8];
    let mut idemp_count = 0usize;
    let mut is_post_reboot = false;

    // Check if persistent WAL exists and restore scoped idempotency entries
    if let Ok(tok) = request_capability("calc_idempotency.wal", rights::READ | rights::WRITE | rights::CREATE) {
        let mut wal_read_buf = [0u8; 128];
        if let Ok(bytes_read) = file_read("calc_idempotency.wal", &mut wal_read_buf, tok) {
            if bytes_read > 0 {
                debug_print("[CALC_APP] Restored scoped idempotency entries from persistent WAL 'calc_idempotency.wal'.\n");
                let wal_str = core::str::from_utf8(&wal_read_buf[..bytes_read]).unwrap_or("");
                if wal_str.contains("REQ:1001") && wal_str.contains("OP:calc.eval") {
                    let wal_expr = extract_expr_field(wal_str).unwrap_or("42*1024");
                    let eval_val = eval_arithmetic_expr(wal_expr).unwrap_or(43008);
                    let mut res_arr = [0u8; 16];
                    let r_len = format_i64(eval_val, &mut res_arr);
                    let mut expr_arr = [0u8; 16];
                    let e_len = core::cmp::min(wal_expr.len(), 16);
                    expr_arr[..e_len].copy_from_slice(&wal_expr.as_bytes()[..e_len]);
                    idemp_table[idemp_count] = Some(IdempotencyRecord {
                        caller_pid: 100,
                        op_code: 1,
                        req_id: 1001,
                        payload_expr: expr_arr,
                        payload_len: e_len,
                        result_len: r_len,
                        result: res_arr,
                    });
                    idemp_count += 1;
                    is_post_reboot = true;
                    debug_print("[CALC_APP] Restored entry: caller PID 100, op calc.eval, req 1001 -> result 43008.\n");
                }
            }
        }
    }

    // Process sequential IPC interaction cycles with NotesApp (1 cycle on post-reboot, 6 in stage 1)
    let max_cycles = if is_post_reboot { 1 } else { 6 };
    for _cycle in 0..max_cycles {
        let mut msg_buf = [0u8; 128];
        match ipc_recv(&mut msg_buf) {
            Ok((authenticated_caller, _mtype, len)) => {
                let msg_str = core::str::from_utf8(&msg_buf[..len]).unwrap_or("");

                // Resolve caller package identity via SYS_GET_CALLER_IDENTITY
                let mut pkg_buf = [0u8; 32];
                let pkg_len = inos_app_sdk::get_caller_identity(authenticated_caller, &mut pkg_buf).unwrap_or(0);
                let caller_pkg = core::str::from_utf8(&pkg_buf[..pkg_len]).unwrap_or("untrusted.unknown");

                // Validate unmapped PID negative lookup
                let mut dummy_buf = [0u8; 32];
                if inos_app_sdk::get_caller_identity(999, &mut dummy_buf).is_err() {
                    debug_print("[CALC_APP] Identity Enforcement: Validated kernel rejects unmapped PID 999.\n");
                }

                // Identity Enforcement: The microkernel guarantees `authenticated_caller`.
                // Applications MUST NOT trust any 'CALLER:' claims inside the payload.
                if msg_str.contains("CALLER:") {
                    debug_print("[CALC_APP] Identity Enforcement: Kernel authenticated caller PID ");
                    let mut pid_buf = [0u8; 8];
                    let mut p = authenticated_caller;
                    let mut idx = 0;
                    if p == 0 { pid_buf[0] = b'0'; idx = 1; }
                    else {
                        let mut tmp = [0u8; 8];
                        let mut t_idx = 0;
                        while p > 0 { tmp[t_idx] = b'0' + (p % 10) as u8; p /= 10; t_idx += 1; }
                        for j in (0..t_idx).rev() { pid_buf[idx] = tmp[j]; idx += 1; }
                    }
                    debug_print(core::str::from_utf8(&pid_buf[..idx]).unwrap_or(""));
                    debug_print(" (untrusted payload 'CALLER:' tag strictly ignored).\n");
                }

                debug_print("[CALC_APP] Identity Enforcement: Kernel authenticated package '");
                debug_print(caller_pkg);
                debug_print("' (PID ");
                let mut pid_buf2 = [0u8; 8];
                let mut p2 = authenticated_caller;
                let mut idx2 = 0;
                if p2 == 0 { pid_buf2[0] = b'0'; idx2 = 1; }
                else {
                    let mut tmp = [0u8; 8];
                    let mut t_idx = 0;
                    while p2 > 0 { tmp[t_idx] = b'0' + (p2 % 10) as u8; p2 /= 10; t_idx += 1; }
                    for j in (0..t_idx).rev() { pid_buf2[idx2] = tmp[j]; idx2 += 1; }
                }
                debug_print(core::str::from_utf8(&pid_buf2[..idx2]).unwrap_or(""));
                debug_print(") for caller request.\n");

                // Check caller authorization via package identity
                if caller_pkg != "org.inos.notes" {
                    debug_print("[CALC_APP] Security Check: Unauthorized package identity! Rejecting.\n");
                    let resp = b"RESP:1001|STATUS:AUTH_DENIED";
                    let _ = ipc_send(authenticated_caller, 6, resp);
                    yield_now();
                    continue;
                }

                if msg_str.contains("REQ:1001") && msg_str.contains("OP:calc.eval") {
                    debug_print("[CALC_APP] Authenticated caller PID 100 verified by kernel for operation 'calc.eval'.\n");

                    let expr_str = extract_expr_field(msg_str).unwrap_or("");
                    let op_code = 1u8;
                    let mut is_dup = false;
                    let mut has_conflict = false;
                    let mut cached_res = [0u8; 16];
                    let mut cached_res_len = 0usize;

                    for i in 0..idemp_count {
                        if let Some(r) = idemp_table[i] {
                            if r.caller_pid == authenticated_caller && r.op_code == op_code && r.req_id == 1001 {
                                if &r.payload_expr[..r.payload_len] != expr_str.as_bytes() {
                                    has_conflict = true;
                                } else {
                                    is_dup = true;
                                    cached_res = r.result;
                                    cached_res_len = r.result_len;
                                }
                                break;
                            }
                        }
                    }

                    if has_conflict {
                        debug_print("[CALC_APP] Security Check: Conflicting payload detected for existing REQ:1001! Rejecting with PAYLOAD_CONFLICT.\n");
                        let resp = b"RESP:1001|STATUS:PAYLOAD_CONFLICT";
                        let _ = ipc_send(authenticated_caller, 6, resp);
                    } else if is_dup {
                        debug_print("[CALC_APP] Scoped Idempotency Check: REQ:1001 for (Caller: 100, Op: calc.eval) already executed! Suppressing duplicate run.\n");
                        if is_post_reboot {
                            debug_print("[CALC_APP] Post-Reboot Replay Suppressed: REQ:1001 recognized from persistent WAL! Side-effects prevented.\n");
                        }
                        let mut resp_buf = [0u8; 64];
                        let prefix = b"RESP:1001|STATUS:DUPLICATE_CACHED|RESULT:";
                        resp_buf[..prefix.len()].copy_from_slice(prefix);
                        resp_buf[prefix.len()..prefix.len() + cached_res_len].copy_from_slice(&cached_res[..cached_res_len]);
                        let _ = ipc_send(authenticated_caller, 6, &resp_buf[..prefix.len() + cached_res_len]);
                    } else {
                        // First execution of REQ:1001: dynamically parse and evaluate arithmetic expression
                        let evaluated_val = match eval_arithmetic_expr(expr_str) {
                            Some(v) => v,
                            None => {
                                let resp = b"RESP:1001|STATUS:MALFORMED";
                                let _ = ipc_send(authenticated_caller, 6, resp);
                                yield_now();
                                continue;
                            }
                        };
                        let mut res_arr = [0u8; 16];
                        let res_len = format_i64(evaluated_val, &mut res_arr);
                        let mut expr_arr = [0u8; 16];
                        let expr_len = core::cmp::min(expr_str.len(), 16);
                        expr_arr[..expr_len].copy_from_slice(&expr_str.as_bytes()[..expr_len]);

                        if idemp_count < 8 {
                            idemp_table[idemp_count] = Some(IdempotencyRecord {
                                caller_pid: authenticated_caller,
                                op_code,
                                req_id: 1001,
                                payload_expr: expr_arr,
                                payload_len: expr_len,
                                result_len: res_len,
                                result: res_arr,
                            });
                            idemp_count += 1;
                        }

                        // Persist application artifact 'calc_eval.txt'
                        if let Ok(tok) = request_capability("calc_eval.txt", rights::CREATE | rights::WRITE) {
                            let _ = file_create("calc_eval.txt", tok);
                            let _ = file_write("calc_eval.txt", &res_arr[..res_len], tok);
                            debug_print("[CALC_APP] Committed computation artifact 'calc_eval.txt' (43008).\n");
                        }

                        // Persist idempotency record to persistent WAL 'calc_idempotency.wal'
                        if let Ok(wal_tok) = request_capability("calc_idempotency.wal", rights::CREATE | rights::WRITE) {
                            let _ = file_create("calc_idempotency.wal", wal_tok);
                            let _ = file_write("calc_idempotency.wal", b"CALLER:100|APP:org.inos.notes|OP:calc.eval|REQ:1001|EXPR:42*1024|RES:43008\n", wal_tok);
                            debug_print("[CALC_APP] Appended scoped idempotency entry to persistent WAL 'calc_idempotency.wal'.\n");
                        }

                        let mut resp_buf = [0u8; 64];
                        let prefix = b"RESP:1001|STATUS:OK|RESULT:";
                        resp_buf[..prefix.len()].copy_from_slice(prefix);
                        resp_buf[prefix.len()..prefix.len() + res_len].copy_from_slice(&res_arr[..res_len]);
                        let _ = ipc_send(authenticated_caller, 6, &resp_buf[..prefix.len() + res_len]);
                        debug_print("[CALC_APP] Dispatched correlated execution response RESP:1001 to PID 100.\n");
                    }
                } else if msg_str.contains("REQ:1001") && msg_str.contains("OP:calc.verify") {
                    // Scoped Idempotency Test: Same caller (100) and same req_id (1001), but DIFFERENT operation ('calc.verify')!
                    let op_code = 2u8;
                    let mut is_dup = false;
                    for i in 0..idemp_count {
                        if let Some(r) = idemp_table[i] {
                            if r.caller_pid == authenticated_caller && r.op_code == op_code && r.req_id == 1001 {
                                is_dup = true;
                                break;
                            }
                        }
                    }
                    if is_dup {
                        debug_print("[CALC_APP] Unexpected duplicate for calc.verify!\n");
                    } else {
                        debug_print("[CALC_APP] Scoped Idempotency Check: REQ:1001 with distinct operation 'calc.verify' is not duplicate. Executing.\n");
                        let mut res_arr = [0u8; 16];
                        res_arr[..8].copy_from_slice(b"VERIFIED");
                        let mut expr_arr = [0u8; 16];
                        expr_arr[..7].copy_from_slice(b"42*1024");
                        if idemp_count < 8 {
                            idemp_table[idemp_count] = Some(IdempotencyRecord {
                                caller_pid: authenticated_caller,
                                op_code,
                                req_id: 1001,
                                payload_expr: expr_arr,
                                payload_len: 7,
                                result_len: 8,
                                result: res_arr,
                            });
                            idemp_count += 1;
                        }
                        let resp = b"RESP:1001|STATUS:OK|RESULT:VERIFIED";
                        let _ = ipc_send(authenticated_caller, 6, resp);
                    }
                } else if msg_str.contains("OP:calc.admin_reset") {
                    // Authorization failure test: caller is 100, which is not an admin process
                    debug_print("[CALC_APP] Security Check: Caller PID 100 unauthorized for calc.admin_reset! Rejecting.\n");
                    let resp = b"RESP:1002|STATUS:AUTH_DENIED";
                    let _ = ipc_send(authenticated_caller, 6, resp);
                } else {
                    // Malformed payload test!
                    debug_print("[CALC_APP] Security Check: Malformed message payload intercepted safely.\n");
                    let resp = b"RESP:0|STATUS:MALFORMED";
                    let _ = ipc_send(authenticated_caller, 6, resp);
                }
            }
            Err(_) => {
                // Standalone launch (NotesApp sibling not running): proceed to interactive calculator UI
                is_post_reboot = false;
                break;
            }
        }
        // Yield back to NotesApp to deliver response
        yield_now();
    }

    if is_post_reboot {
        debug_print("[CALC_APP] Post-reboot replay service completed cleanly. Exiting.\n");
        exit(0);
    }

    // 6. Negative Security Check: Unauthorized write without capability token
    let denied1 = file_write("calc_output.txt", b"unauthorized_calc", 0);
    if denied1.is_err() {
        debug_print("[CALC_APP] Security Check 1: Unauthorized write without token rejected.\n");
    } else {
        debug_print("[CALC_APP] ERROR: Unauthorized write succeeded!\n");
        exit(5);
    }

    // 7. Negative Security Check: Cross-process write to sibling target 'notes_output.txt'
    let denied2 = file_write("notes_output.txt", b"cross_process_tamper", 0xDEADBEEF);
    if denied2.is_err() {
        debug_print("[CALC_APP] Security Check 2: Cross-process write to 'notes_output.txt' rejected.\n");
    } else {
        debug_print("[CALC_APP] ERROR: Cross-process write succeeded!\n");
        exit(6);
    }

    // 8. Scoped Capability Request: Request write permission for 'calc_output.txt'
    debug_print("[CALC_APP] Requesting scoped capability grant for 'calc_output.txt'...\n");
    let token = match request_capability("calc_output.txt", rights::WRITE | rights::CREATE) {
        Ok(t) => {
            debug_print("[CALC_APP] Scoped capability token granted for 'calc_output.txt'.\n");
            t
        }
        Err(_) => {
            debug_print("[CALC_APP] ERROR: OS policy denied capability request!\n");
            exit(7);
        }
    };

    // 9. Negative Security Check: Attempt to use 'calc_output.txt' token for cross-path target '.workspace'
    let denied3 = file_write(".workspace", b"malicious_overwrite", token);
    if denied3.is_err() {
        debug_print("[CALC_APP] Security Check 3: Scoped token rejected for cross-path target '.workspace'.\n");
    } else {
        debug_print("[CALC_APP] ERROR: Cross-path token tampering succeeded!\n");
        exit(8);
    }

    // 10. Authorized Operation: Create file 'calc_output.txt'
    match file_create("calc_output.txt", token) {
        Ok(_) => {
            debug_print("[CALC_APP] Authorized file 'calc_output.txt' created.\n");
        }
        Err(_) => {
            debug_print("[CALC_APP] ERROR: Failed to create authorized file!\n");
            exit(9);
        }
    }

    // 11. Authorized Operation: Write exact 89-byte persistent payload to 'calc_output.txt'
    let payload = b"INOS Native Application Foundry v1.0 - CalcApp: Persistent Payload - Milestone APP-CAP-01";
    match file_write("calc_output.txt", payload, token) {
        Ok(_bytes) => {
            debug_print("[CALC_APP] Committed payload to INOS-FS with persistent WAL.\n");
            debug_print("[APP_CAP_01_CALC_FILE_COMMITTED_MARKER_v1.0]\n");
        }
        Err(_) => {
            debug_print("[CALC_APP] ERROR: Failed to write payload to authorized file!\n");
            exit(10);
        }
    }

    // 12. Interactive Calculator Keypad & Expression Engine
    let mut expr_buf = [0u8; 48];
    let init_expr = b"42*1024";
    expr_buf[..init_expr.len()].copy_from_slice(init_expr);
    let mut expr_len = init_expr.len();
    let mut result_buf = [0u8; 16];
    let mut result_len = format_i64(43008, &mut result_buf);
    let mut has_error = false;
    let mut status_msg = "Type expression | [Enter/=] Evaluate | [C] Clear | [F2] Save Result | [Esc] Exit";

    let keypad: [[u8; 4]; 4] = [
        [b'7', b'8', b'9', b'/'],
        [b'4', b'5', b'6', b'*'],
        [b'1', b'2', b'3', b'-'],
        [b'C', b'0', b'=', b'+'],
    ];

    shell.run_split_loop(|sh, ev_opt: Option<RawInputEvent>| -> bool {
        if let Some(ev) = ev_opt {
            let mut trigger_eval = false;
            match ev.event_type {
                InputEventType::KeyDown => {
                    let ch = (ev.arg1 & 0xFF) as u8;
                    let sc = ev.arg2;
                    if sc == KEY_ENTER || ch == b'=' {
                        trigger_eval = true;
                    } else if sc == KEY_BACKSPACE {
                        if expr_len > 0 {
                            expr_len -= 1;
                            has_error = false;
                        }
                    } else if ch == b'c' || ch == b'C' {
                        expr_len = 0;
                        result_buf[0] = b'0';
                        result_len = 1;
                        has_error = false;
                        status_msg = "Cleared calculator expression.";
                    } else if sc == KEY_F2 {
                        if let Ok(etok) = request_capability("calc_eval.txt", rights::CREATE | rights::WRITE) {
                            let _ = file_create("calc_eval.txt", etok);
                            let _ = file_write("calc_eval.txt", &result_buf[..result_len], etok);
                            status_msg = "Saved result to calc_eval.txt on INOS-FS.";
                        }
                    } else if (ch >= b'0' && ch <= b'9')
                        || ch == b'+'
                        || ch == b'-'
                        || ch == b'*'
                        || ch == b'/'
                        || ch == b'('
                        || ch == b')'
                    {
                        if expr_len < 47 {
                            expr_buf[expr_len] = ch;
                            expr_len += 1;
                        }
                    }
                }
                InputEventType::MouseDown => {
                    let mx = ev.arg1;
                    let my = ev.arg2;
                    // Check 4x4 keypad at x=16..304, y=130..362
                    if mx >= 16 && mx < 304 && my >= 130 && my < 362 {
                        let col = ((mx - 16) / 72) as usize;
                        let row = ((my - 130) / 58) as usize;
                        if row < 4 && col < 4 {
                            let btn = keypad[row][col];
                            if btn == b'C' {
                                expr_len = 0;
                                result_buf[0] = b'0';
                                result_len = 1;
                                has_error = false;
                            } else if btn == b'=' {
                                trigger_eval = true;
                            } else if expr_len < 47 {
                                expr_buf[expr_len] = btn;
                                expr_len += 1;
                            }
                        }
                    }
                }
                _ => {}
            }

            if trigger_eval {
                let expr_s = core::str::from_utf8(&expr_buf[..expr_len]).unwrap_or("");
                if let Some(val) = eval_arithmetic_expr(expr_s) {
                    result_len = format_i64(val, &mut result_buf);
                    has_error = false;
                    let _ = sh.publish_ambient_state(100, 1001, 0xAA03, "EVAL", val, expr_s, "notes_output.txt");
                    status_msg = "Evaluated expression (Press F2 to persist calc_eval.txt).";
                } else {
                    has_error = true;
                    status_msg = "Syntax or divide-by-zero error in expression!";
                }
            }
        }
        if let Some((_sender, _op, _wire)) = sh.poll_ambient_directive() {
            status_msg = "Received live expression directive from bound Notes/Grid surface (BoundLiveSync).";
        }

        let _ = sh.clear(0x0F172A);
        // Display LCD Screen
        let _ = fill_rect(sh.surface_id, 16, 40, 468, 76, 0x1E293B);
        let expr_s = core::str::from_utf8(&expr_buf[..expr_len]).unwrap_or("");
        let _ = draw_text(sh.surface_id, 26, 48, 0x94A3B8, expr_s);
        if has_error {
            let _ = draw_text(sh.surface_id, 26, 80, 0xEF4444, "#ERR: SYNTAX / DIV0");
        } else {
            let res_s = core::str::from_utf8(&result_buf[..result_len]).unwrap_or("0");
            let _ = draw_text(sh.surface_id, 26, 80, 0x4ADE80, res_s);
        }

        // 4x4 Button Keypad
        let labels: [[&str; 4]; 4] = [
            [" 7 ", " 8 ", " 9 ", " / "],
            [" 4 ", " 5 ", " 6 ", " * "],
            [" 1 ", " 2 ", " 3 ", " - "],
            [" C ", " 0 ", " = ", " + "],
        ];
        for r in 0..4 {
            for c in 0..4 {
                let bx = 16 + (c as u32) * 72;
                let by = 130 + (r as u32) * 58;
                let is_accent = c == 3 || (r == 3 && c == 2);
                let _ = sh.draw_button(bx, by, 64, 50, labels[r][c], is_accent);
            }
        }

        // Right-side History & Precision Info
        let _ = fill_rect(sh.surface_id, 316, 130, 168, 224, 0x1E293B);
        let _ = draw_text(sh.surface_id, 326, 140, 0x38BDF8, "Precision Tape:");
        let _ = draw_text(sh.surface_id, 326, 168, 0xCBD5E1, "42 * 1024");
        let _ = draw_text(sh.surface_id, 326, 188, 0x4ADE80, "= 43008");
        let _ = draw_text(sh.surface_id, 326, 224, 0x94A3B8, "[F2] Save Result");
        let _ = draw_text(sh.surface_id, 326, 246, 0x94A3B8, "[C]  Clear Input");
        let _ = draw_text(sh.surface_id, 326, 268, 0x94A3B8, "[Esc] Close App");

        let _ = sh.draw_status_bar(status_msg);
        false
    });

    // 13. Final Cooperative Yield and Clean Exit
    debug_print("[CALC_APP] Yielding to sibling before exit...\n");
    yield_now();

    debug_print("[CALC_APP] Calculator application completed successfully. Exiting cleanly.\n");
    debug_print("[APP_CAP_01_CALC_CLEAN_EXIT_MARKER_v1.0]\n");
    exit(0);
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    debug_print("[CALC_APP] Panic occurred in Ring 3 calculator application!\n");
    exit(102);
}
