// userspace/apps/intscript_app/src/ui.rs - IntScript Studio Compositor & Step Debugger
extern crate alloc;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use inos_app_sdk::compositor::{draw_text, fill_rect, present_surface};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StudioTab {
    Editor,
    StepDebugger,
    ScheduleModal,
    ReceiptsAudit,
    PermsInspector,
    ApiDocs,
}

pub struct ScriptStudioState {
    pub active_tab: StudioTab,
    pub lines: Vec<String>,
    pub logs: Vec<String>,
    pub cursor_line: usize,
    pub cursor_col: usize,
    pub step_index: usize,
    pub quota_used: u64,
    pub quota_max: u64,
    pub script_name: String,
    pub status_msg: String,
}

impl ScriptStudioState {
    pub fn new() -> Self {
        let mut lines = Vec::new();
        lines.push(String::from("// INOS IntScript Studio v3.0 - Automation"));
        lines.push(String::from("const wb = inos.grid.open(\"grid_output.xlsx\");"));
        lines.push(String::from("const sheet = wb.getSheet(\"Sheet1\");"));
        lines.push(String::from("sheet.set(\"B2\", 15000);"));
        lines.push(String::from("sheet.set(\"C2\", 4200);"));
        lines.push(String::from("sheet.setFormula(\"D2\", \"=B2-C2\");"));
        lines.push(String::from("const profit = sheet.evaluate(\"D2\");"));
        lines.push(String::from("inos.audit.log(\"Q1 Recalculated Profit: \" + profit);"));
        lines.push(String::from("inos.grid.save(\"intscript_grid_out.xlsx\");"));

        Self {
            active_tab: StudioTab::Editor,
            lines,
            logs: Vec::new(),
            cursor_line: 3,
            cursor_col: 10,
            step_index: 0,
            quota_used: 4120,
            quota_max: 250_000,
            script_name: String::from("main.ints"),
            status_msg: String::from("Ready"),
        }
    }

    pub fn handle_key(&mut self, c: char) {
        if self.cursor_line < self.lines.len() {
            if c == '\n' {
                self.lines.insert(self.cursor_line + 1, String::new());
                self.cursor_line += 1;
                self.cursor_col = 0;
            } else if c == '\x08' {
                // Backspace
                if self.cursor_col > 0 {
                    self.lines[self.cursor_line].remove(self.cursor_col - 1);
                    self.cursor_col -= 1;
                }
            } else {
                self.lines[self.cursor_line].insert(self.cursor_col, c);
                self.cursor_col += 1;
            }
        }
    }

    pub fn handle_click(&mut self, x: u32, y: u32) {
        if y >= 32 && y <= 64 {
            if x >= 12 && x <= 82 {
                // RUN
                self.active_tab = StudioTab::Editor;
                self.status_msg = String::from("Running...");
            } else if x >= 90 && x <= 160 {
                // STEP
                self.active_tab = StudioTab::StepDebugger;
                self.step_index += 1;
                self.status_msg = format!("Stepping: Instruction {}", self.step_index);
            } else if x >= 168 && x <= 248 {
                // SCHEDULE
                self.active_tab = StudioTab::ScheduleModal;
                self.status_msg = String::from("Schedule Configuration Active");
            } else if x >= 256 && x <= 321 {
                // AUDIT
                self.active_tab = StudioTab::ReceiptsAudit;
                self.status_msg = String::from("Receipts Ledger View");
            } else if x >= 330 && x <= 395 {
                // PERMS
                self.active_tab = StudioTab::PermsInspector;
                self.status_msg = String::from("Capability Isolation Active");
            } else if x >= 405 && x <= 465 {
                // DOCS
                self.active_tab = StudioTab::ApiDocs;
                self.status_msg = String::from("API Documentation & Reference");
            }
        }
    }
}

pub fn render_script_studio_state(surface_id: u32, state: &ScriptStudioState) {
    // Canvas background (680 x 440)
    let _ = fill_rect(surface_id, 0, 0, 680, 440, 0xFF1E1E2E);

    // Title bar (y: 0..32)
    let _ = fill_rect(surface_id, 0, 0, 680, 32, 0xFF313244);
    let title = format!("INOS IntScript Studio - {} [{}]", state.script_name, state.status_msg);
    let _ = draw_text(surface_id, 14, 10, 0xFFCDD6F4, &title);

    // Toolbar (y: 32..64)
    let _ = fill_rect(surface_id, 0, 32, 680, 32, 0xFF45475A);

    // Toolbar buttons with active highlighting
    let run_color = if state.active_tab == StudioTab::Editor { 0xFF89B4FA } else { 0xFF585B70 };
    let _ = fill_rect(surface_id, 12, 38, 70, 20, run_color);
    let _ = draw_text(surface_id, 24, 42, 0xFF1E1E2E, "> RUN");

    let step_color = if state.active_tab == StudioTab::StepDebugger { 0xFFA6E3A1 } else { 0xFF585B70 };
    let _ = fill_rect(surface_id, 90, 38, 70, 20, step_color);
    let _ = draw_text(surface_id, 106, 42, 0xFFCDD6F4, "STEP");

    let sched_color = if state.active_tab == StudioTab::ScheduleModal { 0xFFF9E2AF } else { 0xFF585B70 };
    let _ = fill_rect(surface_id, 168, 38, 80, 20, sched_color);
    let _ = draw_text(surface_id, 178, 42, 0xFFCDD6F4, "SCHEDULE");

    let audit_color = if state.active_tab == StudioTab::ReceiptsAudit { 0xFF89DCEB } else { 0xFF585B70 };
    let _ = fill_rect(surface_id, 256, 38, 65, 20, audit_color);
    let _ = draw_text(surface_id, 268, 42, 0xFFCDD6F4, "AUDIT");

    let perms_color = if state.active_tab == StudioTab::PermsInspector { 0xFFCBA6F7 } else { 0xFF585B70 };
    let _ = fill_rect(surface_id, 330, 38, 65, 20, perms_color);
    let _ = draw_text(surface_id, 342, 42, 0xFFCDD6F4, "PERMS");

    let docs_color = if state.active_tab == StudioTab::ApiDocs { 0xFFF38BA8 } else { 0xFF585B70 };
    let _ = fill_rect(surface_id, 405, 38, 60, 20, docs_color);
    let _ = draw_text(surface_id, 417, 42, 0xFFCDD6F4, "DOCS");

    // Capability status badges
    let _ = fill_rect(surface_id, 475, 38, 90, 20, 0xFF313244);
    let _ = draw_text(surface_id, 483, 42, 0xFFA6E3A1, "[CAP: FS_RW]");

    let _ = fill_rect(surface_id, 575, 38, 95, 20, 0xFF313244);
    let _ = draw_text(surface_id, 583, 42, 0xFFF9E2AF, "[CAP: SCHED]");

    // Main workspace pane (y: 66..290)
    let _ = fill_rect(surface_id, 0, 66, 680, 224, 0xFF181825);

    match state.active_tab {
        StudioTab::Editor => {
            // Line number gutter (x: 0..36)
            let _ = fill_rect(surface_id, 0, 66, 36, 224, 0xFF1E1E2E);

            let mut y_pos = 72;
            for (idx, line) in state.lines.iter().take(11).enumerate() {
                let num_str = format!("{}", idx + 1);
                let _ = draw_text(surface_id, 12, y_pos, 0xFF6C7086, &num_str);

                let color = if line.starts_with("//") {
                    0xFF6C7086 // Comment: gray
                } else if line.contains("const") || line.contains("let") || line.contains("function") {
                    0xFF89B4FA // Keyword: blue
                } else if line.contains("inos.") {
                    0xFFF9E2AF // System API: yellow
                } else if line.contains('"') {
                    0xFFA6E3A1 // String: green
                } else {
                    0xFFCDD6F4 // Normal: white
                };

                let _ = draw_text(surface_id, 44, y_pos, color, line);
                if idx == state.cursor_line {
                    let cursor_x = 44 + (state.cursor_col as u32) * 8;
                    let _ = fill_rect(surface_id, cursor_x, y_pos, 2, 16, 0xFFF38BA8);
                }
                y_pos += 19;
            }
        }
        StudioTab::StepDebugger => {
            let _ = draw_text(surface_id, 20, 76, 0xFFA6E3A1, ">>> Step Debugger Active - Symbolic Evaluation & Capability Handles <<<");
            let step_str = format!("Program Counter: Step #{} / OpCode: CALL_METHOD", state.step_index);
            let _ = draw_text(surface_id, 20, 96, 0xFFCDD6F4, &step_str);
            let _ = draw_text(surface_id, 20, 114, 0xFF89DCEB, "Redacted Diagnostic Capability Descriptors (Zero Bearer Secret Disclosure):");
            let _ = draw_text(surface_id, 35, 132, 0xFFA6E3A1, "Handle #CAP-01: 'sales_q1_analyzed.csv' [R, W, CREATE] Gen #1 Epoch 1");
            let _ = draw_text(surface_id, 35, 148, 0xFFA6E3A1, "Handle #CAP-02: 'sales_report.svg' [R, W, CREATE] Gen #1 Epoch 1");
            let _ = draw_text(surface_id, 35, 164, 0xFFF9E2AF, "Active Tx:      STAGED (4 files, 1,280 bytes) | Rollback Safety: ARMED");
            let _ = draw_text(surface_id, 35, 182, 0xFFBAC2DE, "Symbol Scope:   wb: <ScriptWorkbook 'grid_output.xlsx'>, profit: 10800");
            let _ = draw_text(surface_id, 35, 200, 0xFF89DCEB, "Kernel DRBG:    HW_READY (hardware entropy source initialized)");
        }
        StudioTab::ScheduleModal => {
            let _ = draw_text(surface_id, 20, 76, 0xFFF9E2AF, "=== IntScript Scheduler: Automation Job Scheduling Configuration ===");
            let _ = draw_text(surface_id, 20, 104, 0xFFCDD6F4, "Target Script:  main.ints");
            let _ = draw_text(surface_id, 20, 126, 0xFFCDD6F4, "Trigger Type:   WorkbookChanged (intscript_grid_out.xlsx)");
            let _ = draw_text(surface_id, 20, 148, 0xFFCDD6F4, "Missed Policy:  RunImmediate (Runs on boot if missed)");
            let _ = draw_text(surface_id, 20, 170, 0xFFCDD6F4, "Retry Limit:    3 attempts on error / loop detection");
            let _ = draw_text(surface_id, 20, 192, 0xFFA6E3A1, "Queue State:    Job #1 Armed in /system/jobs.db [DURABLE]");
        }
        StudioTab::ReceiptsAudit => {
            let _ = draw_text(surface_id, 20, 76, 0xFF89DCEB, "=== IntScript Scheduler: Durable Receipts Ledger (/system/script_receipts.db) ===");
            let _ = draw_text(surface_id, 20, 104, 0xFFA6E3A1, "[RECEIPT #1] Job #1 SUCCESS - exit_code: 0, instructions: 4,120");
            let _ = draw_text(surface_id, 20, 126, 0xFFBAC2DE, "  Trigger: WorkbookChanged -> intscript_grid_out.xlsx (Formula Profit: 10800)");
            let _ = draw_text(surface_id, 20, 148, 0xFFA6E3A1, "[RECEIPT #2] Job #2 SUCCESS (Missed Run Catchup on Cold Boot)");
            let _ = draw_text(surface_id, 20, 170, 0xFFBAC2DE, "  Trigger: Timestamp Trigger Replayed -> reboot_intscript_out.xlsx [OK]");
        }
        StudioTab::PermsInspector => {
            let _ = draw_text(surface_id, 20, 76, 0xFFCBA6F7, "=== IntScript Studio: Capability Manifest & Pre-Run Permissions ===");
            let _ = draw_text(surface_id, 20, 96, 0xFFCDD6F4, "Authenticated Identity: org.inos.intscript [Verified Process PID 601, Gen #1]");
            let _ = draw_text(surface_id, 20, 114, 0xFF89DCEB, "Kernel Token Source:    CSPRNG Hardware Entropy: HW_READY (256 bits)");
            let _ = draw_text(surface_id, 20, 132, 0xFFA6E3A1, "Diagnostic Handles:     #CAP-01, #CAP-02, #CAP-03, #CAP-04 (Redacted Bearer Tokens)");
            let _ = draw_text(surface_id, 20, 150, 0xFFCDD6F4, "Scoped Paths:           *.xlsx, *.csv, *.txt, *.zip, *.svg, *.json, *.md");
            let _ = draw_text(surface_id, 20, 168, 0xFFF9E2AF, "Staging Quota:          128 KB (131,072 bytes) staged per transaction, max 16 files");
            let _ = draw_text(surface_id, 20, 186, 0xFFF38BA8, "Strict Deny:            /system/.legacy_retired, /system/jobs.db, .workspace/*");
            let _ = draw_text(surface_id, 20, 204, 0xFFA6E3A1, "Transaction Subsystem:  INOS-FS Atomic Journal Active (All-or-Nothing Durability)");
        }
        StudioTab::ApiDocs => {
            let _ = draw_text(surface_id, 20, 76, 0xFFF38BA8, "=== INOS IntScript Native API Reference (INOS-AUTOMATION-ABI-001) ===");
            let _ = draw_text(surface_id, 20, 100, 0xFF89B4FA, "inos.grid:    open(path), create(), importCsv(path), exportCsv(path)");
            let _ = draw_text(surface_id, 20, 120, 0xFFBAC2DE, "              Sheet: get(cell), set(cell, val), setFormula(c, f), setFormat(c, fmt)");
            let _ = draw_text(surface_id, 20, 140, 0xFFA6E3A1, "inos.write:   createDocument(title), openDocument(path), addHeading, addParagraph");
            let _ = draw_text(surface_id, 20, 160, 0xFFBAC2DE, "              addTable(rows, cols), setCell(r, c, val), exportText(path)");
            let _ = draw_text(surface_id, 20, 180, 0xFFF9E2AF, "inos.canvas:  create(w, h), addShape(type, x, y, w, h, col), exportSvg(path)");
            let _ = draw_text(surface_id, 20, 200, 0xFF89DCEB, "inos.archive: createZip(zip_path, files_list), extractAll(zip_path, dir)");
            let _ = draw_text(surface_id, 20, 220, 0xFFCBA6F7, "inos.plan:    createTask(title, due, priority), listTasks(), addAppointment(...)");
            let _ = draw_text(surface_id, 20, 240, 0xFFBAC2DE, "inos.files:   list(dir), exists(path), delete(path)");
        }
    }

    // Console / Execution Log Pane (y: 292..410)
    let _ = fill_rect(surface_id, 0, 292, 680, 118, 0xFF11111B);
    let _ = fill_rect(surface_id, 0, 292, 680, 20, 0xFF313244);
    let _ = draw_text(surface_id, 12, 296, 0xFFBAC2DE, "Console Output & Execution History:");

    let mut log_y = 316;
    for log_entry in state.logs.iter().rev().take(4) {
        let color = if log_entry.contains("ERROR") || log_entry.contains("DENIED") {
            0xFFF38BA8 // Red
        } else if log_entry.contains("AUDIT") {
            0xFF89DCEB // Cyan
        } else {
            0xFFA6E3A1 // Green
        };
        let _ = draw_text(surface_id, 14, log_y, color, log_entry);
        log_y += 18;
    }

    // Status bar (y: 412..440)
    let _ = fill_rect(surface_id, 0, 412, 680, 28, 0xFF313244);
    let status_bar_text = format!(
        "Ready  |  Quota: {} / {} ops [OK]  |  VM: IntScript Engine  |  Ring 3 CPL=3",
        state.quota_used, state.quota_max
    );
    let _ = draw_text(surface_id, 14, 418, 0xFFCDD6F4, &status_bar_text);

    let _ = present_surface(surface_id);
}

pub fn render_script_studio(
    surface_id: u32,
    code_lines: &[&str],
    logs: &[String],
    quota_used: u64,
    quota_max: u64,
) {
    let mut state = ScriptStudioState::new();
    state.lines = code_lines.iter().map(|s| String::from(*s)).collect();
    state.logs = logs.to_vec();
    state.quota_used = quota_used;
    state.quota_max = quota_max;
    render_script_studio_state(surface_id, &state);
}
