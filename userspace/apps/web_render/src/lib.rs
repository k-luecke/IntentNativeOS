// userspace/apps/web_render/src/lib.rs
// INOS Native Browser Subsystem — HTML Parser Library (Milestone WEB-02B)

#![no_std]

extern crate alloc;

pub mod limits;
pub mod dom;
pub mod decoder;
pub mod tokenizer;
pub mod tree_builder;

use alloc::vec::Vec;
use crate::limits::{
    MAX_DOM_NODES, MAX_INPUT_BYTES, MAX_WORK_OPERATIONS, ParseError, WorkBudgetTracker,
};
use crate::dom::DomArena;
use crate::decoder::Utf8StreamDecoder;
use crate::tokenizer::HtmlTokenizer;
use crate::tree_builder::HtmlTreeBuilder;

pub struct HtmlParser {
    pub decoder: Utf8StreamDecoder,
    pub tokenizer: HtmlTokenizer,
    pub builder: HtmlTreeBuilder,
    pub arena: DomArena,
    pub budget: WorkBudgetTracker,
}

impl HtmlParser {
    pub fn new() -> Result<Self, ParseError> {
        Ok(Self {
            decoder: Utf8StreamDecoder::new(MAX_INPUT_BYTES),
            tokenizer: HtmlTokenizer::new(),
            builder: HtmlTreeBuilder::new(),
            arena: DomArena::new(MAX_DOM_NODES)?,
            budget: WorkBudgetTracker::new(MAX_WORK_OPERATIONS),
        })
    }

    pub fn parse_chunk(&mut self, chunk: &[u8]) -> Result<(), ParseError> {
        let mut chars = Vec::new();
        self.decoder.feed_chunk(chunk, &mut chars, &mut self.budget)?;

        let mut tokens = Vec::new();
        self.tokenizer.feed_chars(&chars, &mut tokens, &mut self.budget)?;

        for token in tokens {
            self.builder.process_token(token, &mut self.arena, &mut self.budget)?;
        }
        Ok(())
    }

    pub fn finish(&mut self) -> Result<(), ParseError> {
        let mut chars = Vec::new();
        self.decoder.finish(&mut chars, &mut self.budget)?;

        let mut tokens = Vec::new();
        if !chars.is_empty() {
            self.tokenizer.feed_chars(&chars, &mut tokens, &mut self.budget)?;
        }
        self.tokenizer.finish(&mut tokens, &mut self.budget)?;

        for token in tokens {
            self.builder.process_token(token, &mut self.arena, &mut self.budget)?;
        }
        Ok(())
    }
}

pub fn parse_document(html: &[u8]) -> Result<DomArena, ParseError> {
    let mut parser = HtmlParser::new()?;
    parser.parse_chunk(html)?;
    parser.finish()?;
    Ok(parser.arena)
}

pub fn parse_document_chunked(html: &[u8], chunk_size: usize) -> Result<DomArena, ParseError> {
    let mut parser = HtmlParser::new()?;
    for chunk in html.chunks(chunk_size) {
        parser.parse_chunk(chunk)?;
    }
    parser.finish()?;
    Ok(parser.arena)
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use crate::limits::LimitKind;
    use crate::dom::NodeData;

    #[test]
    fn test_chunk_invariance() {
        let doc = b"<!DOCTYPE html><html><head><title>Test &amp; Page</title></head><body><p>Hello, <b>world</b> &copy; 2026!</p></body></html>";

        let arena_full = parse_document(doc).expect("Full document parse should succeed");
        let arena_1byte = parse_document_chunked(doc, 1).expect("1-byte chunk parse should succeed");
        let arena_3byte = parse_document_chunked(doc, 3).expect("3-byte chunk parse should succeed");
        let arena_7byte = parse_document_chunked(doc, 7).expect("7-byte chunk parse should succeed");

        assert_eq!(arena_full, arena_1byte, "1-byte chunks must produce identical DOM to full buffer");
        assert_eq!(arena_full, arena_3byte, "3-byte chunks must produce identical DOM to full buffer");
        assert_eq!(arena_full, arena_7byte, "7-byte chunks must produce identical DOM to full buffer");
    }

    #[test]
    fn test_tree_depth_limit() {
        // Construct deeply nested <div> elements (e.g. 70 levels, exceeding MAX_TREE_DEPTH = 64)
        let mut deep_html = std::vec::Vec::new();
        for _ in 0..70 {
            deep_html.extend_from_slice(b"<div>");
        }
        for _ in 0..70 {
            deep_html.extend_from_slice(b"</div>");
        }

        let res = parse_document(&deep_html);
        assert!(matches!(res, Err(ParseError::LimitExceeded(LimitKind::TreeDepth))),
            "Deep nesting must be rejected with TreeDepth limit error");
    }

    #[test]
    fn test_attr_value_length_limit() {
        let mut huge_attr = std::vec::Vec::new();
        huge_html_attr(&mut huge_attr, 2048);

        let res = parse_document(&huge_attr);
        assert!(matches!(res, Err(ParseError::LimitExceeded(LimitKind::AttrValueLength))),
            "Oversized attribute values must be rejected with AttrValueLength error");
    }

    fn huge_html_attr(buf: &mut std::vec::Vec<u8>, val_len: usize) {
        buf.extend_from_slice(b"<div class=\"");
        for _ in 0..val_len {
            buf.push(b'x');
        }
        buf.extend_from_slice(b"\"></div>");
    }

    #[test]
    fn test_inert_script_and_event_handlers() {
        let html = b"<html><head><script>alert('xss'); while(true){}</script></head><body><div onclick=\"runBad()\">Click</div></body></html>";
        let arena = parse_document(html).expect("Parse should succeed");

        // Verify script is present as an element with inert text
        let script_id = arena.find_element_by_tag("script").expect("Script element should exist");
        let script_node = arena.get(script_id).unwrap();
        let child_id = script_node.first_child.expect("Script should have text child");
        let child_node = arena.get(child_id).unwrap();
        match &child_node.data {
            NodeData::Text(s) => assert_eq!(s, "alert('xss'); while(true){}"),
            _ => panic!("Script child must be inert Text node"),
        }

        // Verify onclick is stored passively in attributes
        let div_id = arena.find_element_by_tag("div").expect("Div element should exist");
        let div_node = arena.get(div_id).unwrap();
        if let NodeData::Element(ref el) = div_node.data {
            assert_eq!(el.get_attribute("onclick"), Some("runBad()"));
        } else {
            panic!("Expected element data");
        }
    }

    #[test]
    fn test_nested_5000_divs() {
        // Stress test: 5,000 nested tags must fail safely with TreeDepth limit without stack overflow
        let mut deep_html = std::vec::Vec::new();
        for _ in 0..5000 {
            deep_html.extend_from_slice(b"<div>");
        }
        let res = parse_document(&deep_html);
        assert!(matches!(res, Err(ParseError::LimitExceeded(LimitKind::TreeDepth))),
            "5,000 nested tags must fail deterministically with TreeDepth limit");
    }

    #[test]
    fn test_oversized_2mb_attribute() {
        // Stress test: 2 MB payload exceeds MAX_INPUT_BYTES (512 KiB) and fails immediately at decoder layer
        let mut huge_attr = std::vec::Vec::new();
        huge_html_attr(&mut huge_attr, 2 * 1024 * 1024);
        let res = parse_document(&huge_attr);
        assert!(matches!(res, Err(ParseError::LimitExceeded(LimitKind::InputBytes))),
            "2 MB document must be rejected at decoder boundary without heap exhaustion");
    }

    #[test]
    fn test_malformed_stream_and_entity_recovery() {
        // Malformed entities, dangling tags, and incomplete UTF-8 at EOF
        let mut malformed = std::vec::Vec::new();
        malformed.extend_from_slice(b"<p>Broken &nonexistent; entity and unclosed <span>tag");
        // Trailing incomplete 3-byte UTF-8 sequence (first 2 bytes of euro sign 0xE2 0x82 0xAC)
        malformed.push(0xE2);
        malformed.push(0x82);

        let arena = parse_document(&malformed).expect("Malformed input should be recovered gracefully");
        // Verify text node contains literal broken entity and replacement character U+FFFD
        let p_id = arena.find_element_by_tag("p").expect("p element should exist");
        let p_node = arena.get(p_id).unwrap();
        let child_id = p_node.first_child.expect("p should have child");
        let child_node = arena.get(child_id).unwrap();
        match &child_node.data {
            NodeData::Text(s) => {
                assert!(s.contains("&nonexistent;"), "Broken entity should be preserved literally");
            }
            _ => panic!("Expected text node"),
        }

        let span_id = arena.find_element_by_tag("span").expect("span element should exist");
        let span_node = arena.get(span_id).unwrap();
        let span_child_id = span_node.first_child.expect("span should have child");
        let span_child_node = arena.get(span_child_id).unwrap();
        match &span_child_node.data {
            NodeData::Text(s) => {
                assert!(s.contains('\u{FFFD}'), "Dangling incomplete UTF-8 must be flushed as replacement char");
            }
            _ => panic!("Expected text node"),
        }
    }

    #[test]
    fn test_list_and_table_closing() {
        let html = b"<ul><li>Item 1<li>Item 2<li>Item 3</ul><table><tr><td>Cell 1<td>Cell 2<tr><td>Cell 3</table>";
        let arena = parse_document(html).expect("Parse should succeed");

        assert_eq!(arena.count_elements_by_tag("li"), 3);
        assert_eq!(arena.count_elements_by_tag("tr"), 2);
        assert_eq!(arena.count_elements_by_tag("td"), 3);
    }
}
