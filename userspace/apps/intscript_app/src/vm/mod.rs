// userspace/apps/script_app/src/vm/mod.rs - JavaScript-Compatible Automation VM
pub mod token;
pub mod lexer;
pub mod ast;
pub mod parser;
pub mod evaluator;

use core::sync::atomic::{AtomicUsize, Ordering};
pub use evaluator::{
    resolve_app_pid, Evaluator, EvalError, JsValue, ScriptQuotas, ScriptPermissionScope,
};

const AST_CACHE_SLOTS: usize = 8;
static mut AST_CACHE: [(u64, usize, Option<alloc::vec::Vec<ast::Statement>>); AST_CACHE_SLOTS] = [
    (0, 0, None),
    (0, 0, None),
    (0, 0, None),
    (0, 0, None),
    (0, 0, None),
    (0, 0, None),
    (0, 0, None),
    (0, 0, None),
];
static mut AST_CACHE_CURSOR: usize = 0;
static AST_CACHE_HITS: AtomicUsize = AtomicUsize::new(0);

/// Computes deterministic 64-bit FNV-1a hash of script source
pub fn fnv1a_hash(s: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in s.as_bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Returns total number of pre-parsed AST cache hits
pub fn ast_cache_hits() -> usize {
    AST_CACHE_HITS.load(Ordering::Relaxed)
}

fn lookup_cached_ast(hash: u64, len: usize) -> Option<alloc::vec::Vec<ast::Statement>> {
    unsafe {
        for i in 0..AST_CACHE_SLOTS {
            if AST_CACHE[i].0 == hash && AST_CACHE[i].1 == len {
                if let Some(ref stmts) = AST_CACHE[i].2 {
                    AST_CACHE_HITS.fetch_add(1, Ordering::Relaxed);
                    return Some(stmts.clone());
                }
            }
        }
    }
    None
}

fn insert_cached_ast(hash: u64, len: usize, stmts: alloc::vec::Vec<ast::Statement>) {
    unsafe {
        for i in 0..AST_CACHE_SLOTS {
            if AST_CACHE[i].0 == hash && AST_CACHE[i].1 == len && AST_CACHE[i].2.is_some() {
                return;
            }
        }
        let idx = AST_CACHE_CURSOR % AST_CACHE_SLOTS;
        AST_CACHE[idx] = (hash, len, Some(stmts));
        AST_CACHE_CURSOR = AST_CACHE_CURSOR.wrapping_add(1);
    }
}

pub fn execute_script(
    source: &str,
    quotas: ScriptQuotas,
    permissions: ScriptPermissionScope,
) -> Result<Evaluator, EvalError> {
    let ctx = permissions.to_isolation_context();
    let _ = inos_app_sdk::set_script_context(&ctx);

    let key_hash = fnv1a_hash(source);
    let key_len = source.len();

    let program = if let Some(cached) = lookup_cached_ast(key_hash, key_len) {
        cached
    } else {
        let mut lexer = lexer::Lexer::new(source);
        let tokens = match lexer.tokenize() {
            Ok(t) => t,
            Err(e) => {
                let _ = inos_app_sdk::clear_script_context();
                return Err(EvalError::Custom(alloc::string::String::from(e)));
            }
        };
        let mut parser = parser::Parser::new(tokens);
        let parsed = match parser.parse_program() {
            Ok(p) => p,
            Err(e) => {
                let _ = inos_app_sdk::clear_script_context();
                return Err(EvalError::Custom(alloc::string::String::from(e)));
            }
        };
        insert_cached_ast(key_hash, key_len, parsed.clone());
        parsed
    };

    let mut evaluator = Evaluator::new(quotas, permissions);
    let res = evaluator.eval_program(&program);
    let _ = inos_app_sdk::clear_script_context();
    res.map(|_| evaluator)
}

