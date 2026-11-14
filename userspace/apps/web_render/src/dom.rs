// userspace/apps/web_render/src/dom.rs
// Bounded Arena-Backed Document Object Model (Milestone WEB-02B)

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use crate::limits::{LimitKind, ParseError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribute {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementData {
    pub tag_name: String,
    pub attributes: Vec<Attribute>,
}

impl ElementData {
    pub fn get_attribute(&self, name: &str) -> Option<&str> {
        for attr in &self.attributes {
            if attr.name.eq_ignore_ascii_case(name) {
                return Some(&attr.value);
            }
        }
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeData {
    Document,
    Doctype { name: String },
    Element(ElementData),
    Text(String),
    Comment(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub id: NodeId,
    pub parent: Option<NodeId>,
    pub first_child: Option<NodeId>,
    pub last_child: Option<NodeId>,
    pub next_sibling: Option<NodeId>,
    pub prev_sibling: Option<NodeId>,
    pub data: NodeData,
}

impl Node {
    pub fn new(id: NodeId, data: NodeData) -> Self {
        Self {
            id,
            parent: None,
            first_child: None,
            last_child: None,
            next_sibling: None,
            prev_sibling: None,
            data,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomArena {
    nodes: Vec<Node>,
    max_nodes: usize,
    pub document_id: NodeId,
}

impl DomArena {
    pub fn new(max_nodes: usize) -> Result<Self, ParseError> {
        let mut nodes = Vec::new();
        nodes.try_reserve(core::cmp::min(max_nodes, 128))
            .map_err(|_| ParseError::OutOfMemory)?;

        let doc_id = NodeId(0);
        let doc_node = Node::new(doc_id, NodeData::Document);
        nodes.push(doc_node);

        Ok(Self {
            nodes,
            max_nodes,
            document_id: doc_id,
        })
    }

    pub fn alloc_node(&mut self, data: NodeData) -> Result<NodeId, ParseError> {
        if self.nodes.len() >= self.max_nodes {
            return Err(ParseError::LimitExceeded(LimitKind::DomNodes));
        }
        let id = NodeId(self.nodes.len() as u32);
        self.nodes.try_reserve(1).map_err(|_| ParseError::OutOfMemory)?;
        self.nodes.push(Node::new(id, data));
        Ok(id)
    }

    pub fn get(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id.0 as usize)
    }

    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.nodes.get_mut(id.0 as usize)
    }

    pub fn append_child(&mut self, parent_id: NodeId, child_id: NodeId) {
        if parent_id == child_id {
            return;
        }

        // Set child's parent
        if let Some(child) = self.get_mut(child_id) {
            child.parent = Some(parent_id);
            child.next_sibling = None;
        }

        let old_last = self.get(parent_id).and_then(|p| p.last_child);
        if let Some(last_id) = old_last {
            if let Some(last_node) = self.get_mut(last_id) {
                last_node.next_sibling = Some(child_id);
            }
            if let Some(child) = self.get_mut(child_id) {
                child.prev_sibling = Some(last_id);
            }
        } else {
            // First child
            if let Some(parent) = self.get_mut(parent_id) {
                parent.first_child = Some(child_id);
            }
            if let Some(child) = self.get_mut(child_id) {
                child.prev_sibling = None;
            }
        }

        if let Some(parent) = self.get_mut(parent_id) {
            parent.last_child = Some(child_id);
        }
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn find_element_by_tag(&self, tag: &str) -> Option<NodeId> {
        for node in &self.nodes {
            if let NodeData::Element(ref el) = node.data {
                if el.tag_name.eq_ignore_ascii_case(tag) {
                    return Some(node.id);
                }
            }
        }
        None
    }

    pub fn count_elements_by_tag(&self, tag: &str) -> usize {
        let mut count = 0;
        for node in &self.nodes {
            if let NodeData::Element(ref el) = node.data {
                if el.tag_name.eq_ignore_ascii_case(tag) {
                    count += 1;
                }
            }
        }
        count
    }
}
