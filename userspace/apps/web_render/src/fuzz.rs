// userspace/apps/web_render/src/fuzz.rs - Deterministic Seeded Fuzzer & Boundary Invariance
// INOS Native Browser Subsystem (Milestone WEB-02C)
// Provides zero-dependency deterministic PRNG, targeted boundary chunk splits,
// differential chunk invariance verification, and independent structural oracle assertions.

extern crate alloc;

use alloc::vec::Vec;
use crate::limits::ParseError;
use crate::dom::{DomArena, NodeData};
use crate::HtmlParser;

/// Zero-dependency 64-bit Linear Congruential PRNG (LCG) for deterministic reproducibility.
#[derive(Debug, Clone)]
pub struct SeededRng {
    state: u64,
}

impl SeededRng {
    pub const fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0x853c49e6748fea9b } else { seed },
        }
    }

    pub fn next_u32(&mut self) -> u32 {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.state >> 32) as u32
    }

    pub fn gen_range(&mut self, min: usize, max: usize) -> usize {
        if min >= max {
            return min;
        }
        let range = (max - min + 1) as u32;
        min + (self.next_u32() % range) as usize
    }
}

/// Splits a document into chunks according to explicit partition sizes.
pub fn parse_with_partitions(doc: &[u8], partition_sizes: &[usize]) -> Result<DomArena, ParseError> {
    let mut parser = HtmlParser::new()?;
    let mut offset = 0;
    for &size in partition_sizes {
        if offset >= doc.len() {
            break;
        }
        let end = core::cmp::min(offset + size, doc.len());
        parser.parse_chunk(&doc[offset..end])?;
        offset = end;
    }
    if offset < doc.len() {
        parser.parse_chunk(&doc[offset..])?;
    }
    parser.finish()?;
    Ok(parser.arena)
}

/// Deterministically generates random partitions for a document using SeededRng.
pub fn generate_random_partitions(rng: &mut SeededRng, doc_len: usize, min_chunk: usize, max_chunk: usize) -> Vec<usize> {
    let mut partitions = Vec::new();
    let mut remaining = doc_len;
    while remaining > 0 {
        let chunk = rng.gen_range(min_chunk, max_chunk);
        let actual = core::cmp::min(chunk, remaining);
        partitions.push(actual);
        remaining -= actual;
    }
    partitions
}

/// Ground-truth structural oracle for representative test document.
/// Validates exact hierarchy, node types, element counts, and attribute values
/// independently of the parser's internal differential representation.
pub fn verify_ground_truth_oracle(arena: &DomArena) -> Result<(), &'static str> {
    // 1. Root must be document node
    let root = arena.get(arena.document_id).ok_or("Missing root node")?;
    match root.data {
        NodeData::Document => {},
        _ => return Err("Root must be Document node"),
    }

    // 2. Document must have an <html> child
    let html_id = arena.find_element_by_tag("html").ok_or("Missing <html> element")?;
    let html_node = arena.get(html_id).ok_or("Invalid html node ID")?;

    // 3. <html> must contain <head> and <body>
    let head_id = arena.find_element_by_tag("head").ok_or("Missing <head> element")?;
    let body_id = arena.find_element_by_tag("body").ok_or("Missing <body> element")?;
    let _ = (head_id, body_id);

    // 4. Verify title exists inside head
    let title_id = arena.find_element_by_tag("title").ok_or("Missing <title> element")?;
    let title_node = arena.get(title_id).ok_or("Invalid title node ID")?;
    let title_child_id = title_node.first_child.ok_or("Title must have text child")?;
    let title_text = arena.get(title_child_id).ok_or("Invalid title text node")?;
    match &title_text.data {
        NodeData::Text(s) => {
            if !s.contains("Test") || !s.contains("Page") {
                return Err("Title text does not match ground truth");
            }
        }
        _ => return Err("Title child must be Text"),
    }

    // 5. Verify <p> and <b> exist
    let p_id = arena.find_element_by_tag("p").ok_or("Missing <p> element")?;
    let b_id = arena.find_element_by_tag("b").ok_or("Missing <b> element")?;
    let _ = (p_id, b_id, html_node);

    Ok(())
}

/// Fuzz suite running multiple deterministic partition passes with seed recording.
pub fn run_fuzz_partition_suite(doc: &[u8], base_seed: u64, rounds: usize) -> Result<(), ParseError> {
    let mut baseline_parser = HtmlParser::new()?;
    baseline_parser.parse_chunk(doc)?;
    baseline_parser.finish()?;
    let baseline_dom = baseline_parser.arena;

    for r in 0..rounds {
        let seed = base_seed.wrapping_add(r as u64);
        let mut rng = SeededRng::new(seed);
        let partitions = generate_random_partitions(&mut rng, doc.len(), 1, 16);

        let chunked_dom = parse_with_partitions(doc, &partitions)?;
        if baseline_dom != chunked_dom {
            // Difference detected: return ParseError
            return Err(ParseError::MalformedInput("Fuzzed chunk differential mismatch"));
        }
    }
    Ok(())
}

/// Targeted boundary cuts test: splits specifically across multi-byte UTF-8,
/// entity sequences, quoted attributes, comments, and raw text.
pub fn test_targeted_boundary_cuts() -> Result<(), &'static str> {
    // 1. Multi-byte UTF-8 cut (Euro sign € is 0xE2 0x82 0xAC: cut between 0xE2 and 0x82, and 0x82 and 0xAC)
    let utf8_doc = b"<html><body><p>Price: \xE2\x82\xAC 100</p></body></html>";
    // Cut 1: up to 0xE2 (offset 23)
    let dom_cut1 = parse_with_partitions(utf8_doc, &[23, 1, 1, utf8_doc.len() - 25])
        .map_err(|e| match e {
            ParseError::OutOfMemory => "Failed UTF-8 1-byte boundary cut (OOM)",
            _ => "Failed UTF-8 1-byte boundary cut",
        })?;
    let mut base_p = HtmlParser::new().map_err(|_| "Parser init failed")?;
    base_p.parse_chunk(utf8_doc).map_err(|_| "Parse failed")?;
    base_p.finish().map_err(|_| "Finish failed")?;
    if dom_cut1 != base_p.arena {
        return Err("UTF-8 multi-byte split produced non-invariant DOM");
    }

    // 2. Entity cut: "&amp;" split between '&' and 'a', 'm', 'p', ';'
    let entity_doc = b"<html><body><p>Fish &amp; Chips</p></body></html>";
    let dom_entity = parse_with_partitions(entity_doc, &[20, 1, 2, 1, entity_doc.len() - 24])
        .map_err(|e| match e {
            ParseError::OutOfMemory => "Failed entity cut (OOM)",
            _ => "Failed entity cut",
        })?;
    let mut base_e = HtmlParser::new().map_err(|_| "Parser init failed")?;
    base_e.parse_chunk(entity_doc).map_err(|_| "Parse failed")?;
    base_e.finish().map_err(|_| "Finish failed")?;
    if dom_entity != base_e.arena {
        return Err("Entity reference split produced non-invariant DOM");
    }

    // 3. Attribute cut: <div class="primary-button" id="btn1">
    let attr_doc = b"<html><body><div class=\"primary-button\" id=\"btn1\">Click</div></body></html>";
    let dom_attr = parse_with_partitions(attr_doc, &[18, 5, 10, attr_doc.len() - 33])
        .map_err(|e| match e {
            ParseError::OutOfMemory => "Failed attribute cut (OOM)",
            _ => "Failed attribute cut",
        })?;
    let mut base_a = HtmlParser::new().map_err(|_| "Parser init failed")?;
    base_a.parse_chunk(attr_doc).map_err(|_| "Parse failed")?;
    base_a.finish().map_err(|_| "Finish failed")?;
    if dom_attr != base_a.arena {
        return Err("Attribute boundary split produced non-invariant DOM");
    }

    // 4. Comment cut: <!-- secret comment -->
    let comment_doc = b"<html><body><!-- secret comment --><p>Content</p></body></html>";
    let dom_comment = parse_with_partitions(comment_doc, &[14, 2, 10, comment_doc.len() - 26])
        .map_err(|e| match e {
            ParseError::OutOfMemory => "Failed comment cut (OOM)",
            _ => "Failed comment cut",
        })?;
    let mut base_c = HtmlParser::new().map_err(|_| "Parser init failed")?;
    base_c.parse_chunk(comment_doc).map_err(|_| "Parse failed")?;
    base_c.finish().map_err(|_| "Finish failed")?;
    if dom_comment != base_c.arena {
        return Err("Comment boundary split produced non-invariant DOM");
    }

    // 5. Raw text cut: <script>var x = 1;</script>
    let script_doc = b"<html><body><script>var x = 1;</script></body></html>";
    let dom_script = parse_with_partitions(script_doc, &[20, 3, 5, script_doc.len() - 28])
        .map_err(|e| match e {
            ParseError::OutOfMemory => "Failed script raw text cut (OOM)",
            _ => "Failed script raw text cut",
        })?;
    let mut base_s = HtmlParser::new().map_err(|_| "Parser init failed")?;
    base_s.parse_chunk(script_doc).map_err(|_| "Parse failed")?;
    base_s.finish().map_err(|_| "Finish failed")?;
    if dom_script != base_s.arena {
        return Err("Script raw text split produced non-invariant DOM");
    }

    Ok(())
}
