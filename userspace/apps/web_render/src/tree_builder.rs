// userspace/apps/web_render/src/tree_builder.rs
// Bounded HTML5 Tree Construction with WHATWG Error Recovery and Inert Script Execution (Milestone WEB-02B)

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use crate::dom::{DomArena, ElementData, NodeData, NodeId};
use crate::limits::{LimitKind, ParseError, WorkBudgetTracker, MAX_TREE_DEPTH};
use crate::tokenizer::HtmlToken;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InsertionMode {
    Initial,
    BeforeHtml,
    BeforeHead,
    InHead,
    AfterHead,
    InBody,
    AfterBody,
}

pub struct HtmlTreeBuilder {
    mode: InsertionMode,
    open_elements: Vec<NodeId>,
    head_element: Option<NodeId>,
    body_element: Option<NodeId>,
}

impl HtmlTreeBuilder {
    pub fn new() -> Self {
        Self {
            mode: InsertionMode::Initial,
            open_elements: Vec::new(),
            head_element: None,
            body_element: None,
        }
    }

    pub fn process_token(
        &mut self,
        token: HtmlToken,
        arena: &mut DomArena,
        budget: &mut WorkBudgetTracker,
    ) -> Result<(), ParseError> {
        budget.charge(1)?;

        match token {
            HtmlToken::Doctype { name } => {
                if self.mode == InsertionMode::Initial {
                    let doc_id = arena.document_id;
                    let dt_id = arena.alloc_node(NodeData::Doctype { name })?;
                    arena.append_child(doc_id, dt_id);
                    self.mode = InsertionMode::BeforeHtml;
                }
                // Later doctypes ignored per standard
                Ok(())
            }

            HtmlToken::Comment(comment) => {
                let target_parent = self.current_node().unwrap_or(arena.document_id);
                let c_id = arena.alloc_node(NodeData::Comment(comment))?;
                arena.append_child(target_parent, c_id);
                Ok(())
            }

            HtmlToken::Character(c) => {
                // If in initial or before html/head, ignore whitespace
                if self.mode == InsertionMode::Initial || self.mode == InsertionMode::BeforeHtml {
                    if c.is_ascii_whitespace() {
                        return Ok(());
                    }
                    // Non-whitespace character before html: ensure html exists
                    self.ensure_html_element(arena, budget)?;
                }

                if self.mode == InsertionMode::BeforeHead {
                    if c.is_ascii_whitespace() {
                        return Ok(());
                    }
                    self.ensure_head_element(arena, budget)?;
                }

                let target_parent = self.current_node().unwrap_or(arena.document_id);

                // Coalesce text with existing last child if already a Text node
                let coalesced = if let Some(last_child_id) = arena.get(target_parent).and_then(|p| p.last_child) {
                    if let Some(last_node) = arena.get_mut(last_child_id) {
                        if let NodeData::Text(ref mut s) = last_node.data {
                            s.push(c);
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                } else {
                    false
                };

                if !coalesced {
                    let mut s = String::new();
                    s.push(c);
                    let text_id = arena.alloc_node(NodeData::Text(s))?;
                    arena.append_child(target_parent, text_id);
                }

                Ok(())
            }

            HtmlToken::StartTag { name, attributes, self_closing } => {
                self.process_start_tag(name, attributes, self_closing, arena, budget)
            }

            HtmlToken::EndTag { name } => {
                self.process_end_tag(name, arena, budget)
            }

            HtmlToken::EndOfFile => {
                // Close all remaining open elements safely
                self.open_elements.clear();
                Ok(())
            }
        }
    }

    fn current_node(&self) -> Option<NodeId> {
        self.open_elements.last().copied()
    }

    fn ensure_html_element(
        &mut self,
        arena: &mut DomArena,
        budget: &mut WorkBudgetTracker,
    ) -> Result<(), ParseError> {
        budget.charge(1)?;
        if self.open_elements.is_empty() {
            let doc_id = arena.document_id;
            let html_id = arena.alloc_node(NodeData::Element(ElementData {
                tag_name: String::from("html"),
                attributes: Vec::new(),
            }))?;
            arena.append_child(doc_id, html_id);
            self.push_element(html_id)?;
            self.mode = InsertionMode::BeforeHead;
        }
        Ok(())
    }

    fn ensure_head_element(
        &mut self,
        arena: &mut DomArena,
        budget: &mut WorkBudgetTracker,
    ) -> Result<(), ParseError> {
        self.ensure_html_element(arena, budget)?;
        if self.head_element.is_none() {
            let parent = self.current_node().unwrap_or(arena.document_id);
            let head_id = arena.alloc_node(NodeData::Element(ElementData {
                tag_name: String::from("head"),
                attributes: Vec::new(),
            }))?;
            arena.append_child(parent, head_id);
            self.head_element = Some(head_id);
            self.push_element(head_id)?;
            self.mode = InsertionMode::InHead;
        }
        Ok(())
    }

    fn ensure_body_element(
        &mut self,
        arena: &mut DomArena,
        budget: &mut WorkBudgetTracker,
    ) -> Result<(), ParseError> {
        self.ensure_html_element(arena, budget)?;

        // If head is open, close it
        if self.mode == InsertionMode::InHead {
            self.pop_element_by_tag("head", arena);
            self.mode = InsertionMode::AfterHead;
        }

        if self.body_element.is_none() {
            // Find html element to append body to
            let html_id = self.open_elements.first().copied().unwrap_or(arena.document_id);
            let body_id = arena.alloc_node(NodeData::Element(ElementData {
                tag_name: String::from("body"),
                attributes: Vec::new(),
            }))?;
            arena.append_child(html_id, body_id);
            self.body_element = Some(body_id);
            self.push_element(body_id)?;
            self.mode = InsertionMode::InBody;
        }
        Ok(())
    }

    fn push_element(&mut self, node_id: NodeId) -> Result<(), ParseError> {
        if self.open_elements.len() >= MAX_TREE_DEPTH {
            return Err(ParseError::LimitExceeded(LimitKind::TreeDepth));
        }
        self.open_elements.try_reserve(1).map_err(|_| ParseError::OutOfMemory)?;
        self.open_elements.push(node_id);
        Ok(())
    }

    fn pop_element_by_tag(&mut self, tag: &str, arena: &DomArena) -> Option<NodeId> {
        for idx in (0..self.open_elements.len()).rev() {
            let node_id = self.open_elements[idx];
            if let Some(node) = arena.get(node_id) {
                if let NodeData::Element(ref el) = node.data {
                    if el.tag_name.eq_ignore_ascii_case(tag) {
                        self.open_elements.truncate(idx);
                        return Some(node_id);
                    }
                }
            }
        }
        None
    }

    fn current_tag_matches(&self, tag: &str, arena: &DomArena) -> bool {
        if let Some(curr_id) = self.current_node() {
            if let Some(node) = arena.get(curr_id) {
                if let NodeData::Element(ref el) = node.data {
                    return el.tag_name.eq_ignore_ascii_case(tag);
                }
            }
        }
        false
    }

    fn has_open_element(&self, tag: &str, arena: &DomArena) -> bool {
        self.open_elements.iter().any(|&nid| {
            if let Some(node) = arena.get(nid) {
                if let NodeData::Element(ref el) = node.data {
                    return el.tag_name.eq_ignore_ascii_case(tag);
                }
            }
            false
        })
    }

    fn process_start_tag(
        &mut self,
        name: String,
        attributes: Vec<crate::dom::Attribute>,
        self_closing: bool,
        arena: &mut DomArena,
        budget: &mut WorkBudgetTracker,
    ) -> Result<(), ParseError> {
        let tag_lower = name.to_ascii_lowercase();

        if tag_lower == "html" {
            if self.open_elements.is_empty() {
                let doc_id = arena.document_id;
                let html_id = arena.alloc_node(NodeData::Element(ElementData {
                    tag_name: name,
                    attributes,
                }))?;
                arena.append_child(doc_id, html_id);
                self.push_element(html_id)?;
                self.mode = InsertionMode::BeforeHead;
            }
            return Ok(());
        }

        if tag_lower == "head" {
            self.ensure_html_element(arena, budget)?;
            let parent = self.current_node().unwrap_or(arena.document_id);
            let head_id = arena.alloc_node(NodeData::Element(ElementData {
                tag_name: name,
                attributes,
            }))?;
            arena.append_child(parent, head_id);
            self.head_element = Some(head_id);
            self.push_element(head_id)?;
            self.mode = InsertionMode::InHead;
            return Ok(());
        }

        if tag_lower == "body" {
            self.ensure_body_element(arena, budget)?;
            return Ok(());
        }

        // Tags permitted inside <head>
        if self.mode == InsertionMode::InHead || self.mode == InsertionMode::BeforeHead {
            if matches!(tag_lower.as_str(), "title" | "meta" | "link" | "style" | "script" | "base") {
                self.ensure_head_element(arena, budget)?;
                let parent = self.current_node().unwrap_or(arena.document_id);
                let el_id = arena.alloc_node(NodeData::Element(ElementData {
                    tag_name: name.clone(),
                    attributes,
                }))?;
                arena.append_child(parent, el_id);
                if !is_void_element(&tag_lower) && !self_closing {
                    self.push_element(el_id)?;
                }
                return Ok(());
            } else {
                // Any other tag in head implicitly closes head and opens body
                self.ensure_body_element(arena, budget)?;
            }
        } else if self.mode == InsertionMode::Initial || self.mode == InsertionMode::BeforeHtml {
            self.ensure_body_element(arena, budget)?;
        }

        // We are InBody (or AfterHead)
        // WHATWG Subset Rule: Implicit Paragraph Closing
        // When a block element opens, if <p> is in button/paragraph scope, close <p>
        if is_block_element(&tag_lower) {
            if self.has_open_element("p", arena) {
                self.pop_element_by_tag("p", arena);
            }
        }

        // List item closing: <li> closes open <li>
        if tag_lower == "li" {
            if self.current_tag_matches("li", arena) {
                self.open_elements.pop();
            }
        }

        // Table structural closing
        if tag_lower == "tr" {
            if self.current_tag_matches("tr", arena) {
                self.open_elements.pop();
            }
        } else if tag_lower == "td" || tag_lower == "th" {
            if self.current_tag_matches("td", arena) || self.current_tag_matches("th", arena) {
                self.open_elements.pop();
            }
        }

        let is_void = is_void_element(&tag_lower);
        let parent = self.current_node().unwrap_or(arena.document_id);
        let el_id = arena.alloc_node(NodeData::Element(ElementData {
            tag_name: name,
            attributes,
        }))?;
        arena.append_child(parent, el_id);

        if !is_void && !self_closing {
            self.push_element(el_id)?;
        }

        Ok(())
    }

    fn process_end_tag(
        &mut self,
        name: String,
        arena: &mut DomArena,
        _budget: &mut WorkBudgetTracker,
    ) -> Result<(), ParseError> {
        let tag_lower = name.to_ascii_lowercase();

        if tag_lower == "head" && self.mode == InsertionMode::InHead {
            self.pop_element_by_tag("head", arena);
            self.mode = InsertionMode::AfterHead;
            return Ok(());
        }

        if tag_lower == "body" {
            self.mode = InsertionMode::AfterBody;
            return Ok(());
        }

        if tag_lower == "html" {
            return Ok(());
        }

        // Table close: </table> closes internal cells/rows first
        if tag_lower == "table" {
            while let Some(top_id) = self.current_node() {
                if let Some(node) = arena.get(top_id) {
                    if let NodeData::Element(ref el) = node.data {
                        if el.tag_name.eq_ignore_ascii_case("table") {
                            self.open_elements.pop();
                            break;
                        } else if matches!(el.tag_name.to_ascii_lowercase().as_str(), "td" | "th" | "tr" | "tbody" | "thead" | "tfoot") {
                            self.open_elements.pop();
                            continue;
                        }
                    }
                }
                break;
            }
            return Ok(());
        }

        // Standard tag matching: scan stack backwards
        self.pop_element_by_tag(&tag_lower, arena);
        Ok(())
    }
}

pub fn is_void_element(tag: &str) -> bool {
    matches!(
        tag,
        "area" | "base" | "br" | "col" | "embed" | "hr" | "img" | "input" | "link" | "meta" | "param" | "source" | "track" | "wbr"
    )
}

pub fn is_block_element(tag: &str) -> bool {
    matches!(
        tag,
        "address" | "article" | "aside" | "blockquote" | "details" | "dialog" | "dd" | "div" | "dl" | "dt"
        | "fieldset" | "figcaption" | "figure" | "footer" | "form" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
        | "header" | "hgroup" | "hr" | "main" | "menu" | "nav" | "ol" | "p" | "pre" | "section" | "table" | "ul"
    )
}
