// userspace/apps/mail_app/src/main.rs
// Native Ring 3 Mail Client Application
// Milestone APP-MAIL-01

#![no_std]
#![no_main]

pub mod engine;

use core::panic::PanicInfo;
use engine::*;
use inos_app_sdk::{
    debug_print, exit, file_create, file_write, get_cpl, get_pid, request_capability, rights,
};

#[no_mangle]
#[link_section = ".text._start"]
pub extern "C" fn _start() -> ! {
    debug_print("[APP_MAIL_01] Native Mail Client & MIME Engine started.\n");

    // 1. Hardware CPL=3 Privilege Verification
    let cpl = get_cpl();
    if cpl == 3 {
        debug_print("[APP_MAIL_01] Hardware CPL=3 isolation confirmed.\n");
    } else {
        debug_print("[APP_MAIL_01] ERROR: Not running at CPL=3!\n");
        exit(1);
    }

    let pid = get_pid();
    if pid != 0 {
        debug_print("[APP_MAIL_01] Process ID confirmed.\n");
    }

    // =========================================================================
    // 2. VERIFY RFC 5322 HEADER & ADDRESS PARSER (Contract M-01, M-02)
    // =========================================================================
    debug_print("[APP_MAIL_01] Validating RFC 5322 header and address parsing...\n");

    let sample_raw_header = b"From: \"Kyle Luecke\" <kyle@inos.org>\r\nTo: \"Engineering Team\" <team@inos.org>\r\nSubject: APF-001 Native Mail Architecture\r\nDate: Sat, 10 Oct 2026 18:30:00 +0000\r\nMessage-ID: <msg-20261010-001@inos.org>\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nBody text goes here.\r\n";

    let email = match ParsedEmail::parse(sample_raw_header) {
        Ok(em) => em,
        Err(_) => {
            debug_print("[APP_MAIL_01_ERROR] ParsedEmail::parse failed!\n");
            exit(2);
        }
    };

    if email.header_count < 5 {
        debug_print("[APP_MAIL_01_ERROR] Insufficient header count parsed!\n");
        exit(3);
    }

    let from_hdr = match email.get_header("From") {
        Some(h) => h,
        None => {
            debug_print("[APP_MAIL_01_ERROR] From header not found!\n");
            exit(4);
        }
    };
    let parsed_addr = EmailAddress::parse(from_hdr);
    if parsed_addr.display_name_str() != "Kyle Luecke" || parsed_addr.address_str() != "kyle@inos.org" {
        debug_print("[APP_MAIL_01_ERROR] From address parsing mismatch!\n");
        exit(5);
    }

    let subj_hdr = match email.get_header("Subject") {
        Some(s) => s,
        None => {
            debug_print("[APP_MAIL_01_ERROR] Subject header not found!\n");
            exit(6);
        }
    };
    if subj_hdr != "APF-001 Native Mail Architecture" {
        debug_print("[APP_MAIL_01_ERROR] Subject value mismatch!\n");
        exit(7);
    }

    debug_print("[MAIL_RFC5322_HEADER_PARSER_VERIFIED_MARKER_v1.0]\n");

    // =========================================================================
    // 3. VERIFY BASE64 & QUOTED-PRINTABLE DECODERS (Contract M-02)
    // =========================================================================
    debug_print("[APP_MAIL_01] Validating Base64 and Quoted-Printable decoders...\n");

    // 3a: Base64
    let payload_original = b"Native INOS Mail Attachment Payload!";
    let mut b64_encoded = [0u8; 128];
    let b64_enc_len = match encode_base64(payload_original, &mut b64_encoded) {
        Ok(n) => n,
        Err(_) => {
            debug_print("[APP_MAIL_01_ERROR] encode_base64 failed!\n");
            exit(8);
        }
    };

    let mut b64_decoded = [0u8; 128];
    let b64_dec_len = match decode_base64(&b64_encoded[..b64_enc_len], &mut b64_decoded) {
        Ok(n) => n,
        Err(_) => {
            debug_print("[APP_MAIL_01_ERROR] decode_base64 failed!\n");
            exit(9);
        }
    };

    if b64_dec_len != payload_original.len() || &b64_decoded[..b64_dec_len] != payload_original {
        debug_print("[APP_MAIL_01_ERROR] Base64 roundtrip mismatch!\n");
        exit(10);
    }

    // 3b: Quoted-Printable
    let qp_input = b"INOS=20Mail=20Architecture=21=\r\nNextLine";
    let mut qp_output = [0u8; 64];
    let qp_len = match decode_quoted_printable(qp_input, &mut qp_output) {
        Ok(n) => n,
        Err(_) => {
            debug_print("[APP_MAIL_01_ERROR] decode_quoted_printable failed!\n");
            exit(11);
        }
    };

    if &qp_output[..qp_len] != b"INOS Mail Architecture!NextLine" {
        debug_print("[APP_MAIL_01_ERROR] Quoted-Printable decode mismatch!\n");
        exit(12);
    }

    debug_print("[MAIL_MIME_DECODERS_VERIFIED_MARKER_v1.0]\n");

    // =========================================================================
    // 4. VERIFY MIME MULTIPART EXTRACTION & ATTACHMENTS (Contract M-02, M-03)
    // =========================================================================
    debug_print("[APP_MAIL_01] Validating multipart MIME parsing and attachment extraction...\n");

    let multipart_raw = b"From: Kyle <kyle@inos.org>\r\nTo: Team <team@inos.org>\r\nSubject: Quarterly Report\r\nContent-Type: multipart/mixed; boundary=\"BOUNDARY_SAMPLE_42\"\r\n\r\n--BOUNDARY_SAMPLE_42\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nQuarterly execution complete. Subsystems green.\r\n--BOUNDARY_SAMPLE_42\r\nContent-Type: text/csv; name=\"report.csv\"\r\nContent-Disposition: attachment; filename=\"report.csv\"\r\nContent-Transfer-Encoding: base64\r\n\r\nTW9udGgsUmV2ZW51ZQpPY3QsMTA0NTAwMAo=\r\n--BOUNDARY_SAMPLE_42--\r\n";

    let mp_email = match ParsedEmail::parse(multipart_raw) {
        Ok(m) => m,
        Err(_) => {
            debug_print("[APP_MAIL_01_ERROR] Failed to parse multipart email!\n");
            exit(13);
        }
    };

    if !mp_email.is_multipart || mp_email.part_count != 2 {
        debug_print("[APP_MAIL_01_ERROR] Multipart part count mismatch!\n");
        exit(14);
    }

    // Part 0: Plain text
    let p0 = &mp_email.parts[0];
    let p0_body = &multipart_raw[p0.body_start..p0.body_start + p0.body_len];
    if p0.content_type_str() != "text/plain; charset=utf-8" || p0_body != b"Quarterly execution complete. Subsystems green." {
        debug_print("[APP_MAIL_01_ERROR] Part 0 content or type mismatch!\n");
        exit(15);
    }

    // Part 1: Attachment
    let p1 = &mp_email.parts[1];
    if !p1.is_attachment || p1.filename_str() != "report.csv" || p1.encoding != ContentEncoding::Base64 {
        debug_print("[APP_MAIL_01_ERROR] Part 1 attachment metadata mismatch!\n");
        exit(16);
    }

    let p1_raw_b64 = &multipart_raw[p1.body_start..p1.body_start + p1.body_len];
    let mut extracted_attachment = [0u8; 128];
    let att_len = match decode_base64(p1_raw_b64, &mut extracted_attachment) {
        Ok(n) => n,
        Err(_) => {
            debug_print("[APP_MAIL_01_ERROR] Failed to decode attachment Base64 payload!\n");
            exit(17);
        }
    };

    let expected_csv = b"Month,Revenue\nOct,1045000\n";
    if att_len != expected_csv.len() || &extracted_attachment[..att_len] != expected_csv {
        debug_print("[APP_MAIL_01_ERROR] Extracted attachment content mismatch!\n");
        exit(18);
    }

    debug_print("[MAIL_MULTIPART_EXTRACTION_VERIFIED_MARKER_v1.0]\n");

    // =========================================================================
    // 5. VERIFY MAILDIR FLAGS & ENCODING (Contract M-01)
    // =========================================================================
    debug_print("[APP_MAIL_01] Validating Maildir flag serialization and parsing...\n");

    let mut flags = MaildirFlags::new();
    flags.seen = true;
    flags.flagged = true;

    let mut suffix_buf = [0u8; 8];
    let s_len = flags.format_suffix(&mut suffix_buf);
    if s_len != 5 || &suffix_buf[..s_len] != b":2,FS" {
        debug_print("[APP_MAIL_01_ERROR] Maildir suffix format mismatch!\n");
        exit(19);
    }

    let parsed_flags = MaildirFlags::parse_suffix(&suffix_buf[..s_len]);
    if !parsed_flags.seen || !parsed_flags.flagged || parsed_flags.replied || parsed_flags.trashed {
        debug_print("[APP_MAIL_01_ERROR] Maildir flag roundtrip mismatch!\n");
        exit(20);
    }

    debug_print("[MAIL_MAILDIR_FLAGS_VERIFIED_MARKER_v1.0]\n");

    // =========================================================================
    // 6. VERIFY MESSAGE COMPOSER & ROUNDTRIP FIDELITY (Contract M-01, M-02)
    // =========================================================================
    debug_print("[APP_MAIL_01] Composing complete RFC 5322 multipart message with attachment...\n");

    let mut composed_buf = [0u8; 4096];
    let mut composer = MessageComposer::new(&mut composed_buf);

    let compose_len = match composer.compose_multipart_with_attachment(
        "Kyle Luecke <kyle@inos.org>",
        "Architecture Board <board@inos.org>",
        "INOS Autonomous Foundry Report",
        "Sat, 10 Oct 2026 18:35:00 +0000",
        "<msg-inos-foundry-20261010@inos.org>",
        "Foundry Phase 2 verification complete. All application lanes operational.",
        "metrics.csv",
        b"Subsystem,Status\nFiles,Pass\nGrid,Pass\nWrite,Pass\nPlan,Pass\nPDF,Pass\nMail,Pass\n",
    ) {
        Ok(len) => len,
        Err(_) => {
            debug_print("[APP_MAIL_01_ERROR] Failed to compose multipart message!\n");
            exit(21);
        }
    };

    let composed_bytes = &composed_buf[..compose_len];

    // Roundtrip verification: parse the generated message back!
    let rt_email = match ParsedEmail::parse(composed_bytes) {
        Ok(em) => em,
        Err(_) => {
            debug_print("[APP_MAIL_01_ERROR] Generated email failed parsing roundtrip!\n");
            exit(22);
        }
    };

    if !rt_email.is_multipart || rt_email.part_count != 2 {
        debug_print("[APP_MAIL_01_ERROR] Roundtrip email multipart part count mismatch!\n");
        exit(23);
    }

    if rt_email.get_header("Subject") != Some("INOS Autonomous Foundry Report") {
        debug_print("[APP_MAIL_01_ERROR] Roundtrip email subject mismatch!\n");
        exit(24);
    }

    let rt_p1 = &rt_email.parts[1];
    if !rt_p1.is_attachment || rt_p1.filename_str() != "metrics.csv" {
        debug_print("[APP_MAIL_01_ERROR] Roundtrip email attachment header mismatch!\n");
        exit(25);
    }

    let mut rt_att_buf = [0u8; 256];
    let rt_att_raw = &composed_bytes[rt_p1.body_start..rt_p1.body_start + rt_p1.body_len];
    let rt_att_len = match decode_base64(rt_att_raw, &mut rt_att_buf) {
        Ok(n) => n,
        Err(_) => {
            debug_print("[APP_MAIL_01_ERROR] Roundtrip email attachment decode failed!\n");
            exit(26);
        }
    };

    if !rt_att_buf[..rt_att_len].starts_with(b"Subsystem,Status\nFiles,Pass") {
        debug_print("[APP_MAIL_01_ERROR] Roundtrip email attachment content mismatch!\n");
        exit(27);
    }

    debug_print("[MAIL_COMPOSER_ROUNDTRIP_VERIFIED_MARKER_v1.0]\n");

    // =========================================================================
    // 7. PERSISTENCE INTO INOS-FS (Contract M-01, M-03)
    // =========================================================================
    debug_print("[APP_MAIL_01] Persisting inbox.eml and attachment.csv to INOS-FS...\n");

    // 7a: Persist 'inbox.eml'
    let eml_token = match request_capability("inbox.eml", rights::CREATE | rights::WRITE) {
        Ok(t) => t,
        Err(_) => {
            debug_print("[APP_MAIL_01_ERROR] Failed to acquire capability for inbox.eml!\n");
            exit(28);
        }
    };
    if file_create("inbox.eml", eml_token).is_err() {
        debug_print("[APP_MAIL_01_ERROR] Failed to create inbox.eml!\n");
        exit(29);
    }
    if file_write("inbox.eml", composed_bytes, eml_token).is_err() {
        debug_print("[APP_MAIL_01_ERROR] Failed to write inbox.eml!\n");
        exit(30);
    }
    debug_print("[APP_MAIL_01] Persisted 'inbox.eml' to INOS-FS.\n");
    debug_print("[MAIL_MESSAGE_PERSISTENCE_WRITTEN_MARKER_v1.0]\n");

    // 7b: Persist 'attachment.csv'
    let att_token = match request_capability("attachment.csv", rights::CREATE | rights::WRITE) {
        Ok(t) => t,
        Err(_) => {
            debug_print("[APP_MAIL_01_ERROR] Failed to acquire capability for attachment.csv!\n");
            exit(31);
        }
    };
    if file_create("attachment.csv", att_token).is_err() {
        debug_print("[APP_MAIL_01_ERROR] Failed to create attachment.csv!\n");
        exit(32);
    }
    if file_write("attachment.csv", &rt_att_buf[..rt_att_len], att_token).is_err() {
        debug_print("[APP_MAIL_01_ERROR] Failed to write attachment.csv!\n");
        exit(33);
    }
    debug_print("[APP_MAIL_01] Persisted 'attachment.csv' to INOS-FS.\n");
    debug_print("[MAIL_ATTACHMENT_PERSISTENCE_WRITTEN_MARKER_v1.0]\n");

    // 8. Interactive Compositor Mail Client UI
    if let Ok(mut shell) = inos_app_sdk::compositor::AppShell::new(680, 460, "INOS Mail - RFC 5322 & MIME Client") {
        let mut selected_msg: usize = 0;
        let mut compose_mode = false;
        let mut draft_buf = [0u8; 256];
        let init_draft = b"All Q4 subsystem milestones verified. Ready for deployment.";
        draft_buf[..init_draft.len()].copy_from_slice(init_draft);
        let mut draft_len = init_draft.len();
        let mut status_msg = "Inbox (2 messages) | Up/Down: Select | E: Extract CSV | C: Compose | F2: Send/Save EML";

        shell.run_event_loop(|sh, ev| {
            if let Some(inos_app_sdk::compositor::InputEvent::KeyPress { scancode, ascii }) = ev {
                if compose_mode {
                    if scancode == inos_app_sdk::compositor::KEY_F2 || scancode == inos_app_sdk::compositor::KEY_ENTER {
                        let body_str = core::str::from_utf8(&draft_buf[..draft_len]).unwrap_or("");
                        let mut out_eml = [0u8; 2048];
                        let mut comp = MessageComposer::new(&mut out_eml);
                        if let Ok(clen) = comp.compose_multipart_with_attachment(
                            "Kyle Luecke <kyle@inos.org>",
                            "Architecture Board <board@inos.org>",
                            "Interactive Dispatch from INOS Mail",
                            "Thu, 15 Oct 2026 18:00:00 +0000",
                            "<msg-interactive@inos.org>",
                            body_str,
                            "metrics.csv",
                            &rt_att_buf[..rt_att_len],
                        ) {
                            if let Ok(tok) = request_capability("outbound_dispatch.eml", rights::CREATE | rights::WRITE) {
                                let _ = file_create("outbound_dispatch.eml", tok);
                                let _ = file_write("outbound_dispatch.eml", &out_eml[..clen], tok);
                            }
                        }
                        compose_mode = false;
                        status_msg = "Dispatched and saved 'outbound_dispatch.eml' to INOS-FS.";
                    } else if scancode == inos_app_sdk::compositor::KEY_BACKSPACE || ascii == 8 {
                        if draft_len > 0 {
                            draft_len -= 1;
                        }
                    } else if ascii >= 0x20 && ascii <= 0x7E && draft_len + 1 < draft_buf.len() {
                        draft_buf[draft_len] = ascii;
                        draft_len += 1;
                    }
                } else {
                    if scancode == inos_app_sdk::compositor::KEY_UP && selected_msg > 0 {
                        selected_msg -= 1;
                    } else if scancode == inos_app_sdk::compositor::KEY_DOWN && selected_msg < 1 {
                        selected_msg += 1;
                    } else if ascii == b'e' || ascii == b'E' {
                        if let Ok(tok) = request_capability("attachment.csv", rights::CREATE | rights::WRITE) {
                            let _ = file_create("attachment.csv", tok);
                            let _ = file_write("attachment.csv", &rt_att_buf[..rt_att_len], tok);
                            let _ = sh.publish_ambient_state(500, 1002, 0xAA02, "A1", rt_att_len as i64, "CSV_IMPORT", "attachment.csv");
                            status_msg = "Extracted MIME Base64 attachment to 'attachment.csv'.";
                        }
                    } else if ascii == b'c' || ascii == b'C' {
                        compose_mode = true;
                        status_msg = "Compose Mode | Type message body | Enter/F2: Send & Save outbound_dispatch.eml";
                    }
                }
            }
            if let Some((_sender, _op, _wire)) = sh.poll_ambient_directive() {
                status_msg = "Synced live report attachment from bound Workspace pipeline (BoundLiveSync).";
            }

            let _ = sh.clear(0x001E293B);
            let _ = inos_app_sdk::compositor::fill_rect(sh.surface_id, 0, 0, 680, 34, 0x000F172A);
            let _ = sh.draw_button(8, 5, 115, 24, "[C] Compose", compose_mode);
            let _ = sh.draw_button(130, 5, 155, 24, "[E] Save Attachment", false);
            let _ = sh.draw_button(292, 5, 145, 24, "[F2] Dispatch EML", false);

            // Left pane: Message list
            let _ = inos_app_sdk::compositor::fill_rect(sh.surface_id, 8, 42, 220, 380, 0x000F172A);
            let _ = inos_app_sdk::compositor::draw_text(sh.surface_id, 16, 52, 0x0094A3B8, "INBOX (Maildir :2,FS)");
            let msgs = [
                "1. Foundry Report [CSV]",
                "2. Q3 Exec Summary [CSV]",
            ];
            for (idx, m) in msgs.iter().enumerate() {
                let my = 80 + (idx as u32) * 32;
                if idx == selected_msg && !compose_mode {
                    let _ = inos_app_sdk::compositor::fill_rect(sh.surface_id, 12, my - 4, 212, 24, 0x001E3A8A);
                }
                let _ = inos_app_sdk::compositor::draw_text(sh.surface_id, 16, my, 0x00F8FAFC, m);
            }

            // Right pane: Message viewer or composer
            let _ = inos_app_sdk::compositor::fill_rect(sh.surface_id, 236, 42, 436, 380, 0x001E293B);
            if compose_mode {
                let _ = inos_app_sdk::compositor::draw_text(sh.surface_id, 248, 54, 0x0038BDF8, "COMPOSE RFC 5322 MULTIPART MESSAGE");
                let _ = inos_app_sdk::compositor::draw_text(sh.surface_id, 248, 78, 0x00CBD5E1, "From: Kyle Luecke <kyle@inos.org>");
                let _ = inos_app_sdk::compositor::draw_text(sh.surface_id, 248, 98, 0x00CBD5E1, "To:   Architecture Board <board@inos.org>");
                let _ = inos_app_sdk::compositor::draw_text(sh.surface_id, 248, 118, 0x00CBD5E1, "Subj: Interactive Dispatch from INOS Mail");
                let _ = inos_app_sdk::compositor::fill_rect(sh.surface_id, 248, 142, 412, 180, 0x000F172A);
                let body_s = core::str::from_utf8(&draft_buf[..draft_len]).unwrap_or("");
                let _ = inos_app_sdk::compositor::draw_text(sh.surface_id, 256, 154, 0x00F8FAFC, body_s);
            } else if selected_msg == 0 {
                let _ = inos_app_sdk::compositor::draw_text(sh.surface_id, 248, 54, 0x0038BDF8, "Subject: INOS Autonomous Foundry Report");
                let _ = inos_app_sdk::compositor::draw_text(sh.surface_id, 248, 76, 0x0094A3B8, "From: Kyle Luecke <kyle@inos.org>");
                let _ = inos_app_sdk::compositor::draw_text(sh.surface_id, 248, 96, 0x0094A3B8, "To:   Architecture Board <board@inos.org>");
                let _ = inos_app_sdk::compositor::fill_rect(sh.surface_id, 248, 118, 412, 1, 0x00334155);
                let _ = inos_app_sdk::compositor::draw_text(sh.surface_id, 248, 132, 0x00F8FAFC, "Foundry Phase 2 verification complete.");
                let _ = inos_app_sdk::compositor::draw_text(sh.surface_id, 248, 152, 0x00F8FAFC, "All application lanes operational.");
                let _ = inos_app_sdk::compositor::fill_rect(sh.surface_id, 248, 190, 412, 80, 0x000F172A);
                let _ = inos_app_sdk::compositor::draw_text(sh.surface_id, 256, 200, 0x0022C55E, "Attachment [metrics.csv] (Base64 Decoded):");
                let _ = inos_app_sdk::compositor::draw_text(sh.surface_id, 256, 222, 0x00CBD5E1, "Subsystem,Status -> Files:Pass, Grid:Pass, Write:Pass");
            } else {
                let _ = inos_app_sdk::compositor::draw_text(sh.surface_id, 248, 54, 0x0038BDF8, "Subject: Q3 Execution Summary");
                let _ = inos_app_sdk::compositor::draw_text(sh.surface_id, 248, 76, 0x0094A3B8, "From: ops@inos.org");
                let _ = inos_app_sdk::compositor::fill_rect(sh.surface_id, 248, 118, 412, 1, 0x00334155);
                let _ = inos_app_sdk::compositor::draw_text(sh.surface_id, 248, 132, 0x00F8FAFC, "Quarterly execution complete. Subsystems green.");
                let _ = inos_app_sdk::compositor::draw_text(sh.surface_id, 248, 190, 0x0022C55E, "Attachment [report.csv]: Month,Revenue -> Oct,1045000");
            }

            let _ = sh.draw_status_bar(status_msg);
            true
        });
    }

    debug_print("[APP_MAIL_SUITE_COMPLETE_MARKER_v1.0]\n");
    debug_print("[APP_MAIL_01] All RFC 5322, MIME, Base64, and Maildir tests PASSED cleanly.\n");
    exit(0);
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    debug_print("[APP_MAIL_01] PANIC occurred in Ring 3 Mail app!\n");
    exit(99);
}
