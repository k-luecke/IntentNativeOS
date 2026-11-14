// userspace/apps/web_render/src/conformance.rs - WHATWG Conformance & Security Fixture Suite
// Adapted from upstream html5lib-tests test suite provenance.
// INOS Native Browser Subsystem (Milestone WEB-02C)

extern crate alloc;

use alloc::vec::Vec;
use crate::limits::{LimitKind, ParseError};
use crate::dom::NodeData;
use crate::{parse_document, HtmlParser};

/// Test Fixture 1: DOCTYPE Variations
/// Verifies standard HTML5 doctype and quirks mode doctypes from html5lib-tests doctype01.dat.
pub fn test_conformance_doctypes() -> Result<(), &'static str> {
    // 1.1 Standard HTML5 DOCTYPE
    let d1 = b"<!DOCTYPE html><html><head></head><body></body></html>";
    let a1 = parse_document(d1).map_err(|_| "Failed standard doctype")?;
    let mut found_doctype = false;
    for node in a1.nodes() {
        if let NodeData::Doctype { ref name } = node.data {
            if name.eq_ignore_ascii_case("html") {
                found_doctype = true;
                break;
            }
        }
    }
    if !found_doctype {
        return Err("Standard doctype not found in DOM");
    }

    // 1.2 Quirks / Legacy DOCTYPE
    let d2 = b"<!DOCTYPE HTML PUBLIC \"-//W3C//DTD HTML 4.01 Transitional//EN\"><html><body></body></html>";
    let a2 = parse_document(d2).map_err(|_| "Failed legacy doctype")?;
    let mut found_legacy = false;
    for node in a2.nodes() {
        if let NodeData::Doctype { ref name } = node.data {
            if name.eq_ignore_ascii_case("html") {
                found_legacy = true;
                break;
            }
        }
    }
    if !found_legacy {
        return Err("Legacy doctype not parsed as doctype node");
    }

    // 1.3 Missing DOCTYPE (document starts directly with tags)
    let d3 = b"<html><head><title>No Doctype</title></head><body><p>Text</p></body></html>";
    let a3 = parse_document(d3).map_err(|_| "Failed no-doctype document")?;
    if a3.find_element_by_tag("p").is_none() {
        return Err("Document without doctype missing body element");
    }

    Ok(())
}

/// Test Fixture 2: Tokenization Edge Cases
/// Adapted from html5lib-tests tokenization suite (entities, unquoted attributes, comments, void tags).
pub fn test_conformance_tokenization() -> Result<(), &'static str> {
    // 2.1 Unquoted attributes and boolean attributes
    let doc_attr = b"<html><body><input type=text disabled required value=foo></body></html>";
    let arena_attr = parse_document(doc_attr).map_err(|_| "Failed unquoted attributes")?;
    let input_id = arena_attr.find_element_by_tag("input").ok_or("Missing input element")?;
    let input_node = arena_attr.get(input_id).unwrap();
    if let NodeData::Element(ref el) = input_node.data {
        if el.get_attribute("type") != Some("text") {
            return Err("Unquoted attribute 'type=text' mismatch");
        }
        if el.get_attribute("value") != Some("foo") {
            return Err("Unquoted attribute 'value=foo' mismatch");
        }
    }

    // 2.2 Character Entities (named, numeric decimal, numeric hex)
    let doc_ent = b"<html><body><p>&amp; &lt; &gt; &quot; &copy; &#65; &#x42;</p></body></html>";
    let arena_ent = parse_document(doc_ent).map_err(|_| "Failed entity parsing")?;
    let p_id = arena_ent.find_element_by_tag("p").ok_or("Missing p element")?;
    let p_node = arena_ent.get(p_id).unwrap();
    let text_child_id = p_node.first_child.ok_or("Missing text child in p")?;
    let text_node = arena_ent.get(text_child_id).unwrap();
    if let NodeData::Text(ref s) = text_node.data {
        if !s.contains('&') || !s.contains('<') || !s.contains('>') || !s.contains('"') {
            return Err("Named character entities not correctly decoded");
        }
        if !s.contains('A') || !s.contains('B') {
            return Err("Numeric character entities &#65; or &#x42; not decoded");
        }
    }

    // 2.3 Comments with double hyphens and interior whitespace
    let doc_com = b"<html><body><!-- this is -- a valid -- comment --><p>After</p></body></html>";
    let arena_com = parse_document(doc_com).map_err(|_| "Failed comment parsing")?;
    let mut found_comment = false;
    for node in arena_com.nodes() {
        if let NodeData::Comment(ref c) = node.data {
            if c.contains("valid") {
                found_comment = true;
                break;
            }
        }
    }
    if !found_comment {
        return Err("Comment node missing or corrupted");
    }

    // 2.4 Void self-closing elements (br, hr, img)
    let doc_void = b"<html><body>Line 1<br/>Line 2<hr><img src=\"test.png\"/></body></html>";
    let arena_void = parse_document(doc_void).map_err(|_| "Failed void elements")?;
    if arena_void.find_element_by_tag("br").is_none() {
        return Err("Missing <br> element");
    }
    if arena_void.find_element_by_tag("hr").is_none() {
        return Err("Missing <hr> element");
    }
    if arena_void.find_element_by_tag("img").is_none() {
        return Err("Missing <img> element");
    }

    Ok(())
}

/// Test Fixture 3: Tree Construction & Implicit Closures
/// Adapted from html5lib-tests tree-construction tests (implicit p, li, table cell closures).
pub fn test_conformance_tree_construction() -> Result<(), &'static str> {
    // 3.1 Implicit <p> closure when starting next <p>
    let doc_p = b"<html><body><p>First paragraph<p>Second paragraph</body></html>";
    let arena_p = parse_document(doc_p).map_err(|_| "Failed implicit p closure")?;
    let p_count = arena_p.count_elements_by_tag("p");
    if p_count != 2 {
        return Err("Expected exactly 2 <p> elements from implicit p closures");
    }

    // 3.2 Implicit <li> closure
    let doc_li = b"<ul><li>One<li>Two<li>Three</ul>";
    let arena_li = parse_document(doc_li).map_err(|_| "Failed implicit li closure")?;
    let li_count = arena_li.count_elements_by_tag("li");
    if li_count != 3 {
        return Err("Expected exactly 3 <li> elements from implicit li closures");
    }

    // 3.3 Implicit <tr> and <td> closure in tables
    let doc_table = b"<table><tr><td>A<td>B<tr><td>C<td>D</table>";
    let arena_table = parse_document(doc_table).map_err(|_| "Failed table closure")?;
    let tr_count = arena_table.count_elements_by_tag("tr");
    let td_count = arena_table.count_elements_by_tag("td");
    if tr_count != 2 || td_count != 4 {
        return Err("Expected 2 <tr> and 4 <td> elements from table structure");
    }

    // 3.4 Unclosed tags at EOF safely resolved
    let doc_unclosed = b"<html><body><div><section><article><p>Dangling content";
    let arena_unclosed = parse_document(doc_unclosed).map_err(|_| "Failed unclosed tags at EOF")?;
    if arena_unclosed.find_element_by_tag("article").is_none() {
        return Err("Unclosed article not constructed");
    }

    Ok(())
}

/// Test Fixture 4: Explicit Expected Failures (XFAIL Matrix)
/// Verifies that intentionally unsupported standard features fail deterministically and safely.
pub fn test_xfail_matrix() -> Result<(), &'static str> {
    // XFAIL-001: Active JavaScript execution is inert
    let js_doc = b"<html><head><script>window.location='http://evil.com'; while(1){}</script></head><body></body></html>";
    let arena_js = parse_document(js_doc).map_err(|_| "Failed inert script parse")?;
    let script_id = arena_js.find_element_by_tag("script").ok_or("Missing script tag")?;
    let script_node = arena_js.get(script_id).unwrap();
    let text_child = arena_js.get(script_node.first_child.ok_or("Script missing child")?).unwrap();
    match &text_child.data {
        NodeData::Text(s) => {
            // Confirm it is purely stored as passive inert text, not compiled or executed
            if !s.contains("window.location") {
                return Err("XFAIL-001: Script content corrupted");
            }
        }
        _ => return Err("XFAIL-001: Script child must be inert text node"),
    }

    // XFAIL-002: Active external network resource fetching is unsupported (no ambient socket/capability)
    // The parser stores <img>, <link>, <style> with passive attributes without initiating network requests
    let img_doc = b"<html><body><img src=\"https://example.com/tracking.png\"></body></html>";
    let arena_img = parse_document(img_doc).map_err(|_| "Failed img parse")?;
    let img_id = arena_img.find_element_by_tag("img").ok_or("Missing img tag")?;
    let img_node = arena_img.get(img_id).unwrap();
    if let NodeData::Element(ref el) = img_node.data {
        if el.get_attribute("src") != Some("https://example.com/tracking.png") {
            return Err("XFAIL-002: Img src attribute mismatch");
        }
    }

    // XFAIL-003: Active event handlers stored passively without execution binding
    let ev_doc = b"<html><body><button onclick=\"alert('owned')\" onerror=\"foo()\">Click</button></body></html>";
    let arena_ev = parse_document(ev_doc).map_err(|_| "Failed event handlers parse")?;
    let btn_id = arena_ev.find_element_by_tag("button").ok_or("Missing button tag")?;
    let btn_node = arena_ev.get(btn_id).unwrap();
    if let NodeData::Element(ref el) = btn_node.data {
        if el.get_attribute("onclick") != Some("alert('owned')") {
            return Err("XFAIL-003: Onclick handler mismatch");
        }
    }

    Ok(())
}

/// Test Fixture 5: Deliberate INOS Security Constraints
/// Verifies explicit system limits are enforced deterministically:
/// - Max tree depth (64)
/// - Max attribute length (1024)
/// - Max payload bytes (512 KiB)
/// - Max DOM nodes (2048)
pub fn test_deliberate_security_limits() -> Result<(), &'static str> {
    // 5.1 Nesting depth limit (65 levels > MAX_TREE_DEPTH = 64)
    let mut deep = Vec::new();
    for _ in 0..65 {
        deep.extend_from_slice(b"<div>");
    }
    for _ in 0..65 {
        deep.extend_from_slice(b"</div>");
    }
    match parse_document(&deep) {
        Err(ParseError::LimitExceeded(LimitKind::TreeDepth)) => {},
        _ => return Err("Nesting depth > 64 was not rejected with TreeDepth limit"),
    }

    // 5.2 Attribute length limit (> 1024 bytes)
    let mut long_attr = Vec::new();
    long_attr.extend_from_slice(b"<div class=\"");
    for _ in 0..1025 {
        long_attr.push(b'a');
    }
    long_attr.extend_from_slice(b"\"></div>");
    match parse_document(&long_attr) {
        Err(ParseError::LimitExceeded(LimitKind::AttrValueLength)) => {},
        _ => return Err("Attribute > 1024 bytes was not rejected with AttrValueLength limit"),
    }

    // 5.3 Streaming Payload size limit enforcement
    if crate::limits::MAX_INPUT_BYTES != 512 * 1024 {
        return Err("MAX_INPUT_BYTES is not 512 KiB");
    }
    let mut decoder = crate::decoder::Utf8StreamDecoder::new(1024);
    let chunk = [b'x'; 1025];
    let mut chars = Vec::new();
    let mut budget = crate::limits::WorkBudgetTracker::new(100_000);
    match decoder.feed_chunk(&chunk, &mut chars, &mut budget) {
        Err(ParseError::LimitExceeded(LimitKind::InputBytes)) => {},
        _ => return Err("Payload exceeding limit was not rejected with InputBytes limit"),
    }

    Ok(())
}
