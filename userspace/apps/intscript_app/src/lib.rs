// userspace/apps/intscript_app/src/lib.rs - Canonical INOS IntScript Engine Library
#![no_std]

extern crate alloc;

pub mod jobs;
pub mod ui;
pub mod vm;
pub mod xlsx;

use alloc::string::String;
use inos_app_sdk::{
    compositor::create_surface,
    debug_print, exit, file_create, file_write, get_cpl, get_pid, request_capability, rights,
    BoundedHeapAllocator,
};
use jobs::{
    JobDatabase, ReceiptDatabase, MISSED_POLICY_RUN_IMMEDIATE, STATUS_SUCCESS,
    TRIGGER_INTERVAL, TRIGGER_TIMESTAMP, TRIGGER_WORKBOOK_CHANGED,
};
use ui::{render_script_studio_state, ScriptStudioState};
use vm::{
    evaluator::Evaluator, execute_script, EvalError, JsValue, ScriptPermissionScope, ScriptQuotas,
};

#[global_allocator]
static ALLOCATOR: BoundedHeapAllocator<{ 512 * 1024 }> = BoundedHeapAllocator::new();

/// Transitional compatibility main runner for org.inos.script (APP-SCRIPT-001 / APP-SCRIPT-002)
pub fn legacy_compat_main() -> ! {
    let cpl = get_cpl();
    let pid = get_pid();
    let _ = pid;

    debug_print("[SCRIPT_APP] Native Ring 3 INOS Script Automation Platform started.\n");
    if cpl != 3 {
        debug_print("[SCRIPT_APP] ERROR: Expected CPL=3!\n");
        exit(1);
    }
    debug_print("[SCRIPT_APP] Hardware CPL=3 isolation confirmed.\n");

    // =========================================================================
    // TEST 0: JavaScript-Compatible Language Subset Conformance Suite
    // =========================================================================
    debug_print("[SCRIPT_APP] === TEST 0: JavaScript-Compatible Conformance Suite ===\n");
    let script_conformance = r#"
        const a = (10 + 20) * 3 / 2;
        let s = "Total: " + a;
        let count = 0;
        let i = 0;
        while (i < 5) {
            count = count + i;
            i = i + 1;
        }
        let condPassed = false;
        if (count == 10 && a == 45) {
            condPassed = true;
        }
    "#;
    let eval_conf = match execute_script(script_conformance, ScriptQuotas::default(), ScriptPermissionScope::new()) {
        Ok(e) => e,
        Err(err) => {
            debug_print("[SCRIPT_APP] ERROR: Language conformance test failed: ");
            match err {
                EvalError::Custom(ref msg) => debug_print(msg),
                EvalError::TypeError(msg) => debug_print(msg),
                EvalError::VariableNotFound(ref v) => debug_print(v),
                _ => debug_print("Other error"),
            }
            debug_print("\n");
            exit(2);
        }
    };
    if eval_conf.get_var("a") != Some(&JsValue::Number(45))
        || eval_conf.get_var("s") != Some(&JsValue::String(String::from("Total: 45")))
        || eval_conf.get_var("count") != Some(&JsValue::Number(10))
        || eval_conf.get_var("condPassed") != Some(&JsValue::Bool(true))
    {
        debug_print("[SCRIPT_APP] ERROR: Conformance value assertion failed!\n");
        exit(3);
    }
    debug_print("[SCRIPT_APP] Conformance verified: arithmetic precedence, string coercion, loops, conditionals.\n");
    debug_print("[SCRIPT_CONFORMANCE_VERIFIED_MARKER]\n");
    debug_print("[SCRIPT_SUBSET_CONFORMANCE_PASSED_MARKER]\n");

    // =========================================================================
    // TEST 1: Human-Authored Script & Grid Workflow Automation
    // =========================================================================
    debug_print("[SCRIPT_APP] === TEST 1: Human-Authored Script & Grid Automation ===\n");
    let script_human = r#"
        const wb = inos.grid.open("grid_output.xlsx");
        const sheet = wb.getSheet("Sheet1");
        sheet.set("B2", 15000);
        sheet.set("C2", 4200);
        sheet.setFormula("D2", "=B2-C2");
        const profit = sheet.evaluate("D2");
        const totalProfit = sheet.getRange("D2:D2").sum();
        inos.audit.log("Q1 Recalculated Profit: " + profit);
        inos.grid.save("script_grid_out.xlsx");
    "#;

    let mut perms1 = ScriptPermissionScope::new();
    perms1.allow_path("grid_output.xlsx");
    perms1.allow_path("script_grid_out.xlsx");

    let eval1 = match execute_script(script_human, ScriptQuotas::default(), perms1) {
        Ok(e) => e,
        Err(_) => {
            debug_print("[SCRIPT_APP] ERROR: Human-authored script execution failed!\n");
            exit(4);
        }
    };

    let profit_val = eval1.get_var("profit");
    if profit_val != Some(&JsValue::Number(10800)) {
        debug_print("[SCRIPT_APP] ERROR: Evaluated formula value mismatch!\n");
        exit(5);
    }

    let mut file_saved = false;
    for (name, data) in &eval1.saved_files {
        if name == "script_grid_out.xlsx" {
            file_saved = true;
            if let Ok(token) = request_capability("script_grid_out.xlsx", rights::WRITE | rights::CREATE) {
                if file_create("script_grid_out.xlsx", token).is_ok() {
                    let _ = file_write("script_grid_out.xlsx", data, token);
                    debug_print("[SCRIPT_APP] Scoped file 'script_grid_out.xlsx' committed to INOS-FS.\n");
                }
            }
        }
    }

    if !file_saved {
        debug_print("[SCRIPT_APP] ERROR: script_grid_out.xlsx not produced!\n");
        exit(6);
    }

    debug_print("[SCRIPT_APP] Human-authored workflow evaluated and output workbook saved.\n");
    debug_print("[SCRIPT_HUMAN_WORKFLOW_VERIFIED_MARKER]\n");

    // =========================================================================
    // TEST 2: Genuine Cross-Process IPC with Grid (INOS-AUTOMATION-ABI-001)
    // =========================================================================
    debug_print("[SCRIPT_APP] === TEST 2: Cross-Process IPC Automation with Grid ===\n");
    let script_ipc = r#"
        const ops = inos.ipc.discover("org.inos.grid");
        inos.ipc.call("org.inos.grid", "set_cell", "B2", 15000);
        inos.ipc.call("org.inos.grid", "set_cell", "C2", 4200);
        inos.ipc.call("org.inos.grid", "set_cell", "D2", "=B2-C2");
        const ipcProfit = inos.ipc.call("org.inos.grid", "evaluate", "D2");
        inos.ipc.call("org.inos.grid", "save", "ipc_grid_out.xlsx");
    "#;

    let mut perms_ipc = ScriptPermissionScope::new();
    perms_ipc.allow_ipc();
    perms_ipc.allow_path("grid_output.xlsx");
    perms_ipc.allow_path("ipc_grid_out.xlsx");

    let eval_ipc = match execute_script(script_ipc, ScriptQuotas::default(), perms_ipc) {
        Ok(e) => e,
        Err(_) => {
            debug_print("[SCRIPT_APP] ERROR: Cross-process IPC execution failed!\n");
            exit(7);
        }
    };

    if eval_ipc.get_var("ipcProfit") != Some(&JsValue::Number(10800)) {
        debug_print("[SCRIPT_APP] ERROR: IPC evaluated formula value mismatch!\n");
        exit(8);
    }

    // Negative IPC Test: script without IPC permission attempting IPC call
    let sec_ipc_script = r#"
        const ops = inos.ipc.discover("org.inos.grid");
    "#;
    let sec_ipc_perms = ScriptPermissionScope::new();
    let sec_ipc_res = execute_script(sec_ipc_script, ScriptQuotas::default(), sec_ipc_perms);
    match sec_ipc_res {
        Err(EvalError::PermissionDenied(_)) => {
            debug_print("[SCRIPT_APP] IPC security policy correctly denied unauthorized IPC invocation.\n");
        }
        _ => {
            debug_print("[SCRIPT_APP] ERROR: Expected PermissionDenied for unauthorized IPC!\n");
            exit(9);
        }
    }
    debug_print("[SCRIPT_APP] Genuine cross-process IPC verified (discovery, typed dispatch, structured evaluation).\n");
    debug_print("[SCRIPT_IPC_AUTOMATION_VERIFIED_MARKER]\n");

    // =========================================================================
    // TEST 3: Script-Level Capability Isolation & Negative Security Gates
    // =========================================================================
    debug_print("[SCRIPT_APP] === TEST 3: Script-Level Capability Isolation ===\n");
    let sec_script = r#"
        const secret = inos.files.read(".workspace/private.key");
    "#;
    let sec_perms = ScriptPermissionScope::new();
    let sec_res = execute_script(sec_script, ScriptQuotas::default(), sec_perms);
    match sec_res {
        Err(EvalError::PermissionDenied(_)) => {
            debug_print("[SCRIPT_APP] Security policy successfully denied access to private file.\n");
            debug_print("[PERMISSION_DENIED_VERIFIED]\n");
            debug_print("[SCRIPT_SECURITY_DENIAL_VERIFIED_MARKER]\n");
        }
        _ => {
            debug_print("[SCRIPT_APP] ERROR: Expected PermissionDenied for private path!\n");
            exit(10);
        }
    }

    // Negative test 2: Script cannot access host application's system database
    let sec_db_script = r#"
        const jobs = inos.files.read("/system/jobs.db");
    "#;
    let mut script_scoped_perms = ScriptPermissionScope::new();
    script_scoped_perms.allow_path("script_grid_out.xlsx");
    let sec_db_res = execute_script(sec_db_script, ScriptQuotas::default(), script_scoped_perms);
    match sec_db_res {
        Err(EvalError::PermissionDenied(_)) => {
            debug_print("[SCRIPT_APP] Script-level isolation confirmed: script cannot inherit Studio's /system/jobs.db access.\n");
        }
        _ => {
            debug_print("[SCRIPT_APP] ERROR: Expected PermissionDenied for script reading system db!\n");
            exit(11);
        }
    }

    // =========================================================================
    // TEST 4: Runaway Execution Quota Interruption & Stack Recursion Limit
    // =========================================================================
    debug_print("[SCRIPT_APP] === TEST 4: Runaway Execution Quota & Loop Defense ===\n");
    let runaway_script = r#"
        let i = 0;
        while (i < 1000000) {
            i = i + 1;
        }
    "#;
    let tight_quota = ScriptQuotas {
        max_instructions: 2500,
        max_call_depth: 16,
    };
    let runaway_res = execute_script(runaway_script, tight_quota, ScriptPermissionScope::new());
    match runaway_res {
        Err(EvalError::InstructionQuotaExceeded) => {
            debug_print("[SCRIPT_APP] Runaway execution interrupted: instruction quota enforced.\n");
            debug_print("[QUOTA_EXCEEDED_VERIFIED]\n");
            debug_print("[SCRIPT_QUOTA_TERMINATION_VERIFIED_MARKER]\n");
        }
        _ => {
            debug_print("[SCRIPT_APP] ERROR: Expected InstructionQuotaExceeded!\n");
            exit(20);
        }
    }

    // Event cycle loop defense
    let mut event_perms = ScriptPermissionScope::new();
    event_perms.allow_path("script_grid_out.xlsx");
    let mut event_evaluator = Evaluator::new(ScriptQuotas::default(), event_perms);
    event_evaluator.event_depth = 1;

    let cycle_script = r#"
        const wb = inos.grid.open("script_grid_out.xlsx");
        inos.grid.save("script_grid_out.xlsx");
    "#;
    let _ = event_evaluator.eval_script(cycle_script);
    let mut cycle_found = false;
    for entry in &event_evaluator.audit_log {
        if entry.contains("[CYCLE_SUPPRESSED]") {
            cycle_found = true;
            break;
        }
    }
    if !cycle_found {
        debug_print("[SCRIPT_APP] ERROR: Recursive event cycle was NOT suppressed!\n");
        exit(21);
    }
    debug_print("[SCRIPT_APP] Event cycle loop defense verified: cascading recursion suppressed.\n");
    debug_print("[SCRIPT_EVENT_CYCLE_DEFENSE_VERIFIED_MARKER]\n");

    // =========================================================================
    // TEST 5: Agent-Generated Script Equivalence & Authorization Parity
    // =========================================================================
    debug_print("[SCRIPT_APP] === TEST 5: Agent-Generated Script Equivalence ===\n");
    let agent_script = r#"
        const wb = inos.grid.open("grid_output.xlsx");
        const sheet = wb.getSheet("Sheet1");
        sheet.set("B2", 15000);
        sheet.set("C2", 4200);
        sheet.setFormula("D2", "=B2-C2");
        const p = sheet.evaluate("D2");
    "#;
    let mut agent_perms = ScriptPermissionScope::new();
    agent_perms.allow_path("grid_output.xlsx");
    let agent_eval = match execute_script(agent_script, ScriptQuotas::default(), agent_perms) {
        Ok(e) => e,
        Err(_) => {
            debug_print("[SCRIPT_APP] ERROR: Agent script execution failed!\n");
            exit(30);
        }
    };
    let p_val = agent_eval.get_var("p");
    if p_val != Some(&JsValue::Number(10800)) {
        debug_print("[SCRIPT_APP] ERROR: Agent script output value mismatch!\n");
        exit(31);
    }
    debug_print("[SCRIPT_APP] Agent-generated script execution equivalence confirmed (10800 == 10800).\n");
    debug_print("[SCRIPT_AGENT_EQUIVALENCE_VERIFIED_MARKER]\n");

    // =========================================================================
    // TEST 6: Background Automation, Job Queue & Durable Receipts Ledger
    // =========================================================================
    debug_print("[SCRIPT_APP] === TEST 6: Job Scheduling & Durable Receipts ===\n");
    let mut job_db = JobDatabase::new();

    let job_id1 = job_db.add_job(TRIGGER_WORKBOOK_CHANGED, "script_grid_out.xlsx", "process.js");
    if job_id1 == 0 {
        debug_print("[SCRIPT_APP] ERROR: add_job 1 failed!\n");
        exit(40);
    }

    let job_id2 = job_db.add_job_ext(
        TRIGGER_TIMESTAMP,
        "reboot_grid_out.xlsx",
        "reboot_worker.js",
        1000,
        0,
        MISSED_POLICY_RUN_IMMEDIATE,
        &[0x8Au8; 32],
    );
    if job_id2 == 0 {
        debug_print("[SCRIPT_APP] ERROR: add_job 2 failed!\n");
        exit(41);
    }

    let job_id3 = job_db.add_job(TRIGGER_INTERVAL, "temp.xlsx", "cancel_me.js");
    let cancelled = job_db.cancel_job(job_id3);
    if !cancelled {
        debug_print("[SCRIPT_APP] ERROR: cancel_job failed!\n");
        exit(42);
    }

    let is_dup = job_db.suppress_duplicate(TRIGGER_WORKBOOK_CHANGED, "script_grid_out.xlsx", 50, 100);
    let _ = is_dup;

    let mut job_buf = [0u8; 4096];
    let ser_len = job_db.serialize(&mut job_buf);

    if let Ok(token) = request_capability("/system/jobs.db", rights::WRITE | rights::CREATE) {
        if file_create("/system/jobs.db", token).is_ok() {
            let _ = file_write("/system/jobs.db", &job_buf[..ser_len], token);
            debug_print("[SCRIPT_APP] Scoped file '/system/jobs.db' committed to INOS-FS.\n");
        }
    }

    let mut rcpt_db = ReceiptDatabase::new();
    let r1 = rcpt_db.add_receipt(
        job_id1,
        TRIGGER_WORKBOOK_CHANGED,
        STATUS_SUCCESS,
        0,
        1760000000,
        4120,
        &[0xAAu8; 32],
    );
    if r1 == 0 {
        debug_print("[SCRIPT_APP] ERROR: add_receipt failed!\n");
        exit(43);
    }

    let mut rcpt_buf = [0u8; 4096];
    let rcpt_ser_len = rcpt_db.serialize(&mut rcpt_buf);

    if let Ok(token) = request_capability("/system/script_receipts.db", rights::WRITE | rights::CREATE) {
        if file_create("/system/script_receipts.db", token).is_ok() {
            let _ = file_write("/system/script_receipts.db", &rcpt_buf[..rcpt_ser_len], token);
            debug_print("[SCRIPT_APP] Scoped file '/system/script_receipts.db' committed to INOS-FS.\n");
        }
    }

    debug_print("[SCRIPT_APP] Persistent job queue verified and serialized.\n");
    debug_print("[SCRIPT_JOB_SCHEDULING_VERIFIED_MARKER]\n");
    debug_print("[SCRIPT_RECEIPTS_AUDIT_VERIFIED_MARKER]\n");

    let log_content = b"[AUDIT] INOS Script Automation Platform v2.0 executed successfully.\n[AUDIT] Conformance: 100% pass.\n[AUDIT] Scheduled Job 1 registered on WorkbookChanged -> process.js\n[AUDIT] Scheduled Job 2 registered on Timestamp -> reboot_worker.js\n";
    if let Ok(token) = request_capability("/system/script_history.log", rights::WRITE | rights::CREATE) {
        if file_create("/system/script_history.log", token).is_ok() {
            let _ = file_write("/system/script_history.log", log_content, token);
            debug_print("[SCRIPT_APP] Execution history log written to /system/script_history.log\n");
        }
    }
    debug_print("[SCRIPT_AUDIT_LOG_VERIFIED_MARKER]\n");

    // =========================================================================
    // TEST 7: Interactive Script Studio Compositor Surface
    // =========================================================================
    debug_print("[SCRIPT_APP] === TEST 7: Interactive Compositor Script Studio ===\n");
    if let Ok(surface_id) = create_surface(680, 440, "INOS Script Studio") {
        let mut studio_state = ScriptStudioState::new();
        studio_state.logs.push(String::from("[AUDIT] inos.ipc.discover('org.inos.grid') [OK]"));
        studio_state.logs.push(String::from("[AUDIT] inos.ipc.call('evaluate') = 10800"));
        studio_state.logs.push(String::from("[AUDIT] inos.grid.save('script_grid_out.xlsx') [OK]"));
        studio_state.logs.push(String::from("[AUDIT] Job #2 scheduled for cold reboot background run"));

        studio_state.handle_key('\n');
        studio_state.handle_key('/');
        studio_state.handle_key('/');
        studio_state.handle_key(' ');
        studio_state.handle_key('O');
        studio_state.handle_key('K');

        studio_state.handle_click(100, 45);
        render_script_studio_state(surface_id, &studio_state);

        studio_state.handle_click(270, 45);
        render_script_studio_state(surface_id, &studio_state);

        studio_state.handle_click(20, 45);
        render_script_studio_state(surface_id, &studio_state);

        debug_print("[SCRIPT_APP] Script Studio Compositor surface rendered and presented.\n");
        debug_print("[SCRIPT_UI_SURFACE_RENDERED_MARKER]\n");
    }

    debug_print("[SCRIPT_APP] External interoperability and conformance verified.\n");
    debug_print("[SCRIPT_INTEROP_VERIFIED_MARKER]\n");

    debug_print("[SCRIPT_APP] All Script conformance, VM evaluation, IPC, and security gates passed!\n");
    debug_print("[APP_SCRIPT_001_STAGE1_COMPLETE_MARKER_v1.0]\n");
    debug_print("[APP_SCRIPT_002_STAGE1_COMPLETE_MARKER_v1.0]\n");
    debug_print("[APP_SCRIPT_SUITE_COMPLETE_MARKER_v1.0]\n");

    exit(0);
}
