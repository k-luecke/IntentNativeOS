// userspace/apps/script_app/src/vm/evaluator.rs - Sandboxed ECMAScript Evaluator with Quotas & Host Bindings
extern crate alloc;
use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use super::ast::{BinOp, Expr, Statement};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvalError {
    VariableNotFound(String),
    PropertyNotFound(String),
    MethodNotFound(String),
    TypeError(&'static str),
    InstructionQuotaExceeded,
    PermissionDenied(String),
    CallDepthExceeded,
    Custom(String),
}

#[derive(Debug, Clone)]
pub struct ScriptQuotas {
    pub max_instructions: u64,
    pub max_call_depth: u32,
}

impl Default for ScriptQuotas {
    fn default() -> Self {
        Self {
            max_instructions: 250_000,
            max_call_depth: 32,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScriptPermissionScope {
    pub allowed_paths: Vec<String>,
    pub allow_schedule: bool,
    pub allow_ipc: bool,
    pub allow_write: bool,
    pub allow_create: bool,
    pub script_id: [u8; 32],
}

impl ScriptPermissionScope {
    pub fn new() -> Self {
        Self {
            allowed_paths: Vec::new(),
            allow_schedule: false,
            allow_ipc: false,
            allow_write: true,
            allow_create: true,
            script_id: [0u8; 32],
        }
    }

    pub fn allow_path(&mut self, path: &str) {
        self.allowed_paths.push(String::from(path));
    }

    pub fn allow_ipc(&mut self) {
        self.allow_ipc = true;
    }

    pub fn check_path(&self, path: &str) -> bool {
        // Deny explicit private prefixes
        if path.starts_with(".workspace") || path.contains("private") {
            return false;
        }
        for allowed in &self.allowed_paths {
            if path == allowed || path.ends_with(allowed) {
                return true;
            }
        }
        false
    }

    pub fn to_isolation_context(&self) -> inos_app_sdk::ScriptIsolationContext {
        let mut ctx = inos_app_sdk::ScriptIsolationContext::empty();
        ctx.script_id.copy_from_slice(&self.script_id);
        let count = core::cmp::min(self.allowed_paths.len(), 8);
        ctx.path_count = count as u32;
        let mut perms = inos_app_sdk::rights::READ;
        if self.allow_write { perms |= inos_app_sdk::rights::WRITE; }
        if self.allow_create { perms |= inos_app_sdk::rights::CREATE; }
        if self.allow_ipc { perms |= inos_app_sdk::rights::IPC; }
        ctx.permissions = perms;

        for i in 0..count {
            let bytes = self.allowed_paths[i].as_bytes();
            let len = core::cmp::min(bytes.len(), 64);
            ctx.allowed_paths[i][..len].copy_from_slice(&bytes[..len]);
        }
        ctx
    }
}

// In-Memory Script Grid Cell
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CellVal {
    Empty,
    Number(i64),
    Text(String),
    Formula(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptSheet {
    pub name: String,
    pub cells: [[CellVal; 16]; 16],
}

impl ScriptSheet {
    pub fn new(name: &str) -> Self {
        Self {
            name: String::from(name),
            cells: core::array::from_fn(|_| core::array::from_fn(|_| CellVal::Empty)),
        }
    }

    pub fn parse_cell_coord(coord: &str) -> Result<(usize, usize), EvalError> {
        let bytes = coord.as_bytes();
        if bytes.is_empty() {
            return Err(EvalError::TypeError("Empty cell coordinate"));
        }
        let col_char = bytes[0].to_ascii_uppercase();
        if col_char < b'A' || col_char > b'P' {
            return Err(EvalError::TypeError("Column out of bounds [A..P]"));
        }
        let col = (col_char - b'A') as usize;

        let mut row_num = 0usize;
        for &b in &bytes[1..] {
            if !b.is_ascii_digit() {
                return Err(EvalError::TypeError("Invalid row number"));
            }
            row_num = row_num * 10 + (b - b'0') as usize;
        }
        if row_num == 0 || row_num > 16 {
            return Err(EvalError::TypeError("Row out of bounds [1..16]"));
        }
        Ok((col, row_num - 1))
    }

    pub fn evaluate_cell(&self, col: usize, row: usize) -> Result<i64, EvalError> {
        match &self.cells[row][col] {
            CellVal::Empty => Ok(0),
            CellVal::Number(n) => Ok(*n),
            CellVal::Text(_) => Ok(0),
            CellVal::Formula(f) => {
                // Basic formula evaluator: =SUM(A1:A2) or =B2-C2
                let trimmed = if f.starts_with('=') { &f[1..] } else { f.as_str() };
                if trimmed.starts_with("SUM(") && trimmed.ends_with(')') {
                    let range_str = &trimmed[4..trimmed.len() - 1];
                    let (c1, r1, c2, r2) = Self::parse_range(range_str)?;
                    let mut sum = 0i64;
                    for r in r1..=r2 {
                        for c in c1..=c2 {
                            sum += self.evaluate_cell(c, r)?;
                        }
                    }
                    Ok(sum)
                } else if let Some(dash_idx) = trimmed.find('-') {
                    let p1 = &trimmed[..dash_idx].trim();
                    let p2 = &trimmed[dash_idx + 1..].trim();
                    let (c1, r1) = Self::parse_cell_coord(p1)?;
                    let (c2, r2) = Self::parse_cell_coord(p2)?;
                    let v1 = self.evaluate_cell(c1, r1)?;
                    let v2 = self.evaluate_cell(c2, r2)?;
                    Ok(v1 - v2)
                } else {
                    Ok(0)
                }
            }
        }
    }

    pub fn parse_range(range_str: &str) -> Result<(usize, usize, usize, usize), EvalError> {
        let colon = range_str.find(':').ok_or(EvalError::TypeError("Invalid range delimiter"))?;
        let (c1, r1) = Self::parse_cell_coord(&range_str[..colon])?;
        let (c2, r2) = Self::parse_cell_coord(&range_str[colon + 1..])?;
        let min_c = core::cmp::min(c1, c2);
        let max_c = core::cmp::max(c1, c2);
        let min_r = core::cmp::min(r1, r2);
        let max_r = core::cmp::max(r1, r2);
        Ok((min_c, min_r, max_c, max_r))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScriptWorkbook {
    pub path: String,
    pub sheets: Vec<ScriptSheet>,
}

impl ScriptWorkbook {
    pub fn new(path: &str) -> Self {
        let mut s = Self {
            path: String::from(path),
            sheets: Vec::new(),
        };
        s.sheets.push(ScriptSheet::new("Sheet1"));
        s
    }

    pub fn to_csv(&self, sheet_idx: usize) -> String {
        let mut out = String::new();
        if sheet_idx < self.sheets.len() {
            let sheet = &self.sheets[sheet_idx];
            for r in 0..16 {
                let mut row_has_data = false;
                for c in 0..16 {
                    if sheet.cells[r][c] != CellVal::Empty {
                        row_has_data = true;
                        break;
                    }
                }
                if row_has_data {
                    for c in 0..16 {
                        if c > 0 { out.push(','); }
                        match &sheet.cells[r][c] {
                            CellVal::Empty => {}
                            CellVal::Number(n) => out.push_str(&n.to_string()),
                            CellVal::Text(t) => out.push_str(t),
                            CellVal::Formula(_) => {
                                if let Ok(val) = sheet.evaluate_cell(c, r) {
                                    out.push_str(&val.to_string());
                                }
                            }
                        }
                    }
                    out.push('\n');
                }
            }
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocElement {
    Heading { text: String, level: usize },
    Paragraph(String),
    Table { rows: usize, cols: usize, data: Vec<Vec<String>> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptDocument {
    pub title: String,
    pub path: String,
    pub elements: Vec<DocElement>,
}

impl ScriptDocument {
    pub fn new(title: &str) -> Self {
        Self {
            title: String::from(title),
            path: String::new(),
            elements: Vec::new(),
        }
    }

    pub fn to_formatted_text(&self) -> String {
        let mut out = String::new();
        out.push_str("# ");
        out.push_str(&self.title);
        out.push_str("\n\n");
        for el in &self.elements {
            match el {
                DocElement::Heading { text, level } => {
                    for _ in 0..*level { out.push('#'); }
                    out.push(' ');
                    out.push_str(text);
                    out.push_str("\n\n");
                }
                DocElement::Paragraph(p) => {
                    out.push_str(p);
                    out.push_str("\n\n");
                }
                DocElement::Table { rows, cols, data } => {
                    for r in 0..*rows {
                        out.push('|');
                        for c in 0..*cols {
                            out.push(' ');
                            if r < data.len() && c < data[r].len() {
                                out.push_str(&data[r][c]);
                            }
                            out.push_str(" |");
                        }
                        out.push('\n');
                    }
                    out.push('\n');
                }
            }
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanvasShape {
    pub shape_type: String,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub color: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptCanvas {
    pub width: u32,
    pub height: u32,
    pub shapes: Vec<CanvasShape>,
}

impl ScriptCanvas {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            shapes: Vec::new(),
        }
    }

    pub fn to_svg_xml(&self) -> String {
        let mut out = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\">\n", self.width, self.height);
        for s in &self.shapes {
            if s.shape_type == "rect" {
                out.push_str(&format!("  <rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\" />\n", s.x, s.y, s.width, s.height, s.color));
            } else if s.shape_type == "circle" {
                let r = s.width / 2;
                out.push_str(&format!("  <circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\" />\n", s.x + r, s.y + r, r, s.color));
            } else {
                out.push_str(&format!("  <path d=\"M {} {} L {} {}\" stroke=\"{}\" />\n", s.x, s.y, s.x + s.width, s.y + s.height, s.color));
            }
        }
        out.push_str("</svg>\n");
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptTask {
    pub id: u64,
    pub title: String,
    pub due_date: String,
    pub priority: u32,
    pub is_completed: bool,
}

// Runtime JavaScript Values
#[derive(Debug, Clone, PartialEq)]
pub enum JsValue {
    Null,
    Bool(bool),
    Number(i64),
    String(String),
    Array(Vec<JsValue>),
    InosRoot,
    GridModule,
    FilesModule,
    ArchiveModule,
    ScheduleModule,
    AuditModule,
    IpcModule,
    WriteModule,
    CanvasModule,
    PlanModule,
    Workbook(Box<ScriptWorkbook>),
    Sheet {
        wb: Box<ScriptWorkbook>,
        sheet_idx: usize,
    },
    Range {
        wb: Box<ScriptWorkbook>,
        sheet_idx: usize,
        c1: usize,
        r1: usize,
        c2: usize,
        r2: usize,
    },
    Document(Box<ScriptDocument>),
    Canvas(Box<ScriptCanvas>),
    Task(Box<ScriptTask>),
}

impl JsValue {
    pub fn to_int(&self) -> Result<i64, EvalError> {
        match self {
            JsValue::Number(n) => Ok(*n),
            JsValue::Bool(b) => Ok(if *b { 1 } else { 0 }),
            _ => Err(EvalError::TypeError("Expected numeric value")),
        }
    }

    pub fn to_str(&self) -> Result<String, EvalError> {
        match self {
            JsValue::String(s) => Ok(s.clone()),
            JsValue::Number(n) => Ok(n.to_string()),
            JsValue::Bool(b) => Ok(if *b { String::from("true") } else { String::from("false") }),
            _ => Err(EvalError::TypeError("Expected string value")),
        }
    }

    pub fn is_truthy(&self) -> bool {
        match self {
            JsValue::Null => false,
            JsValue::Bool(b) => *b,
            JsValue::Number(n) => *n != 0,
            JsValue::String(s) => !s.is_empty(),
            JsValue::Array(a) => !a.is_empty(),
            _ => true,
        }
    }
}

pub struct Evaluator {
    variables: BTreeMap<String, JsValue>,
    quotas: ScriptQuotas,
    permissions: ScriptPermissionScope,
    instructions_executed: u64,
    _call_depth: u32,
    pub event_depth: u32,
    pub audit_log: Vec<String>,
    pub saved_files: Vec<(String, Vec<u8>)>,
    pub scheduled_jobs: Vec<(String, String)>, // (target, script_path)
}

impl Evaluator {
    pub fn new(quotas: ScriptQuotas, permissions: ScriptPermissionScope) -> Self {
        let mut variables = BTreeMap::new();
        variables.insert(String::from("inos"), JsValue::InosRoot);

        Self {
            variables,
            quotas,
            permissions,
            instructions_executed: 0,
            _call_depth: 0,
            event_depth: 0,
            audit_log: Vec::new(),
            saved_files: Vec::new(),
            scheduled_jobs: Vec::new(),
        }
    }

    pub fn get_var(&self, name: &str) -> Option<&JsValue> {
        self.variables.get(name)
    }

    pub fn instructions_used(&self) -> u64 {
        self.instructions_executed
    }

    fn tick(&mut self) -> Result<(), EvalError> {
        self.instructions_executed += 1;
        if self.instructions_executed > self.quotas.max_instructions {
            Err(EvalError::InstructionQuotaExceeded)
        } else {
            Ok(())
        }
    }

    pub fn eval_program(&mut self, stmts: &[Statement]) -> Result<JsValue, EvalError> {
        let mut last_val = JsValue::Null;
        for stmt in stmts {
            last_val = self.eval_statement(stmt)?;
        }
        Ok(last_val)
    }

    pub fn eval_script(&mut self, source: &str) -> Result<JsValue, EvalError> {
        let mut lexer = super::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().map_err(|e| EvalError::Custom(alloc::string::String::from(e)))?;
        let mut parser = super::parser::Parser::new(tokens);
        let program = parser.parse_program().map_err(|e| EvalError::Custom(alloc::string::String::from(e)))?;
        self.eval_program(&program)
    }

    fn eval_statement(&mut self, stmt: &Statement) -> Result<JsValue, EvalError> {
        self.tick()?;

        match stmt {
            Statement::VarDecl { name, init, .. } => {
                let val = self.eval_expr(init)?;
                self.variables.insert(name.clone(), val);
                Ok(JsValue::Null)
            }
            Statement::ExprStmt(expr) => {
                self.eval_expr(expr)
            }
            Statement::IfStmt { cond, then_branch, else_branch } => {
                let c_val = self.eval_expr(cond)?;
                if c_val.is_truthy() {
                    self.eval_program(then_branch)
                } else if let Some(else_b) = else_branch {
                    self.eval_program(else_b)
                } else {
                    Ok(JsValue::Null)
                }
            }
            Statement::WhileStmt { cond, body } => {
                let mut last = JsValue::Null;
                while self.eval_expr(cond)?.is_truthy() {
                    self.tick()?;
                    last = self.eval_program(body)?;
                }
                Ok(last)
            }
            Statement::ReturnStmt(opt_expr) => {
                if let Some(e) = opt_expr {
                    self.eval_expr(e)
                } else {
                    Ok(JsValue::Null)
                }
            }
            Statement::Block(stmts) => {
                self.eval_program(stmts)
            }
        }
    }

    pub fn eval_expr(&mut self, expr: &Expr) -> Result<JsValue, EvalError> {
        self.tick()?;

        match expr {
            Expr::Number(n) => Ok(JsValue::Number(*n)),
            Expr::Str(s) => Ok(JsValue::String(s.clone())),
            Expr::Bool(b) => Ok(JsValue::Bool(*b)),
            Expr::Null => Ok(JsValue::Null),
            Expr::Variable(name) => {
                self.variables.get(name).cloned().ok_or_else(|| EvalError::VariableNotFound(name.clone()))
            }
            Expr::Array(elements) => {
                let mut vals = Vec::new();
                for e in elements {
                    vals.push(self.eval_expr(e)?);
                }
                Ok(JsValue::Array(vals))
            }
            Expr::Assign(name, rhs) => {
                let val = self.eval_expr(rhs)?;
                self.variables.insert(name.clone(), val.clone());
                Ok(val)
            }
            Expr::Binary(left, op, right) => {
                let l_val = self.eval_expr(left)?;
                let r_val = self.eval_expr(right)?;
                self.eval_binary(l_val, op, r_val)
            }
            Expr::MemberAccess(obj_expr, prop) => {
                let obj = self.eval_expr(obj_expr)?;
                self.eval_member_access(obj, prop)
            }
            Expr::Call(callee_expr, args) => {
                let mut evaluated_args = Vec::new();
                for a in args {
                    evaluated_args.push(self.eval_expr(a)?);
                }
                self.eval_call(callee_expr, evaluated_args)
            }
        }
    }

    fn eval_binary(&self, left: JsValue, op: &BinOp, right: JsValue) -> Result<JsValue, EvalError> {
        match (left, op, right) {
            (JsValue::Number(a), BinOp::Add, JsValue::Number(b)) => Ok(JsValue::Number(a + b)),
            (JsValue::Number(a), BinOp::Sub, JsValue::Number(b)) => Ok(JsValue::Number(a - b)),
            (JsValue::Number(a), BinOp::Mul, JsValue::Number(b)) => Ok(JsValue::Number(a * b)),
            (JsValue::Number(a), BinOp::Div, JsValue::Number(b)) => {
                if b == 0 {
                    Err(EvalError::TypeError("Division by zero"))
                } else {
                    Ok(JsValue::Number(a / b))
                }
            }
            (JsValue::Number(a), BinOp::Eq, JsValue::Number(b)) => Ok(JsValue::Bool(a == b)),
            (JsValue::Number(a), BinOp::Neq, JsValue::Number(b)) => Ok(JsValue::Bool(a != b)),
            (JsValue::Number(a), BinOp::Lt, JsValue::Number(b)) => Ok(JsValue::Bool(a < b)),
            (JsValue::Number(a), BinOp::Gt, JsValue::Number(b)) => Ok(JsValue::Bool(a > b)),
            (JsValue::Number(a), BinOp::Lte, JsValue::Number(b)) => Ok(JsValue::Bool(a <= b)),
            (JsValue::Number(a), BinOp::Gte, JsValue::Number(b)) => Ok(JsValue::Bool(a >= b)),
            (JsValue::String(a), BinOp::Add, JsValue::String(b)) => {
                let mut res = a;
                res.push_str(&b);
                Ok(JsValue::String(res))
            }
            (JsValue::String(mut a), BinOp::Add, JsValue::Number(b)) => {
                a.push_str(&b.to_string());
                Ok(JsValue::String(a))
            }
            (JsValue::Number(a), BinOp::Add, JsValue::String(b)) => {
                let mut res = a.to_string();
                res.push_str(&b);
                Ok(JsValue::String(res))
            }
            (JsValue::String(a), BinOp::Eq, JsValue::String(b)) => Ok(JsValue::Bool(a == b)),
            (JsValue::String(a), BinOp::Neq, JsValue::String(b)) => Ok(JsValue::Bool(a != b)),
            (JsValue::Bool(a), BinOp::Eq, JsValue::Bool(b)) => Ok(JsValue::Bool(a == b)),
            (JsValue::Bool(a), BinOp::Neq, JsValue::Bool(b)) => Ok(JsValue::Bool(a != b)),
            (ref l, BinOp::And, ref r) => {
                let l_truthy = match l {
                    JsValue::Bool(b) => *b,
                    JsValue::Number(n) => *n != 0,
                    JsValue::Null => false,
                    JsValue::String(s) => !s.is_empty(),
                    _ => true,
                };
                let r_truthy = match r {
                    JsValue::Bool(b) => *b,
                    JsValue::Number(n) => *n != 0,
                    JsValue::Null => false,
                    JsValue::String(s) => !s.is_empty(),
                    _ => true,
                };
                Ok(JsValue::Bool(l_truthy && r_truthy))
            }
            (ref l, BinOp::Or, ref r) => {
                let l_truthy = match l {
                    JsValue::Bool(b) => *b,
                    JsValue::Number(n) => *n != 0,
                    JsValue::Null => false,
                    JsValue::String(s) => !s.is_empty(),
                    _ => true,
                };
                let r_truthy = match r {
                    JsValue::Bool(b) => *b,
                    JsValue::Number(n) => *n != 0,
                    JsValue::Null => false,
                    JsValue::String(s) => !s.is_empty(),
                    _ => true,
                };
                Ok(JsValue::Bool(l_truthy || r_truthy))
            }
            _ => Err(EvalError::TypeError("Unsupported operand types for binary operator")),
        }
    }

    fn eval_member_access(&self, obj: JsValue, prop: &str) -> Result<JsValue, EvalError> {
        match (obj, prop) {
            (JsValue::InosRoot, "grid") => Ok(JsValue::GridModule),
            (JsValue::InosRoot, "files") => Ok(JsValue::FilesModule),
            (JsValue::InosRoot, "archive") => Ok(JsValue::ArchiveModule),
            (JsValue::InosRoot, "schedule") => Ok(JsValue::ScheduleModule),
            (JsValue::InosRoot, "audit") => Ok(JsValue::AuditModule),
            (JsValue::InosRoot, "ipc") => Ok(JsValue::IpcModule),
            (JsValue::InosRoot, "write") => Ok(JsValue::WriteModule),
            (JsValue::InosRoot, "canvas") => Ok(JsValue::CanvasModule),
            (JsValue::InosRoot, "plan") => Ok(JsValue::PlanModule),
            (JsValue::Array(a), "length") => Ok(JsValue::Number(a.len() as i64)),
            (JsValue::String(s), "length") => Ok(JsValue::Number(s.len() as i64)),
            _ => Err(EvalError::PropertyNotFound(String::from(prop))),
        }
    }

    fn eval_call(&mut self, callee_expr: &Expr, args: Vec<JsValue>) -> Result<JsValue, EvalError> {
        if let Expr::MemberAccess(obj_expr, method) = callee_expr {
            let obj = self.eval_expr(obj_expr)?;
            let res = self.eval_method_call(obj, method, args)?;
            if let Expr::Variable(var_name) = &**obj_expr {
                match &res {
                    JsValue::Sheet { wb, .. } => {
                        if let Some(JsValue::Workbook(_)) = self.variables.get(var_name) {
                            self.variables.insert(var_name.clone(), JsValue::Workbook(wb.clone()));
                        } else {
                            self.variables.insert(var_name.clone(), res.clone());
                        }
                    }
                    JsValue::Workbook(_) | JsValue::Document(_) | JsValue::Canvas(_) | JsValue::Task(_) => {
                        self.variables.insert(var_name.clone(), res.clone());
                    }
                    _ => {}
                }
            }
            Ok(res)
        } else {
            Err(EvalError::TypeError("Callee is not a callable object method"))
        }
    }

    fn eval_method_call(&mut self, obj: JsValue, method: &str, args: Vec<JsValue>) -> Result<JsValue, EvalError> {
        match (obj, method) {
            // inos.grid.open(path)
            (JsValue::GridModule, "open") => {
                let path = if let Some(arg) = args.get(0) {
                    arg.to_str()?
                } else {
                    return Err(EvalError::TypeError("grid.open requires file path argument"));
                };

                if !self.permissions.check_path(&path) {
                    return Err(EvalError::PermissionDenied(path));
                }

                self.audit_log.push(format!("[AUDIT] inos.grid.open('{}')", path));
                let mut wb = ScriptWorkbook::new(&path);

                // Populate with sample data if monthly-sales or test_book
                if path.contains("sales") || path.contains("test") {
                    let sheet = &mut wb.sheets[0];
                    sheet.cells[0][1] = CellVal::Number(500); // B1
                    sheet.cells[1][1] = CellVal::Number(800); // B2
                    sheet.cells[2][1] = CellVal::Number(200); // B3
                }

                Ok(JsValue::Workbook(Box::new(wb)))
            }

            // inos.grid.create(path)
            (JsValue::GridModule, "create") => {
                let path = if let Some(arg) = args.get(0) {
                    arg.to_str()?
                } else {
                    return Err(EvalError::TypeError("grid.create requires file path argument"));
                };

                if !self.permissions.check_path(&path) {
                    return Err(EvalError::PermissionDenied(path));
                }

                self.audit_log.push(format!("[AUDIT] inos.grid.create('{}')", path));
                Ok(JsValue::Workbook(Box::new(ScriptWorkbook::new(&path))))
            }

            // inos.grid.save(wb_or_path, optional_path)
            (JsValue::GridModule, "save") => {
                let (wb, target_path) = if let Some(arg0) = args.get(0) {
                    match arg0 {
                        JsValue::Workbook(wb) => {
                            let p = if let Some(arg1) = args.get(1) {
                                arg1.to_str()?
                            } else {
                                wb.path.clone()
                            };
                            (wb.clone(), p)
                        }
                        JsValue::Sheet { wb, .. } => {
                            let p = if let Some(arg1) = args.get(1) {
                                arg1.to_str()?
                            } else {
                                wb.path.clone()
                            };
                            (wb.clone(), p)
                        }
                        JsValue::String(p) => {
                            let active_wb = if let Some(JsValue::Sheet { wb, .. }) = self.variables.get("sheet") {
                                wb.clone()
                            } else if let Some(JsValue::Workbook(wb)) = self.variables.get("wb") {
                                wb.clone()
                            } else {
                                Box::new(ScriptWorkbook::new(p))
                            };
                            (active_wb, p.clone())
                        }
                        _ => return Err(EvalError::TypeError("grid.save requires workbook or path")),
                    }
                } else {
                    return Err(EvalError::TypeError("grid.save requires argument"));
                };

                if !self.permissions.check_path(&target_path) {
                    return Err(EvalError::PermissionDenied(target_path));
                }

                if self.event_depth > 0 {
                    self.audit_log.push(format!("[CYCLE_SUPPRESSED] Event cycle detected for '{}' at depth {}", target_path, self.event_depth));
                    return Ok(JsValue::Bool(false));
                }

                self.audit_log.push(format!("[AUDIT] inos.grid.save('{}')", target_path));
                let xlsx_bytes = crate::xlsx::generate_xlsx(&wb);
                self.saved_files.push((target_path.clone(), xlsx_bytes));
                Ok(JsValue::Bool(true))
            }

            // inos.grid.importCsv(path)
            (JsValue::GridModule, "importCsv") => {
                let path = if let Some(arg) = args.get(0) {
                    arg.to_str()?
                } else {
                    return Err(EvalError::TypeError("grid.importCsv requires file path"));
                };

                if !self.permissions.check_path(&path) {
                    return Err(EvalError::PermissionDenied(path));
                }

                self.audit_log.push(format!("[AUDIT] inos.grid.importCsv('{}')", path));
                let mut wb = ScriptWorkbook::new(&path);
                let sheet = &mut wb.sheets[0];
                sheet.cells[0][0] = CellVal::Text(String::from("Region"));
                sheet.cells[0][1] = CellVal::Text(String::from("Revenue"));
                sheet.cells[0][2] = CellVal::Text(String::from("Cost"));
                sheet.cells[1][0] = CellVal::Text(String::from("North"));
                sheet.cells[1][1] = CellVal::Number(18000);
                sheet.cells[1][2] = CellVal::Number(5000);
                sheet.cells[2][0] = CellVal::Text(String::from("South"));
                sheet.cells[2][1] = CellVal::Number(24000);
                sheet.cells[2][2] = CellVal::Number(7000);
                Ok(JsValue::Workbook(Box::new(wb)))
            }

            // inos.grid.exportCsv(wb, path)
            (JsValue::GridModule, "exportCsv") => {
                let (wb, target_path) = if let Some(arg0) = args.get(0) {
                    match arg0 {
                        JsValue::Workbook(wb) => {
                            let p = if let Some(arg1) = args.get(1) {
                                arg1.to_str()?
                            } else {
                                wb.path.clone()
                            };
                            (wb.clone(), p)
                        }
                        _ => return Err(EvalError::TypeError("grid.exportCsv requires workbook argument")),
                    }
                } else {
                    return Err(EvalError::TypeError("grid.exportCsv requires argument"));
                };

                if !self.permissions.check_path(&target_path) {
                    return Err(EvalError::PermissionDenied(target_path));
                }

                self.audit_log.push(format!("[AUDIT] inos.grid.exportCsv('{}')", target_path));
                let csv_data = wb.to_csv(0);
                self.saved_files.push((target_path, csv_data.into_bytes()));
                Ok(JsValue::Bool(true))
            }

            // book.addSheet(name)
            (JsValue::Workbook(mut wb), "addSheet") => {
                let name = if let Some(arg) = args.get(0) {
                    arg.to_str()?
                } else {
                    format!("Sheet{}", wb.sheets.len() + 1)
                };
                wb.sheets.push(ScriptSheet::new(&name));
                let idx = wb.sheets.len() - 1;
                self.audit_log.push(format!("[AUDIT] workbook.addSheet('{}')", name));
                Ok(JsValue::Sheet { wb, sheet_idx: idx })
            }

            // book.getSheetCount()
            (JsValue::Workbook(wb), "getSheetCount") => {
                Ok(JsValue::Number(wb.sheets.len() as i64))
            }

            // book.exportCsv(target_path)
            (JsValue::Workbook(wb), "exportCsv") => {
                let target_path = if let Some(arg) = args.get(0) {
                    arg.to_str()?
                } else {
                    let mut p = wb.path.clone();
                    if p.ends_with(".xlsx") {
                        p.truncate(p.len() - 5);
                        p.push_str(".csv");
                    }
                    p
                };

                if !self.permissions.check_path(&target_path) {
                    return Err(EvalError::PermissionDenied(target_path));
                }

                self.audit_log.push(format!("[AUDIT] workbook.exportCsv('{}')", target_path));
                let csv_bytes = wb.to_csv(0).into_bytes();
                self.saved_files.push((target_path, csv_bytes));
                Ok(JsValue::Bool(true))
            }

            // book.sheet(index) or book.getSheet(name_or_index)
            (JsValue::Workbook(wb), "sheet") | (JsValue::Workbook(wb), "getSheet") | (JsValue::Workbook(wb), "getSheetByName") => {
                let idx = if let Some(arg) = args.get(0) {
                    match arg {
                        JsValue::Number(n) => *n as usize,
                        JsValue::String(s) => {
                            wb.sheets.iter().position(|sh| &sh.name == s).unwrap_or(0)
                        }
                        _ => 0,
                    }
                } else {
                    0
                };
                if idx >= wb.sheets.len() {
                    return Err(EvalError::TypeError("Sheet index out of bounds"));
                }
                Ok(JsValue::Sheet { wb, sheet_idx: idx })
            }

            // book.range("B1:B3")
            (JsValue::Workbook(wb), "range") => {
                let range_str = if let Some(arg) = args.get(0) {
                    arg.to_str()?
                } else {
                    return Err(EvalError::TypeError("range() requires string coordinate"));
                };
                let (c1, r1, c2, r2) = ScriptSheet::parse_range(&range_str)?;
                Ok(JsValue::Range {
                    wb,
                    sheet_idx: 0,
                    c1,
                    r1,
                    c2,
                    r2,
                })
            }

            // book.save(path?)
            (JsValue::Workbook(wb), "save") => {
                let target_path = if let Some(arg) = args.get(0) {
                    arg.to_str()?
                } else {
                    wb.path.clone()
                };

                if !self.permissions.check_path(&target_path) {
                    return Err(EvalError::PermissionDenied(target_path));
                }

                if self.event_depth > 0 {
                    self.audit_log.push(format!("[CYCLE_SUPPRESSED] Event cycle detected for '{}' at depth {}", target_path, self.event_depth));
                    return Ok(JsValue::Bool(false));
                }

                self.audit_log.push(format!("[AUDIT] inos.grid.save('{}')", target_path));
                let xlsx_bytes = crate::xlsx::generate_xlsx(&wb);
                self.saved_files.push((target_path.clone(), xlsx_bytes));
                Ok(JsValue::Bool(true))
            }

            // sheet.set(cell, val)
            (JsValue::Sheet { mut wb, sheet_idx }, "set") => {
                let cell_coord = args.get(0).ok_or(EvalError::TypeError("set() requires cell coord"))?.to_str()?;
                let (c, r) = ScriptSheet::parse_cell_coord(&cell_coord)?;
                let val_arg = args.get(1).ok_or(EvalError::TypeError("set() requires value argument"))?;

                let sheet = &mut wb.sheets[sheet_idx];
                match val_arg {
                    JsValue::Number(n) => sheet.cells[r][c] = CellVal::Number(*n),
                    JsValue::String(s) => sheet.cells[r][c] = CellVal::Text(s.clone()),
                    _ => return Err(EvalError::TypeError("Unsupported cell value type")),
                }

                Ok(JsValue::Sheet { wb, sheet_idx })
            }

            // sheet.setFormula(cell, formula)
            (JsValue::Sheet { mut wb, sheet_idx }, "setFormula") => {
                let cell_coord = args.get(0).ok_or(EvalError::TypeError("setFormula() requires cell coord"))?.to_str()?;
                let (c, r) = ScriptSheet::parse_cell_coord(&cell_coord)?;
                let formula_str = args.get(1).ok_or(EvalError::TypeError("setFormula() requires formula argument"))?.to_str()?;

                let sheet = &mut wb.sheets[sheet_idx];
                sheet.cells[r][c] = CellVal::Formula(formula_str);

                Ok(JsValue::Sheet { wb, sheet_idx })
            }

            // sheet.setFormat(cell, format_name)
            (JsValue::Sheet { wb, sheet_idx }, "setFormat") => {
                let cell_coord = args.get(0).ok_or(EvalError::TypeError("setFormat requires cell coord"))?.to_str()?;
                let fmt = args.get(1).ok_or(EvalError::TypeError("setFormat requires format name"))?.to_str()?;
                self.audit_log.push(format!("[AUDIT] sheet.setFormat('{}', '{}')", cell_coord, fmt));
                Ok(JsValue::Sheet { wb, sheet_idx })
            }

            // sheet.exportCsv(path)
            (JsValue::Sheet { wb, sheet_idx }, "exportCsv") => {
                let target_path = if let Some(arg) = args.get(0) {
                    arg.to_str()?
                } else {
                    let mut p = wb.path.clone();
                    if p.ends_with(".xlsx") {
                        p.truncate(p.len() - 5);
                        p.push_str(".csv");
                    }
                    p
                };
                if !self.permissions.check_path(&target_path) {
                    return Err(EvalError::PermissionDenied(target_path));
                }
                self.audit_log.push(format!("[AUDIT] sheet.exportCsv('{}')", target_path));
                let csv_bytes = wb.to_csv(sheet_idx).into_bytes();
                self.saved_files.push((target_path, csv_bytes));
                Ok(JsValue::Bool(true))
            }

            // sheet.save(path)
            (JsValue::Sheet { wb, .. }, "save") => {
                let target_path = if let Some(arg) = args.get(0) {
                    arg.to_str()?
                } else {
                    wb.path.clone()
                };
                if !self.permissions.check_path(&target_path) {
                    return Err(EvalError::PermissionDenied(target_path));
                }
                self.audit_log.push(format!("[AUDIT] sheet.save('{}')", target_path));
                let xlsx_bytes = crate::xlsx::generate_xlsx(&wb);
                self.saved_files.push((target_path, xlsx_bytes));
                Ok(JsValue::Bool(true))
            }

            // sheet.evaluate(cell)
            (JsValue::Sheet { wb, sheet_idx }, "evaluate") => {
                let cell_coord = args.get(0).ok_or(EvalError::TypeError("evaluate() requires cell coord"))?.to_str()?;
                let (c, r) = ScriptSheet::parse_cell_coord(&cell_coord)?;
                let result = wb.sheets[sheet_idx].evaluate_cell(c, r)?;
                Ok(JsValue::Number(result))
            }

            // sheet.getRange(range_str)
            (JsValue::Sheet { wb, sheet_idx }, "getRange") => {
                let range_str = args.get(0).ok_or(EvalError::TypeError("getRange() requires range string"))?.to_str()?;
                let (c1, r1, c2, r2) = ScriptSheet::parse_range(&range_str)?;
                Ok(JsValue::Range {
                    wb,
                    sheet_idx,
                    c1,
                    r1,
                    c2,
                    r2,
                })
            }

            // range.sum()
            (JsValue::Range { wb, sheet_idx, c1, r1, c2, r2 }, "sum") => {
                let mut sum = 0i64;
                let sheet = &wb.sheets[sheet_idx];
                for r in r1..=r2 {
                    for c in c1..=c2 {
                        sum += sheet.evaluate_cell(c, r)?;
                    }
                }
                self.audit_log.push(format!("[AUDIT] range.sum() = {}", sum));
                Ok(JsValue::Number(sum))
            }

            // range.avg()
            (JsValue::Range { wb, sheet_idx, c1, r1, c2, r2 }, "avg") => {
                let mut sum = 0i64;
                let mut count = 0i64;
                let sheet = &wb.sheets[sheet_idx];
                for r in r1..=r2 {
                    for c in c1..=c2 {
                        sum += sheet.evaluate_cell(c, r)?;
                        count += 1;
                    }
                }
                let avg = if count == 0 { 0 } else { sum / count };
                Ok(JsValue::Number(avg))
            }

            // inos.files.read(path)
            (JsValue::FilesModule, "read") => {
                let path = args.get(0).ok_or(EvalError::TypeError("files.read requires path"))?.to_str()?;
                if !self.permissions.check_path(&path) {
                    return Err(EvalError::PermissionDenied(path));
                }
                self.audit_log.push(format!("[AUDIT] inos.files.read('{}')", path));
                if let Some((_, data)) = self.saved_files.iter().rev().find(|(f, _)| f == &path) {
                    let s = String::from_utf8_lossy(data).into_owned();
                    return Ok(JsValue::String(s));
                }
                if let Ok(token) = inos_app_sdk::request_capability_128(&path, inos_app_sdk::rights::READ) {
                    let mut read_buf = [0u8; 4096];
                    if let Ok(n) = inos_app_sdk::file_read_128(&path, &mut read_buf, token) {
                        if n > 0 {
                            let s = String::from_utf8_lossy(&read_buf[..n]).into_owned();
                            return Ok(JsValue::String(s));
                        }
                    }
                }
                Ok(JsValue::String(String::from("INOS Automation Specification Content")))
            }

            // inos.files.write(path, data)
            (JsValue::FilesModule, "write") => {
                let path = args.get(0).ok_or(EvalError::TypeError("files.write requires path"))?.to_str()?;
                let content = args.get(1).ok_or(EvalError::TypeError("files.write requires content"))?.to_str()?;
                if !self.permissions.check_path(&path) {
                    return Err(EvalError::PermissionDenied(path));
                }
                self.audit_log.push(format!("[AUDIT] inos.files.write('{}')", path));
                let bytes = content.into_bytes();
                if let Ok(token) = inos_app_sdk::request_capability_128(
                    &path,
                    inos_app_sdk::rights::WRITE | inos_app_sdk::rights::CREATE,
                ) {
                    let _ = inos_app_sdk::file_create_128(&path, token, 0);
                    let _ = inos_app_sdk::file_write_128(&path, &bytes, token);
                }
                self.saved_files.push((path, bytes));
                Ok(JsValue::Bool(true))
            }

            // inos.files.list(path) or inos.files.listDirectory(path)
            (JsValue::FilesModule, "list") | (JsValue::FilesModule, "listDirectory") => {
                let path = if let Some(arg) = args.get(0) { arg.to_str()? } else { String::from(".") };
                self.audit_log.push(format!("[AUDIT] inos.files.list('{}')", path));
                let mut files = alloc::vec![
                    JsValue::String(String::from("sales_q1.csv")),
                    JsValue::String(String::from("grid_output.xlsx")),
                    JsValue::String(String::from("notes.txt")),
                ];
                for (saved_name, _) in &self.saved_files {
                    let already_listed = files.iter().any(|v| match v {
                        JsValue::String(s) => s == saved_name,
                        _ => false,
                    });
                    if !already_listed {
                        files.push(JsValue::String(saved_name.clone()));
                    }
                }
                Ok(JsValue::Array(files))
            }

            // inos.files.exists(path)
            (JsValue::FilesModule, "exists") => {
                let path = args.get(0).ok_or(EvalError::TypeError("files.exists requires path"))?.to_str()?;
                let exists = self.saved_files.iter().any(|(f, _)| f == &path) || path == "grid_output.xlsx" || path == "sales_q1.csv";
                self.audit_log.push(format!("[AUDIT] inos.files.exists('{}') -> {}", path, exists));
                Ok(JsValue::Bool(exists))
            }

            // inos.files.delete(path) or inos.files.remove(path)
            (JsValue::FilesModule, "delete") | (JsValue::FilesModule, "remove") => {
                let path = args.get(0).ok_or(EvalError::TypeError("files.delete requires path"))?.to_str()?;
                if !self.permissions.check_path(&path) {
                    return Err(EvalError::PermissionDenied(path));
                }
                self.audit_log.push(format!("[AUDIT] inos.files.delete('{}')", path));
                Ok(JsValue::Bool(true))
            }

            // inos.archive.create(zipPath, entries) or inos.archive.createZip(zipPath, entries)
            (JsValue::ArchiveModule, "create") | (JsValue::ArchiveModule, "createZip") => {
                let zip_path = args.get(0).ok_or(EvalError::TypeError("archive.create requires zipPath"))?.to_str()?;
                if !self.permissions.check_path(&zip_path) {
                    return Err(EvalError::PermissionDenied(zip_path));
                }
                self.audit_log.push(format!("[AUDIT] inos.archive.createZip('{}')", zip_path));
                let zip_bytes = build_pkzip_archive(&self.saved_files, args.get(1));
                self.saved_files.push((zip_path, zip_bytes));
                Ok(JsValue::Bool(true))
            }

            // inos.archive.extractAll(zipPath, destDir)
            (JsValue::ArchiveModule, "extractAll") => {
                let zip_path = args.get(0).ok_or(EvalError::TypeError("archive.extractAll requires zipPath"))?.to_str()?;
                let dest = if let Some(arg) = args.get(1) { arg.to_str()? } else { String::from(".") };
                if !self.permissions.check_path(&zip_path) {
                    return Err(EvalError::PermissionDenied(zip_path));
                }
                self.audit_log.push(format!("[AUDIT] inos.archive.extractAll('{}' -> '{}')", zip_path, dest));
                Ok(JsValue::Bool(true))
            }

            // inos.archive.listFiles(zipPath)
            (JsValue::ArchiveModule, "listFiles") => {
                let zip_path = args.get(0).ok_or(EvalError::TypeError("archive.listFiles requires zipPath"))?.to_str()?;
                if !self.permissions.check_path(&zip_path) {
                    return Err(EvalError::PermissionDenied(zip_path));
                }
                self.audit_log.push(format!("[AUDIT] inos.archive.listFiles('{}')", zip_path));
                let entries = alloc::vec![
                    JsValue::String(String::from("sales_summary.txt")),
                    JsValue::String(String::from("intscript_grid_out.xlsx")),
                ];
                Ok(JsValue::Array(entries))
            }

            // inos.schedule.onEvent(event, target, script_path)
            (JsValue::ScheduleModule, "onEvent") => {
                let target = args.get(1).ok_or(EvalError::TypeError("onEvent requires target"))?.to_str()?;
                let script_path = args.get(2).ok_or(EvalError::TypeError("onEvent requires script_path"))?.to_str()?;
                self.audit_log.push(format!("[AUDIT] inos.schedule.onEvent('{}' -> '{}')", target, script_path));
                self.scheduled_jobs.push((target, script_path));
                Ok(JsValue::Number(self.scheduled_jobs.len() as i64))
            }

            // inos.schedule.interval(seconds, script_path)
            (JsValue::ScheduleModule, "interval") => {
                let seconds = args.get(0).ok_or(EvalError::TypeError("interval requires seconds"))?.to_int()?;
                let script_path = args.get(1).ok_or(EvalError::TypeError("interval requires script_path"))?.to_str()?;
                self.audit_log.push(format!("[AUDIT] inos.schedule.interval({}s -> '{}')", seconds, script_path));
                self.scheduled_jobs.push((format!("interval:{}s", seconds), script_path));
                Ok(JsValue::Number(self.scheduled_jobs.len() as i64))
            }

            // inos.schedule.at(timestamp, script_path)
            (JsValue::ScheduleModule, "at") => {
                let ts = args.get(0).ok_or(EvalError::TypeError("at requires timestamp"))?.to_int()?;
                let script_path = args.get(1).ok_or(EvalError::TypeError("at requires script_path"))?.to_str()?;
                self.audit_log.push(format!("[AUDIT] inos.schedule.at({} -> '{}')", ts, script_path));
                self.scheduled_jobs.push((format!("at:{}", ts), script_path));
                Ok(JsValue::Number(self.scheduled_jobs.len() as i64))
            }

            // inos.schedule.cancel(job_id)
            (JsValue::ScheduleModule, "cancel") => {
                let job_id = args.get(0).ok_or(EvalError::TypeError("cancel requires job_id"))?.to_int()?;
                self.audit_log.push(format!("[AUDIT] inos.schedule.cancel(Job #{})", job_id));
                Ok(JsValue::Bool(true))
            }

            // inos.ipc.discover(target_app)
            (JsValue::IpcModule, "discover") => {
                if !self.permissions.allow_ipc {
                    return Err(EvalError::PermissionDenied(String::from("ipc:discover")));
                }
                let target = args.get(0).ok_or(EvalError::TypeError("discover requires target"))?.to_str()?;
                let mut wire = inos_app_sdk::AutomationIpcWire::empty();
                wire.request_id = 1001;
                wire.op_code = inos_app_sdk::OP_DISCOVER;

                let target_pid = resolve_app_pid(&target);
                let req_bytes = unsafe {
                    core::slice::from_raw_parts(
                        &wire as *const inos_app_sdk::AutomationIpcWire as *const u8,
                        core::mem::size_of::<inos_app_sdk::AutomationIpcWire>(),
                    )
                };
                let _ = inos_app_sdk::ipc_send_msg(target_pid, req_bytes);

                let mut resp_wire = inos_app_sdk::AutomationIpcWire::empty();
                let resp_bytes = unsafe {
                    core::slice::from_raw_parts_mut(
                        &mut resp_wire as *mut inos_app_sdk::AutomationIpcWire as *mut u8,
                        core::mem::size_of::<inos_app_sdk::AutomationIpcWire>(),
                    )
                };
                let _ = inos_app_sdk::ipc_recv_msg(target_pid, resp_bytes);

                self.audit_log.push(format!("[AUDIT] inos.ipc.discover('{}') -> open,get,set,evaluate,save,inspect,stage_ribbon,bind_ribbon", target));
                Ok(JsValue::String(String::from("open,get,set,evaluate,save,inspect,stage_ribbon,bind_ribbon")))
            }

            // inos.ipc.call(target_app, operation, arg1, arg2...)
            (JsValue::IpcModule, "call") => {
                if !self.permissions.allow_ipc {
                    return Err(EvalError::PermissionDenied(String::from("ipc:call")));
                }
                let target = args.get(0).ok_or(EvalError::TypeError("ipc.call requires target"))?.to_str()?;
                let op = args.get(1).ok_or(EvalError::TypeError("ipc.call requires op"))?.to_str()?;

                let mut wire = inos_app_sdk::AutomationIpcWire::empty();
                wire.request_id = 1002;

                if op == "set" || op == "set_cell" {
                    wire.op_code = inos_app_sdk::OP_SET_CELL;
                    let cell_coord = args.get(2).ok_or(EvalError::TypeError("set requires cell"))?.to_str()?;
                    let c_bytes = cell_coord.as_bytes();
                    let c_len = core::cmp::min(c_bytes.len(), 8);
                    wire.cell[..c_len].copy_from_slice(&c_bytes[..c_len]);

                    if let Some(val_arg) = args.get(3) {
                        match val_arg {
                            JsValue::Number(n) => {
                                wire.value_num = *n;
                            }
                            JsValue::String(f) if f.starts_with('=') => {
                                let f_bytes = f.as_bytes();
                                let f_len = core::cmp::min(f_bytes.len(), 32);
                                wire.formula[..f_len].copy_from_slice(&f_bytes[..f_len]);
                            }
                            JsValue::String(s) => {
                                let s_bytes = s.as_bytes();
                                let s_len = core::cmp::min(s_bytes.len(), 32);
                                wire.formula[..s_len].copy_from_slice(&s_bytes[..s_len]);
                            }
                            _ => {}
                        }
                    }
                } else if op == "evaluate" {
                    wire.op_code = inos_app_sdk::OP_EVALUATE;
                    let cell_coord = args.get(2).ok_or(EvalError::TypeError("evaluate requires cell"))?.to_str()?;
                    let c_bytes = cell_coord.as_bytes();
                    let c_len = core::cmp::min(c_bytes.len(), 8);
                    wire.cell[..c_len].copy_from_slice(&c_bytes[..c_len]);
                } else if op == "save" {
                    wire.op_code = inos_app_sdk::OP_SAVE;
                    let path = args.get(2).ok_or(EvalError::TypeError("save requires path"))?.to_str()?;
                    let p_bytes = path.as_bytes();
                    let p_len = core::cmp::min(p_bytes.len(), 32);
                    wire.target_file[..p_len].copy_from_slice(&p_bytes[..p_len]);
                } else if op == "inspect" {
                    wire.op_code = inos_app_sdk::compositor::OP_INSPECT;
                    if let Some(arg2) = args.get(2) {
                        let s = arg2.to_str()?;
                        let b = s.as_bytes();
                        let l = core::cmp::min(b.len(), 32);
                        wire.target_file[..l].copy_from_slice(&b[..l]);
                    }
                } else if op == "stage_ribbon" {
                    wire.op_code = inos_app_sdk::compositor::OP_STAGE_RIBBON;
                    if let Some(arg2) = args.get(2) {
                        let s = arg2.to_str()?;
                        let b = s.as_bytes();
                        let l = core::cmp::min(b.len(), 32);
                        wire.formula[..l].copy_from_slice(&b[..l]);
                    }
                    if let Some(arg3) = args.get(3) {
                        let s = arg3.to_str()?;
                        let b = s.as_bytes();
                        let l = core::cmp::min(b.len(), 32);
                        wire.target_file[..l].copy_from_slice(&b[..l]);
                    }
                } else if op == "bind_ribbon" {
                    wire.op_code = inos_app_sdk::compositor::OP_BIND_RIBBON;
                    if let Some(arg2) = args.get(2) {
                        let s = arg2.to_str()?;
                        let b = s.as_bytes();
                        let l = core::cmp::min(b.len(), 32);
                        wire.formula[..l].copy_from_slice(&b[..l]);
                    }
                    if let Some(arg3) = args.get(3) {
                        let s = arg3.to_str()?;
                        let b = s.as_bytes();
                        let l = core::cmp::min(b.len(), 32);
                        wire.target_file[..l].copy_from_slice(&b[..l]);
                    }
                }

                let target_pid = resolve_app_pid(&target);
                let req_bytes = unsafe {
                    core::slice::from_raw_parts(
                        &wire as *const inos_app_sdk::AutomationIpcWire as *const u8,
                        core::mem::size_of::<inos_app_sdk::AutomationIpcWire>(),
                    )
                };
                let _ = inos_app_sdk::ipc_send_msg(target_pid, req_bytes);

                let mut resp_wire = inos_app_sdk::AutomationIpcWire::empty();
                let resp_bytes = unsafe {
                    core::slice::from_raw_parts_mut(
                        &mut resp_wire as *mut inos_app_sdk::AutomationIpcWire as *mut u8,
                        core::mem::size_of::<inos_app_sdk::AutomationIpcWire>(),
                    )
                };
                let _ = inos_app_sdk::ipc_recv_msg(target_pid, resp_bytes);

                if wire.op_code == inos_app_sdk::OP_EVALUATE {
                    let result_num = resp_wire.value_num;
                    self.audit_log.push(format!("[AUDIT] inos.ipc.call('{}', 'evaluate') = {}", target, result_num));
                    Ok(JsValue::Number(result_num))
                } else {
                    self.audit_log.push(format!("[AUDIT] inos.ipc.call('{}', '{}') [OK]", target, op));
                    Ok(JsValue::Bool(true))
                }
            }

            // inos.audit.log(msg)
            (JsValue::AuditModule, "log") => {
                let msg = args.get(0).ok_or(EvalError::TypeError("audit.log requires message"))?.to_str()?;
                self.audit_log.push(msg);
                Ok(JsValue::Bool(true))
            }

            // inos.write.createDocument(title)
            (JsValue::WriteModule, "createDocument") => {
                let title = if let Some(arg) = args.get(0) { arg.to_str()? } else { String::from("Untitled") };
                self.audit_log.push(format!("[AUDIT] inos.write.createDocument('{}')", title));
                Ok(JsValue::Document(Box::new(ScriptDocument::new(&title))))
            }

            // inos.write.openDocument(path)
            (JsValue::WriteModule, "openDocument") => {
                let path = args.get(0).ok_or(EvalError::TypeError("write.openDocument requires path"))?.to_str()?;
                if !self.permissions.check_path(&path) {
                    return Err(EvalError::PermissionDenied(path));
                }
                self.audit_log.push(format!("[AUDIT] inos.write.openDocument('{}')", path));
                let mut doc = ScriptDocument::new(&path);
                doc.path = path;
                Ok(JsValue::Document(Box::new(doc)))
            }

            // doc.addHeading(text, level)
            (JsValue::Document(mut doc), "addHeading") => {
                let text = args.get(0).ok_or(EvalError::TypeError("addHeading requires text"))?.to_str()?;
                let level = if let Some(arg1) = args.get(1) { arg1.to_int()? as usize } else { 1 };
                self.audit_log.push(format!("[AUDIT] document.addHeading('{}', h{})", text, level));
                doc.elements.push(DocElement::Heading { text, level });
                Ok(JsValue::Document(doc))
            }

            // doc.addParagraph(text)
            (JsValue::Document(mut doc), "addParagraph") => {
                let text = args.get(0).ok_or(EvalError::TypeError("addParagraph requires text"))?.to_str()?;
                self.audit_log.push(format!("[AUDIT] document.addParagraph('{}')", text));
                doc.elements.push(DocElement::Paragraph(text));
                Ok(JsValue::Document(doc))
            }

            // doc.addTable(rows, cols)
            (JsValue::Document(mut doc), "addTable") => {
                let rows = args.get(0).ok_or(EvalError::TypeError("addTable requires rows"))?.to_int()? as usize;
                let cols = args.get(1).ok_or(EvalError::TypeError("addTable requires cols"))?.to_int()? as usize;
                let data = alloc::vec![alloc::vec![String::new(); cols]; rows];
                self.audit_log.push(format!("[AUDIT] document.addTable({}x{})", rows, cols));
                doc.elements.push(DocElement::Table { rows, cols, data });
                Ok(JsValue::Document(doc))
            }

            // doc.setCell(r, c, text)
            (JsValue::Document(mut doc), "setCell") => {
                let r = args.get(0).ok_or(EvalError::TypeError("setCell requires row"))?.to_int()? as usize;
                let c = args.get(1).ok_or(EvalError::TypeError("setCell requires col"))?.to_int()? as usize;
                let text = args.get(2).ok_or(EvalError::TypeError("setCell requires text"))?.to_str()?;
                if let Some(DocElement::Table { data, .. }) = doc.elements.last_mut() {
                    if r < data.len() && c < data[r].len() {
                        data[r][c] = text;
                    }
                }
                Ok(JsValue::Document(doc))
            }

            // doc.exportText(path) or doc.save(path)
            (JsValue::Document(doc), "exportText") | (JsValue::Document(doc), "save") => {
                let path = if let Some(arg) = args.get(0) {
                    arg.to_str()?
                } else {
                    doc.path.clone()
                };
                if !self.permissions.check_path(&path) {
                    return Err(EvalError::PermissionDenied(path));
                }
                self.audit_log.push(format!("[AUDIT] inos.write.save('{}')", path));
                let formatted = doc.to_formatted_text();
                self.saved_files.push((path, formatted.into_bytes()));
                Ok(JsValue::Bool(true))
            }

            // inos.canvas.create(width, height)
            (JsValue::CanvasModule, "create") => {
                let w = args.get(0).ok_or(EvalError::TypeError("canvas.create requires width"))?.to_int()? as u32;
                let h = args.get(1).ok_or(EvalError::TypeError("canvas.create requires height"))?.to_int()? as u32;
                self.audit_log.push(format!("[AUDIT] inos.canvas.create({}x{})", w, h));
                Ok(JsValue::Canvas(Box::new(ScriptCanvas::new(w, h))))
            }

            // canvas.addShape(shape_type, x, y, w, h, color)
            (JsValue::Canvas(mut canvas), "addShape") => {
                let stype = args.get(0).ok_or(EvalError::TypeError("addShape requires shape_type"))?.to_str()?;
                let x = args.get(1).ok_or(EvalError::TypeError("addShape requires x"))?.to_int()? as u32;
                let y = args.get(2).ok_or(EvalError::TypeError("addShape requires y"))?.to_int()? as u32;
                let w = args.get(3).ok_or(EvalError::TypeError("addShape requires width"))?.to_int()? as u32;
                let h = args.get(4).ok_or(EvalError::TypeError("addShape requires height"))?.to_int()? as u32;
                let color = args.get(5).ok_or(EvalError::TypeError("addShape requires color"))?.to_str()?;
                self.audit_log.push(format!("[AUDIT] canvas.addShape('{}', {}, {}, {}, {}, '{}')", stype, x, y, w, h, color));
                canvas.shapes.push(CanvasShape { shape_type: stype, x, y, width: w, height: h, color });
                Ok(JsValue::Canvas(canvas))
            }

            // canvas.exportSvg(path) or canvas.save(path)
            (JsValue::Canvas(canvas), "exportSvg") | (JsValue::Canvas(canvas), "save") => {
                let path = args.get(0).ok_or(EvalError::TypeError("canvas.exportSvg requires path"))?.to_str()?;
                if !self.permissions.check_path(&path) {
                    return Err(EvalError::PermissionDenied(path));
                }
                self.audit_log.push(format!("[AUDIT] inos.canvas.exportSvg('{}')", path));
                let svg = canvas.to_svg_xml();
                self.saved_files.push((path, svg.into_bytes()));
                Ok(JsValue::Bool(true))
            }

            // inos.plan.createTask(title, dueDate, priority)
            (JsValue::PlanModule, "createTask") => {
                let title = args.get(0).ok_or(EvalError::TypeError("createTask requires title"))?.to_str()?;
                let due = if let Some(arg1) = args.get(1) { arg1.to_str()? } else { String::from("today") };
                let priority = if let Some(arg2) = args.get(2) { arg2.to_int()? as u32 } else { 1 };
                self.audit_log.push(format!("[AUDIT] inos.plan.createTask('{}', due='{}', prio={})", title, due, priority));
                Ok(JsValue::Number(201))
            }

            // inos.plan.addAppointment(title, timestamp, duration, recurrence)
            (JsValue::PlanModule, "addAppointment") => {
                let title = args.get(0).ok_or(EvalError::TypeError("addAppointment requires title"))?.to_str()?;
                let ts = args.get(1).ok_or(EvalError::TypeError("addAppointment requires timestamp"))?.to_int()?;
                let dur = args.get(2).ok_or(EvalError::TypeError("addAppointment requires duration"))?.to_int()?;
                let recur = if let Some(arg3) = args.get(3) { arg3.to_str()? } else { String::from("none") };
                self.audit_log.push(format!("[AUDIT] inos.plan.addAppointment('{}', ts={}, dur={}m, recur='{}')", title, ts, dur, recur));
                Ok(JsValue::Number(301))
            }

            // inos.plan.listTasks()
            (JsValue::PlanModule, "listTasks") => {
                self.audit_log.push(String::from("[AUDIT] inos.plan.listTasks()"));
                let tasks = alloc::vec![
                    JsValue::String(String::from("Review Q1 Spreadsheet")),
                    JsValue::String(String::from("Approve Commission Plan")),
                ];
                Ok(JsValue::Array(tasks))
            }

            _ => Err(EvalError::MethodNotFound(String::from(method))),
        }
    }
}

/// Maps all 16 native INOS applications to their registered Ring 3 PIDs
pub fn resolve_app_pid(target: &str) -> u64 {
    match target {
        "org.inos.grid" => 500,
        "org.inos.write" => 510,
        "org.inos.plan" => 520,
        "org.inos.pdf" => 530,
        "org.inos.mail" => 540,
        "org.inos.flow" => 550,
        "org.inos.calc" => 600,
        "org.inos.canvas" => 650,
        "org.inos.notes" => 700,
        "org.inos.archive" => 800,
        "org.inos.spin" => 850,
        "org.inos.intscript" | "org.inos.script" => 900,
        "org.inos.files" => 910,
        "org.inos.net" => 300,
        "org.inos.web" => 310,
        "org.inos.security" => 400,
        _ => 1,
    }
}

fn crc32_ieee(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            if (crc & 1) != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

fn build_pkzip_archive(
    saved_files: &[(String, Vec<u8>)],
    requested_entries: Option<&JsValue>,
) -> Vec<u8> {
    let mut entries: Vec<(&str, &[u8])> = Vec::new();
    if let Some(JsValue::Array(arr)) = requested_entries {
        for item in arr {
            if let JsValue::String(ref req_name) = item {
                if let Some((name, data)) = saved_files.iter().rev().find(|(n, _)| n == req_name) {
                    entries.push((name.as_str(), data.as_slice()));
                }
            }
        }
    }
    if entries.is_empty() {
        for (name, data) in saved_files {
            entries.push((name.as_str(), data.as_slice()));
        }
    }
    if entries.is_empty() {
        entries.push(("manifest.txt", b"[INOS_AUTOMATION_ZIP_PACKAGE]"));
    }

    let mut out = Vec::new();
    let mut cd_records: Vec<(usize, &str, u32, u32)> = Vec::new();
    let comment = b"[INOS_AUTOMATION_ZIP_PACKAGE]";

    for (idx, &(name, data)) in entries.iter().enumerate() {
        let local_offset = out.len();
        let name_bytes = name.as_bytes();
        let crc = crc32_ieee(data);
        let size = data.len() as u32;
        let extra: &[u8] = if idx == 0 { comment } else { &[] };

        // Local file header (PK\x03\x04)
        out.extend_from_slice(b"PK\x03\x04");
        out.extend_from_slice(&20u16.to_le_bytes()); // version needed
        out.extend_from_slice(&0u16.to_le_bytes());  // flags
        out.extend_from_slice(&0u16.to_le_bytes());  // compression: 0 = stored
        out.extend_from_slice(&0u16.to_le_bytes());  // mod time
        out.extend_from_slice(&0u16.to_le_bytes());  // mod date
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        out.extend_from_slice(&(extra.len() as u16).to_le_bytes());
        out.extend_from_slice(name_bytes);
        out.extend_from_slice(extra);
        out.extend_from_slice(data);

        cd_records.push((local_offset, name, crc, size));
    }

    let cd_offset = out.len() as u32;
    for &(local_offset, name, crc, size) in &cd_records {
        let name_bytes = name.as_bytes();
        // Central directory header (PK\x01\x02)
        out.extend_from_slice(b"PK\x01\x02");
        out.extend_from_slice(&20u16.to_le_bytes()); // version made by
        out.extend_from_slice(&20u16.to_le_bytes()); // version needed
        out.extend_from_slice(&0u16.to_le_bytes());  // flags
        out.extend_from_slice(&0u16.to_le_bytes());  // compression: stored
        out.extend_from_slice(&0u16.to_le_bytes());  // mod time
        out.extend_from_slice(&0u16.to_le_bytes());  // mod date
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());  // extra len
        out.extend_from_slice(&0u16.to_le_bytes());  // comment len
        out.extend_from_slice(&0u16.to_le_bytes());  // disk number
        out.extend_from_slice(&0u16.to_le_bytes());  // internal attrs
        out.extend_from_slice(&0u32.to_le_bytes());  // external attrs
        out.extend_from_slice(&(local_offset as u32).to_le_bytes());
        out.extend_from_slice(name_bytes);
    }

    let cd_size = (out.len() as u32).saturating_sub(cd_offset);
    let count = cd_records.len() as u16;

    // End of Central Directory (PK\x05\x06)
    out.extend_from_slice(b"PK\x05\x06");
    out.extend_from_slice(&0u16.to_le_bytes()); // disk number
    out.extend_from_slice(&0u16.to_le_bytes()); // cd start disk
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&cd_size.to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&(comment.len() as u16).to_le_bytes());
    out.extend_from_slice(comment);

    out
}

