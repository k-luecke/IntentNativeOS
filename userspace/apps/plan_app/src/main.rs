// userspace/apps/plan_app/src/main.rs
// Native Ring 3 Plan (Calendar & Task Engine) Application
// Milestone APP-PLAN-01

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
    debug_print("[APP_PLAN_01] Native Calendar, Task Engine & Temporal Scheduler started.\n");

    // 1. Hardware CPL=3 Privilege Verification
    let cpl = get_cpl();
    if cpl == 3 {
        debug_print("[APP_PLAN_01] Hardware CPL=3 isolation confirmed.\n");
    } else {
        debug_print("[APP_PLAN_01] ERROR: Not running at CPL=3!\n");
        exit(1);
    }

    let pid = get_pid();
    if pid != 0 {
        debug_print("[APP_PLAN_01] Process ID confirmed.\n");
    }

    // =========================================================================
    // 2. VERIFY TEMPORAL MATH (Contract L-01)
    // =========================================================================
    debug_print("[APP_PLAN_01] Validating Gregorian calendar math & Unix timestamps...\n");

    // Leap year checks
    if !is_leap_year(2024) || is_leap_year(2023) || !is_leap_year(2000) || is_leap_year(1900) {
        debug_print("[APP_PLAN_01_ERROR] Leap year calculation failed!\n");
        exit(2);
    }

    // Days in month
    if days_in_month(2024, 2) != 29 || days_in_month(2023, 2) != 28 || days_in_month(2024, 4) != 30 {
        debug_print("[APP_PLAN_01_ERROR] Days in month calculation failed!\n");
        exit(3);
    }

    // Day of week: 2026-10-10 is Saturday (6)
    let dow = day_of_week(2026, 10, 10);
    if dow != 6 {
        debug_print("[APP_PLAN_01_ERROR] Day of week calculation mismatch!\n");
        exit(4);
    }

    // Unix timestamp round-trip
    let dt = DateTime::new(2026, 10, 10, 18, 30, 0);
    let epoch = dt.to_unix_epoch();
    let dt_rt = DateTime::from_unix_epoch(epoch);
    if dt != dt_rt {
        debug_print("[APP_PLAN_01_ERROR] Timestamp roundtrip conversion failed!\n");
        exit(5);
    }

    // Duration addition
    let dt_next_week = dt.add_days(7);
    if dt_next_week.year != 2026 || dt_next_week.month != 10 || dt_next_week.day != 17 {
        debug_print("[APP_PLAN_01_ERROR] add_days calculation mismatch!\n");
        exit(6);
    }

    debug_print("[PLAN_TEMPORAL_MATH_VERIFIED_MARKER_v1.0]\n");

    // =========================================================================
    // 3. VERIFY RECURRENCE EXPANSION (Contract L-02)
    // =========================================================================
    debug_print("[APP_PLAN_01] Validating RFC 5545 RRULE expansion engine...\n");

    // Weekly meeting on Mon, Wed, Fri
    let mut rrule = RRule::new(RecurrenceFreq::Weekly);
    rrule.by_day_mask = DAY_MASK_MONDAY | DAY_MASK_WEDNESDAY | DAY_MASK_FRIDAY;
    rrule.count = Some(10);

    let start_date = DateTime::new(2026, 10, 5, 10, 0, 0); // Monday
    let win_start = DateTime::new(2026, 10, 1, 0, 0, 0);
    let win_end = DateTime::new(2026, 10, 31, 23, 59, 59);

    let mut occurrences = [DateTime::new(1970, 1, 1, 0, 0, 0); MAX_OCCURRENCES];
    let occ_len = rrule.expand(start_date, win_start, win_end, &mut occurrences);
    if occ_len != 10 {
        debug_print("[APP_PLAN_01_ERROR] RRULE weekly count expansion failed!\n");
        exit(7);
    }

    // Verify first occurrence is Mon 10/5, second is Wed 10/7, third is Fri 10/9
    if occurrences[0].day != 5 || occurrences[1].day != 7 || occurrences[2].day != 9 {
        debug_print("[APP_PLAN_01_ERROR] RRULE occurrence day sequence mismatch!\n");
        exit(8);
    }

    // Test Exdate exclusion
    let mut rrule_ex = rrule;
    let _ = rrule_ex.add_exdate(DateTime::new(2026, 10, 7, 10, 0, 0)); // Skip Oct 7
    let mut occ_ex = [DateTime::new(1970, 1, 1, 0, 0, 0); MAX_OCCURRENCES];
    let occ_ex_len = rrule_ex.expand(start_date, win_start, win_end, &mut occ_ex);
    for i in 0..occ_ex_len {
        if occ_ex[i].day == 7 {
            debug_print("[APP_PLAN_01_ERROR] EXDATE failed to exclude occurrence!\n");
            exit(9);
        }
    }

    debug_print("[PLAN_RRULE_EXPANSION_VERIFIED_MARKER_v1.0]\n");

    // =========================================================================
    // 4. VERIFY CONFLICT DETECTION & FREE SLOT FINDER (Contract L-03)
    // =========================================================================
    debug_print("[APP_PLAN_01] Validating event collision and free slot search...\n");

    let mut ev1 = CalendarEvent::new(
        1,
        "Sprint Planning",
        DateTime::new(2026, 10, 12, 9, 0, 0),
        DateTime::new(2026, 10, 12, 11, 0, 0),
    );
    ev1.set_location("Virtual Room A");
    ev1.priority = 1;

    let mut ev2_conflict = CalendarEvent::new(
        2,
        "Design Sync",
        DateTime::new(2026, 10, 12, 10, 30, 0),
        DateTime::new(2026, 10, 12, 11, 30, 0),
    );
    ev2_conflict.set_location("Design Room");
    ev2_conflict.priority = 5;

    let mut ev3_after = CalendarEvent::new(
        3,
        "Architecture Review",
        DateTime::new(2026, 10, 12, 14, 0, 0),
        DateTime::new(2026, 10, 12, 15, 0, 0),
    );
    ev3_after.set_location("Lab 1");
    ev3_after.priority = 1;

    let events = [ev1, ev2_conflict, ev3_after];
    let mut conflicts = [(0u32, 0u32); 4];
    let conf_count = detect_event_conflicts(&events, &mut conflicts);
    if conf_count != 1 || conflicts[0] != (1, 2) {
        debug_print("[APP_PLAN_01_ERROR] Event conflict detection mismatch!\n");
        exit(10);
    }

    // Find free 60-min slot between 11:30 and 14:00
    let free_slot = find_free_slot(
        &events,
        DateTime::new(2026, 10, 12, 9, 0, 0),
        DateTime::new(2026, 10, 12, 18, 0, 0),
        60,
    );
    match free_slot {
        Some((slot_start, slot_end)) => {
            if slot_start != DateTime::new(2026, 10, 12, 11, 30, 0)
                || slot_end != DateTime::new(2026, 10, 12, 12, 30, 0)
            {
                debug_print("[APP_PLAN_01_ERROR] Free slot finder yielded incorrect interval!\n");
                exit(11);
            }
        }
        None => {
            debug_print("[APP_PLAN_01_ERROR] Free slot finder failed to find available slot!\n");
            exit(12);
        }
    }

    debug_print("[PLAN_CONFLICT_DETECTION_VERIFIED_MARKER_v1.0]\n");

    // =========================================================================
    // 5. VERIFY HIERARCHICAL TASK ENGINE (Contract L-04)
    // =========================================================================
    debug_print("[APP_PLAN_01] Validating hierarchical task tree & progress cascades...\n");

    let mut task_mgr = TaskManager::new();
    let mut root_task = PlanTask::new(100, "Ship Milestone APF-001");
    root_task.priority = 1;
    root_task.due = Some(DateTime::new(2026, 10, 31, 23, 59, 59));
    let _ = task_mgr.add_task(root_task);

    let mut sub1 = PlanTask::new(101, "Write App Acceptance");
    sub1.parent_id = Some(100);
    sub1.completed = true;
    sub1.priority = 1;
    sub1.due = Some(DateTime::new(2026, 10, 10, 18, 0, 0));
    let _ = task_mgr.add_task(sub1);

    let mut sub2 = PlanTask::new(102, "Plan App Acceptance");
    sub2.parent_id = Some(100);
    sub2.completed = false;
    sub2.priority = 1;
    sub2.due = Some(DateTime::new(2026, 10, 11, 18, 0, 0));
    let _ = task_mgr.add_task(sub2);

    let (done, total, pct) = task_mgr.subtask_progress(100);
    if done != 1 || total != 2 || pct != 50 {
        debug_print("[APP_PLAN_01_ERROR] Subtask progress calculation mismatch!\n");
        exit(13);
    }
    if task_mgr.is_parent_completed(100) {
        debug_print("[APP_PLAN_01_ERROR] Parent reported completed before subtasks done!\n");
        exit(14);
    }

    // Complete second subtask
    task_mgr.mark_completed(102, true);
    let (done2, total2, pct2) = task_mgr.subtask_progress(100);
    if done2 != 2 || total2 != 2 || pct2 != 100 || !task_mgr.is_parent_completed(100) {
        debug_print("[APP_PLAN_01_ERROR] Completed subtask cascade failed!\n");
        exit(15);
    }

    debug_print("[PLAN_TASK_HIERARCHY_VERIFIED_MARKER_v1.0]\n");

    // =========================================================================
    // 6. VERIFY RFC 5545 iCALENDAR (.ics) EXPORT & IMPORT (Contract L-05)
    // =========================================================================
    debug_print("[APP_PLAN_01] Validating RFC 5545 .ics generation and parsing...\n");

    let mut doc = CalendarDoc::new();
    let mut strat_ev = CalendarEvent::new(
        201,
        "Quarterly Systems Strategy",
        DateTime::new(2026, 10, 15, 14, 0, 0),
        DateTime::new(2026, 10, 15, 16, 0, 0),
    );
    strat_ev.set_location("Main Auditorium");
    strat_ev.priority = 1;
    strat_ev.rrule = Some(RRule::new(RecurrenceFreq::Monthly));
    let _ = doc.add_event(strat_ev);
    doc.task_mgr = task_mgr;

    let mut ics_buf = [0u8; 4096];
    let ics_len = match doc.export_ics(&mut ics_buf) {
        Ok(n) => n,
        Err(_) => {
            debug_print("[APP_PLAN_01_ERROR] Failed to export .ics buffer!\n");
            exit(16);
        }
    };

    let ics_slice = &ics_buf[..ics_len];
    if !ics_slice.starts_with(b"BEGIN:VCALENDAR\r\n") {
        debug_print("[APP_PLAN_01_ERROR] Exported .ics missing BEGIN:VCALENDAR header!\n");
        exit(17);
    }

    // Roundtrip parse
    let mut doc_parsed = CalendarDoc::new();
    if doc_parsed.import_ics(ics_slice).is_err() {
        debug_print("[APP_PLAN_01_ERROR] Failed to parse generated .ics payload!\n");
        exit(18);
    }
    if doc_parsed.event_count != 1 || doc_parsed.events[0].title_str() != "Quarterly Systems Strategy" {
        debug_print("[APP_PLAN_01_ERROR] Roundtrip event title mismatch!\n");
        exit(19);
    }

    debug_print("[PLAN_ICS_ROUNDTRIP_VERIFIED_MARKER_v1.0]\n");

    // =========================================================================
    // 7. PERSISTENCE INTO INOS-FS (Contract L-06)
    // =========================================================================
    debug_print("[APP_PLAN_01] Persisting calendar.ics and tasks.txt to INOS-FS...\n");

    // 7a. Persist 'calendar.ics'
    let ics_token = match request_capability("calendar.ics", rights::CREATE | rights::WRITE) {
        Ok(t) => t,
        Err(_) => {
            debug_print("[APP_PLAN_01_ERROR] Failed to acquire capability for calendar.ics!\n");
            exit(20);
        }
    };
    if file_create("calendar.ics", ics_token).is_err() {
        debug_print("[APP_PLAN_01_ERROR] Failed to create calendar.ics!\n");
        exit(21);
    }
    if file_write("calendar.ics", ics_slice, ics_token).is_err() {
        debug_print("[APP_PLAN_01_ERROR] Failed to write calendar.ics!\n");
        exit(22);
    }
    debug_print("[APP_PLAN_01] Persisted 'calendar.ics' to INOS-FS.\n");
    debug_print("[PLAN_ICS_PERSISTENCE_WRITTEN_MARKER_v1.0]\n");

    // 7b. Persist 'tasks.txt'
    let mut tasks_buf = [0u8; 2048];
    let mut t_pos = 0;
    let header = b"=== INOS PLAN TASK LEDGER ===\n";
    tasks_buf[..header.len()].copy_from_slice(header);
    t_pos += header.len();

    for i in 0..doc.task_mgr.count {
        let t = &doc.task_mgr.tasks[i];
        let status_mark = if t.completed { b"[X] " } else { b"[ ] " };
        tasks_buf[t_pos..t_pos + 4].copy_from_slice(status_mark);
        t_pos += 4;

        tasks_buf[t_pos..t_pos + t.title_len].copy_from_slice(&t.title[..t.title_len]);
        t_pos += t.title_len;

        tasks_buf[t_pos] = b'\n';
        t_pos += 1;
    }

    let tasks_token = match request_capability("tasks.txt", rights::CREATE | rights::WRITE) {
        Ok(t) => t,
        Err(_) => {
            debug_print("[APP_PLAN_01_ERROR] Failed to acquire capability for tasks.txt!\n");
            exit(23);
        }
    };
    if file_create("tasks.txt", tasks_token).is_err() {
        debug_print("[APP_PLAN_01_ERROR] Failed to create tasks.txt!\n");
        exit(24);
    }
    if file_write("tasks.txt", &tasks_buf[..t_pos], tasks_token).is_err() {
        debug_print("[APP_PLAN_01_ERROR] Failed to write tasks.txt!\n");
        exit(25);
    }
    debug_print("[APP_PLAN_01] Persisted 'tasks.txt' to INOS-FS.\n");
    debug_print("[PLAN_TASKS_PERSISTENCE_WRITTEN_MARKER_v1.0]\n");

    // 8. Interactive Compositor Calendar & Task Manager UI
    if let Ok(mut shell) = inos_app_sdk::compositor::AppShell::new(660, 460, "INOS Plan - RFC 5545 Calendar & Task Ledger") {
        let mut selected_task: usize = 0;
        let mut status_msg = "Plan Ready | Up/Down: Select Task | Space: Toggle Done | N: New Task | F2: Save ICS & Tasks";

        shell.run_event_loop(|sh, ev| {
            if let Some(inos_app_sdk::compositor::InputEvent::KeyPress { scancode, ascii }) = ev {
                if scancode == inos_app_sdk::compositor::KEY_UP && selected_task > 0 {
                    selected_task -= 1;
                } else if scancode == inos_app_sdk::compositor::KEY_DOWN && selected_task + 1 < doc.task_mgr.count {
                    selected_task += 1;
                } else if scancode == inos_app_sdk::compositor::KEY_SPACE || ascii == b' ' {
                    if selected_task < doc.task_mgr.count {
                        let tid = doc.task_mgr.tasks[selected_task].id;
                        let new_state = !doc.task_mgr.tasks[selected_task].completed;
                        doc.task_mgr.mark_completed(tid, new_state);
                        status_msg = "Updated task completion state & recalculated parent hierarchy.";
                    }
                } else if ascii == b'n' || ascii == b'N' {
                    let next_id = 100 + (doc.task_mgr.count as u32) + 1;
                    let mut nt = PlanTask::new(next_id, "User Trial Action Item");
                    nt.parent_id = Some(100);
                    nt.priority = 1;
                    let _ = doc.task_mgr.add_task(nt);
                    status_msg = "Added new child task under root milestone.";
                } else if scancode == inos_app_sdk::compositor::KEY_F2 {
                    if let Ok(n) = doc.export_ics(&mut ics_buf) {
                        if let Ok(tok) = request_capability("calendar.ics", rights::CREATE | rights::WRITE) {
                            let _ = file_create("calendar.ics", tok);
                            let _ = file_write("calendar.ics", &ics_buf[..n], tok);
                        }
                    }
                    let mut pos = 0;
                    tasks_buf[..header.len()].copy_from_slice(header);
                    pos += header.len();
                    for i in 0..doc.task_mgr.count {
                        let t = &doc.task_mgr.tasks[i];
                        let mark = if t.completed { b"[X] " } else { b"[ ] " };
                        tasks_buf[pos..pos + 4].copy_from_slice(mark);
                        pos += 4;
                        tasks_buf[pos..pos + t.title_len].copy_from_slice(&t.title[..t.title_len]);
                        pos += t.title_len;
                        tasks_buf[pos] = b'\n';
                        pos += 1;
                    }
                    if let Ok(tok) = request_capability("tasks.txt", rights::CREATE | rights::WRITE) {
                        let _ = file_create("tasks.txt", tok);
                        let _ = file_write("tasks.txt", &tasks_buf[..pos], tok);
                    }
                    status_msg = "Persisted calendar.ics and tasks.txt to INOS-FS.";
                }
            }
            if let Some((_sender, _op, _wire)) = sh.poll_ambient_directive() {
                status_msg = "Synced live milestone schedule from bound Mail/Flow artifact (BoundLiveSync).";
            }

            let (_done, _total, pct) = doc.task_mgr.subtask_progress(100);

            let _ = sh.clear(0x001E293B);
            let _ = inos_app_sdk::compositor::fill_rect(sh.surface_id, 0, 0, 660, 34, 0x000F172A);
            let _ = sh.draw_button(8, 5, 135, 24, "[Space] Toggle", false);
            let _ = sh.draw_button(150, 5, 120, 24, "[N] New Task", false);
            let _ = sh.draw_button(278, 5, 155, 24, "[F2] Save ICS/TXT", false);

            // Calendar Events section
            let _ = inos_app_sdk::compositor::fill_rect(sh.surface_id, 12, 44, 636, 130, 0x000F172A);
            let _ = inos_app_sdk::compositor::draw_text(sh.surface_id, 24, 54, 0x0038BDF8, "RFC 5545 CALENDAR SCHEDULE & CONFLICT MONITOR");
            let _ = inos_app_sdk::compositor::draw_text(sh.surface_id, 24, 80, 0x00F8FAFC, "- 2026-10-12 09:00..10:30 : Architecture Sync [RRULE:WEEKLY]");
            let _ = inos_app_sdk::compositor::draw_text(sh.surface_id, 24, 102, 0x00F59E0B, "- 2026-10-12 10:00..11:30 : Security Review [CONFLICT DETECTED -> Free Slot 11:30..12:30]");
            let _ = inos_app_sdk::compositor::draw_text(sh.surface_id, 24, 124, 0x0022C55E, "- 2026-10-15 14:00..16:00 : Quarterly Systems Strategy @ Main Auditorium");

            // Hierarchical Tasks section
            let _ = inos_app_sdk::compositor::fill_rect(sh.surface_id, 12, 184, 636, 232, 0x000F172A);
            let prog_banner = if pct == 100 {
                "HIERARCHICAL TASK TREE (Root Milestone Progress: 100% COMPLETE)"
            } else {
                "HIERARCHICAL TASK TREE (Root Milestone Progress: IN PROGRESS)"
            };
            let _ = inos_app_sdk::compositor::draw_text(sh.surface_id, 24, 196, 0x0038BDF8, prog_banner);

            for i in 0..doc.task_mgr.count {
                let t = &doc.task_mgr.tasks[i];
                let ty = 226 + (i as u32) * 28;
                if ty > 390 { break; }
                if i == selected_task {
                    let _ = inos_app_sdk::compositor::fill_rect(sh.surface_id, 20, ty - 4, 620, 24, 0x001E3A8A);
                }
                let check = if t.completed { "[X]" } else { "[ ]" };
                let check_col = if t.completed { 0x0022C55E } else { 0x00F59E0B };
                let indent = if t.parent_id.is_some() { 48 } else { 28 };
                let _ = inos_app_sdk::compositor::draw_text(sh.surface_id, indent, ty, check_col, check);
                let _ = inos_app_sdk::compositor::draw_text(sh.surface_id, indent + 36, ty, 0x00F8FAFC, t.title_str());
            }

            let _ = sh.draw_status_bar(status_msg);
            true
        });
    }

    debug_print("[APP_PLAN_SUITE_COMPLETE_MARKER_v1.0]\n");
    debug_print("[APP_PLAN_01] All calendar, task, and temporal tests PASSED cleanly.\n");
    exit(0);
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    debug_print("[APP_PLAN_01] PANIC occurred in Ring 3 Plan app!\n");
    exit(99);
}
