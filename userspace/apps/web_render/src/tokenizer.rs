// userspace/apps/web_render/src/tokenizer.rs
// Streaming HTML5 Tokenizer with State Preservation Across Chunks (Milestone WEB-02B)

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use crate::dom::Attribute;
use crate::limits::{
    LimitKind, ParseError, WorkBudgetTracker, MAX_ATTRIBUTES_PER_ELEMENT,
    MAX_ATTR_NAME_LEN, MAX_ATTR_VALUE_LEN, MAX_TOKENS,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HtmlToken {
    Doctype {
        name: String,
    },
    StartTag {
        name: String,
        attributes: Vec<Attribute>,
        self_closing: bool,
    },
    EndTag {
        name: String,
    },
    Comment(String),
    Character(char),
    EndOfFile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenizerState {
    Data,
    TagOpen,
    EndTagOpen,
    TagName,
    BeforeAttributeName,
    AttributeName,
    AfterAttributeName,
    BeforeAttributeValue,
    AttributeValueDoubleQuoted,
    AttributeValueSingleQuoted,
    AttributeValueUnquoted,
    AfterAttributeValueQuoted,
    SelfClosingStartTag,
    MarkupDeclarationOpen,
    Doctype,
    Comment,
    CommentEndDash,
    CommentEnd,
    CharacterReference,
    AttrEntityDoubleQuoted,
    AttrEntitySingleQuoted,
    AttrEntityUnquoted,
    RawText,
    RawTextEndTagOpen,
    RawTextEndTagName,
}

pub struct HtmlTokenizer {
    state: TokenizerState,
    pending_tag_name: String,
    pending_is_end_tag: bool,
    pending_self_closing: bool,
    pending_attrs: Vec<Attribute>,
    pending_attr_name: String,
    pending_attr_val: String,
    pending_comment: String,
    pending_entity: String,
    raw_text_tag: String,
    raw_text_closing_buf: String,
    markup_decl_buf: String,
    total_tokens: usize,
}

impl HtmlTokenizer {
    pub fn new() -> Self {
        Self {
            state: TokenizerState::Data,
            pending_tag_name: String::new(),
            pending_is_end_tag: false,
            pending_self_closing: false,
            pending_attrs: Vec::new(),
            pending_attr_name: String::new(),
            pending_attr_val: String::new(),
            pending_comment: String::new(),
            pending_entity: String::new(),
            raw_text_tag: String::new(),
            raw_text_closing_buf: String::new(),
            markup_decl_buf: String::new(),
            total_tokens: 0,
        }
    }

    fn emit_token(
        &mut self,
        token: HtmlToken,
        out_tokens: &mut Vec<HtmlToken>,
        budget: &mut WorkBudgetTracker,
    ) -> Result<(), ParseError> {
        self.total_tokens = self.total_tokens.checked_add(1)
            .ok_or(ParseError::LimitExceeded(LimitKind::Tokens))?;
        if self.total_tokens > MAX_TOKENS {
            return Err(ParseError::LimitExceeded(LimitKind::Tokens));
        }
        budget.charge(1)?;
        out_tokens.try_reserve(1).map_err(|_| ParseError::OutOfMemory)?;
        out_tokens.push(token);
        Ok(())
    }

    fn flush_pending_attribute(&mut self) -> Result<(), ParseError> {
        if self.pending_attr_name.is_empty() {
            return Ok(());
        }

        let name_lower = self.pending_attr_name.to_ascii_lowercase();
        let exists = self.pending_attrs.iter().any(|a| a.name.eq_ignore_ascii_case(&name_lower));
        if !exists {
            if self.pending_attrs.len() >= MAX_ATTRIBUTES_PER_ELEMENT {
                return Err(ParseError::LimitExceeded(LimitKind::AttributeCount));
            }
            self.pending_attrs.try_reserve(1).map_err(|_| ParseError::OutOfMemory)?;
            self.pending_attrs.push(Attribute {
                name: core::mem::take(&mut self.pending_attr_name),
                value: core::mem::take(&mut self.pending_attr_val),
            });
        } else {
            self.pending_attr_name.clear();
            self.pending_attr_val.clear();
        }
        Ok(())
    }

    fn push_attr_val_char(&mut self, c: char) -> Result<(), ParseError> {
        if self.pending_attr_val.len() >= MAX_ATTR_VALUE_LEN {
            return Err(ParseError::LimitExceeded(LimitKind::AttrValueLength));
        }
        self.pending_attr_val.push(c);
        Ok(())
    }

    fn finish_current_tag(
        &mut self,
        out_tokens: &mut Vec<HtmlToken>,
        budget: &mut WorkBudgetTracker,
    ) -> Result<(), ParseError> {
        let tag_name = core::mem::take(&mut self.pending_tag_name);
        let is_end_tag = self.pending_is_end_tag;
        let self_closing = self.pending_self_closing;
        let attributes = core::mem::take(&mut self.pending_attrs);

        if is_end_tag {
            self.emit_token(HtmlToken::EndTag { name: tag_name }, out_tokens, budget)?;
            self.state = TokenizerState::Data;
        } else {
            let is_raw_text = tag_name.eq_ignore_ascii_case("script") || tag_name.eq_ignore_ascii_case("style");
            if is_raw_text && !self_closing {
                self.raw_text_tag = tag_name.clone();
                self.state = TokenizerState::RawText;
            } else {
                self.state = TokenizerState::Data;
            }

            self.emit_token(
                HtmlToken::StartTag {
                    name: tag_name,
                    attributes,
                    self_closing,
                },
                out_tokens,
                budget,
            )?;
        }

        Ok(())
    }

    /// Feeds decoded characters into the tokenizer state machine.
    pub fn feed_chars(
        &mut self,
        chars: &[char],
        out_tokens: &mut Vec<HtmlToken>,
        budget: &mut WorkBudgetTracker,
    ) -> Result<(), ParseError> {
        let mut idx = 0;
        let len = chars.len();

        while idx < len {
            let c = chars[idx];
            budget.charge(1)?;

            let consumed = self.step_char(c, out_tokens, budget)?;
            if consumed {
                idx += 1;
            }
        }

        Ok(())
    }

    fn step_char(
        &mut self,
        c: char,
        out_tokens: &mut Vec<HtmlToken>,
        budget: &mut WorkBudgetTracker,
    ) -> Result<bool, ParseError> {
        match self.state {
            TokenizerState::Data => {
                if c == '<' {
                    self.state = TokenizerState::TagOpen;
                } else if c == '&' {
                    self.pending_entity.clear();
                    self.state = TokenizerState::CharacterReference;
                } else {
                    self.emit_token(HtmlToken::Character(c), out_tokens, budget)?;
                }
                Ok(true)
            }

            TokenizerState::CharacterReference => {
                if c == ';' {
                    if let Some(resolved) = resolve_entity(&self.pending_entity) {
                        self.emit_token(HtmlToken::Character(resolved), out_tokens, budget)?;
                    } else {
                        self.emit_token(HtmlToken::Character('&'), out_tokens, budget)?;
                        for ec in self.pending_entity.clone().chars() {
                            self.emit_token(HtmlToken::Character(ec), out_tokens, budget)?;
                        }
                        self.emit_token(HtmlToken::Character(';'), out_tokens, budget)?;
                    }
                    self.pending_entity.clear();
                    self.state = TokenizerState::Data;
                    Ok(true)
                } else if c.is_alphanumeric() || c == '#' {
                    if self.pending_entity.len() < 16 {
                        self.pending_entity.push(c);
                        Ok(true)
                    } else {
                        self.emit_token(HtmlToken::Character('&'), out_tokens, budget)?;
                        for ec in self.pending_entity.clone().chars() {
                            self.emit_token(HtmlToken::Character(ec), out_tokens, budget)?;
                        }
                        self.pending_entity.clear();
                        self.state = TokenizerState::Data;
                        Ok(false)
                    }
                } else {
                    self.emit_token(HtmlToken::Character('&'), out_tokens, budget)?;
                    for ec in self.pending_entity.clone().chars() {
                        self.emit_token(HtmlToken::Character(ec), out_tokens, budget)?;
                    }
                    self.pending_entity.clear();
                    self.state = TokenizerState::Data;
                    Ok(false)
                }
            }

            TokenizerState::TagOpen => {
                if c == '/' {
                    self.pending_is_end_tag = true;
                    self.pending_tag_name.clear();
                    self.state = TokenizerState::EndTagOpen;
                    Ok(true)
                } else if c == '!' {
                    self.markup_decl_buf.clear();
                    self.state = TokenizerState::MarkupDeclarationOpen;
                    Ok(true)
                } else if c.is_ascii_alphabetic() {
                    self.pending_is_end_tag = false;
                    self.pending_self_closing = false;
                    self.pending_tag_name.clear();
                    self.pending_attrs.clear();
                    self.pending_tag_name.push(c.to_ascii_lowercase());
                    self.state = TokenizerState::TagName;
                    Ok(true)
                } else if c == '?' {
                    self.pending_comment.clear();
                    self.state = TokenizerState::Comment;
                    Ok(true)
                } else {
                    self.emit_token(HtmlToken::Character('<'), out_tokens, budget)?;
                    self.state = TokenizerState::Data;
                    Ok(false)
                }
            }

            TokenizerState::EndTagOpen => {
                if c.is_ascii_alphabetic() {
                    self.pending_tag_name.push(c.to_ascii_lowercase());
                    self.state = TokenizerState::TagName;
                    Ok(true)
                } else if c == '>' {
                    self.state = TokenizerState::Data;
                    Ok(true)
                } else {
                    self.pending_comment.clear();
                    self.state = TokenizerState::Comment;
                    Ok(true)
                }
            }

            TokenizerState::TagName => {
                if c.is_ascii_whitespace() {
                    self.state = TokenizerState::BeforeAttributeName;
                } else if c == '/' {
                    self.state = TokenizerState::SelfClosingStartTag;
                } else if c == '>' {
                    self.finish_current_tag(out_tokens, budget)?;
                } else {
                    if self.pending_tag_name.len() < MAX_ATTR_NAME_LEN {
                        self.pending_tag_name.push(c.to_ascii_lowercase());
                    }
                }
                Ok(true)
            }

            TokenizerState::BeforeAttributeName => {
                if c.is_ascii_whitespace() {
                    // Skip whitespace
                } else if c == '/' {
                    self.state = TokenizerState::SelfClosingStartTag;
                } else if c == '>' {
                    self.finish_current_tag(out_tokens, budget)?;
                } else {
                    self.pending_attr_name.clear();
                    self.pending_attr_val.clear();
                    self.pending_attr_name.push(c.to_ascii_lowercase());
                    self.state = TokenizerState::AttributeName;
                }
                Ok(true)
            }

            TokenizerState::AttributeName => {
                if c.is_ascii_whitespace() {
                    self.state = TokenizerState::AfterAttributeName;
                } else if c == '/' {
                    self.flush_pending_attribute()?;
                    self.state = TokenizerState::SelfClosingStartTag;
                } else if c == '=' {
                    self.state = TokenizerState::BeforeAttributeValue;
                } else if c == '>' {
                    self.flush_pending_attribute()?;
                    self.finish_current_tag(out_tokens, budget)?;
                } else {
                    if self.pending_attr_name.len() < MAX_ATTR_NAME_LEN {
                        self.pending_attr_name.push(c.to_ascii_lowercase());
                    }
                }
                Ok(true)
            }

            TokenizerState::AfterAttributeName => {
                if c.is_ascii_whitespace() {
                    // Skip
                } else if c == '=' {
                    self.state = TokenizerState::BeforeAttributeValue;
                } else if c == '/' {
                    self.flush_pending_attribute()?;
                    self.state = TokenizerState::SelfClosingStartTag;
                } else if c == '>' {
                    self.flush_pending_attribute()?;
                    self.finish_current_tag(out_tokens, budget)?;
                } else {
                    self.flush_pending_attribute()?;
                    self.pending_attr_name.clear();
                    self.pending_attr_val.clear();
                    self.pending_attr_name.push(c.to_ascii_lowercase());
                    self.state = TokenizerState::AttributeName;
                }
                Ok(true)
            }

            TokenizerState::BeforeAttributeValue => {
                if c.is_ascii_whitespace() {
                    // Skip
                } else if c == '"' {
                    self.state = TokenizerState::AttributeValueDoubleQuoted;
                } else if c == '\'' {
                    self.state = TokenizerState::AttributeValueSingleQuoted;
                } else if c == '>' {
                    self.flush_pending_attribute()?;
                    self.finish_current_tag(out_tokens, budget)?;
                } else {
                    self.push_attr_val_char(c)?;
                    self.state = TokenizerState::AttributeValueUnquoted;
                }
                Ok(true)
            }

            TokenizerState::AttributeValueDoubleQuoted => {
                if c == '"' {
                    self.flush_pending_attribute()?;
                    self.state = TokenizerState::AfterAttributeValueQuoted;
                } else if c == '&' {
                    self.pending_entity.clear();
                    self.state = TokenizerState::AttrEntityDoubleQuoted;
                } else {
                    self.push_attr_val_char(c)?;
                }
                Ok(true)
            }

            TokenizerState::AttributeValueSingleQuoted => {
                if c == '\'' {
                    self.flush_pending_attribute()?;
                    self.state = TokenizerState::AfterAttributeValueQuoted;
                } else if c == '&' {
                    self.pending_entity.clear();
                    self.state = TokenizerState::AttrEntitySingleQuoted;
                } else {
                    self.push_attr_val_char(c)?;
                }
                Ok(true)
            }

            TokenizerState::AttributeValueUnquoted => {
                if c.is_ascii_whitespace() {
                    self.flush_pending_attribute()?;
                    self.state = TokenizerState::BeforeAttributeName;
                } else if c == '>' {
                    self.flush_pending_attribute()?;
                    self.finish_current_tag(out_tokens, budget)?;
                } else if c == '&' {
                    self.pending_entity.clear();
                    self.state = TokenizerState::AttrEntityUnquoted;
                } else {
                    self.push_attr_val_char(c)?;
                }
                Ok(true)
            }

            TokenizerState::AttrEntityDoubleQuoted => {
                self.handle_attr_entity(c, TokenizerState::AttributeValueDoubleQuoted)
            }

            TokenizerState::AttrEntitySingleQuoted => {
                self.handle_attr_entity(c, TokenizerState::AttributeValueSingleQuoted)
            }

            TokenizerState::AttrEntityUnquoted => {
                self.handle_attr_entity(c, TokenizerState::AttributeValueUnquoted)
            }

            TokenizerState::AfterAttributeValueQuoted => {
                if c.is_ascii_whitespace() {
                    self.state = TokenizerState::BeforeAttributeName;
                } else if c == '/' {
                    self.state = TokenizerState::SelfClosingStartTag;
                } else if c == '>' {
                    self.finish_current_tag(out_tokens, budget)?;
                } else {
                    self.state = TokenizerState::BeforeAttributeName;
                }
                Ok(true)
            }

            TokenizerState::SelfClosingStartTag => {
                if c == '>' {
                    self.pending_self_closing = true;
                    self.finish_current_tag(out_tokens, budget)?;
                } else if c.is_ascii_whitespace() {
                    // Skip
                } else {
                    self.state = TokenizerState::BeforeAttributeName;
                }
                Ok(true)
            }

            TokenizerState::MarkupDeclarationOpen => {
                self.markup_decl_buf.push(c);
                if self.markup_decl_buf == "--" {
                    self.pending_comment.clear();
                    self.state = TokenizerState::Comment;
                } else if self.markup_decl_buf.eq_ignore_ascii_case("DOCTYPE") {
                    self.pending_tag_name.clear();
                    self.state = TokenizerState::Doctype;
                } else if self.markup_decl_buf.len() >= 7 {
                    self.pending_comment.clear();
                    self.pending_comment.push_str(&self.markup_decl_buf);
                    self.state = TokenizerState::Comment;
                }
                Ok(true)
            }

            TokenizerState::Doctype => {
                if c == '>' {
                    let name = core::mem::take(&mut self.pending_tag_name).trim().to_ascii_lowercase();
                    self.emit_token(HtmlToken::Doctype { name }, out_tokens, budget)?;
                    self.state = TokenizerState::Data;
                } else {
                    if self.pending_tag_name.len() < MAX_ATTR_NAME_LEN {
                        self.pending_tag_name.push(c);
                    }
                }
                Ok(true)
            }

            TokenizerState::Comment => {
                if c == '-' {
                    self.state = TokenizerState::CommentEndDash;
                } else {
                    if self.pending_comment.len() < MAX_ATTR_VALUE_LEN * 4 {
                        self.pending_comment.push(c);
                    }
                }
                Ok(true)
            }

            TokenizerState::CommentEndDash => {
                if c == '-' {
                    self.state = TokenizerState::CommentEnd;
                } else {
                    self.pending_comment.push('-');
                    self.pending_comment.push(c);
                    self.state = TokenizerState::Comment;
                }
                Ok(true)
            }

            TokenizerState::CommentEnd => {
                if c == '>' {
                    let comment_text = core::mem::take(&mut self.pending_comment);
                    self.emit_token(HtmlToken::Comment(comment_text), out_tokens, budget)?;
                    self.state = TokenizerState::Data;
                } else if c == '-' {
                    self.pending_comment.push('-');
                } else {
                    self.pending_comment.push_str("--");
                    self.pending_comment.push(c);
                    self.state = TokenizerState::Comment;
                }
                Ok(true)
            }

            TokenizerState::RawText => {
                if c == '<' {
                    self.raw_text_closing_buf.clear();
                    self.raw_text_closing_buf.push('<');
                    self.state = TokenizerState::RawTextEndTagOpen;
                } else {
                    self.emit_token(HtmlToken::Character(c), out_tokens, budget)?;
                }
                Ok(true)
            }

            TokenizerState::RawTextEndTagOpen => {
                if c == '/' {
                    self.raw_text_closing_buf.push('/');
                    self.state = TokenizerState::RawTextEndTagName;
                    Ok(true)
                } else {
                    for bc in self.raw_text_closing_buf.clone().chars() {
                        self.emit_token(HtmlToken::Character(bc), out_tokens, budget)?;
                    }
                    self.raw_text_closing_buf.clear();
                    self.state = TokenizerState::RawText;
                    Ok(false)
                }
            }

            TokenizerState::RawTextEndTagName => {
                if c == '>' {
                    let tag_candidate = &self.raw_text_closing_buf[2..];
                    if tag_candidate.trim().eq_ignore_ascii_case(&self.raw_text_tag) {
                        let end_tag = core::mem::take(&mut self.raw_text_tag);
                        self.emit_token(HtmlToken::EndTag { name: end_tag }, out_tokens, budget)?;
                        self.raw_text_closing_buf.clear();
                        self.state = TokenizerState::Data;
                    } else {
                        for bc in self.raw_text_closing_buf.clone().chars() {
                            self.emit_token(HtmlToken::Character(bc), out_tokens, budget)?;
                        }
                        self.emit_token(HtmlToken::Character('>'), out_tokens, budget)?;
                        self.raw_text_closing_buf.clear();
                        self.state = TokenizerState::RawText;
                    }
                    Ok(true)
                } else if c.is_ascii_alphabetic() || c.is_ascii_whitespace() {
                    self.raw_text_closing_buf.push(c);
                    Ok(true)
                } else {
                    for bc in self.raw_text_closing_buf.clone().chars() {
                        self.emit_token(HtmlToken::Character(bc), out_tokens, budget)?;
                    }
                    self.raw_text_closing_buf.clear();
                    self.state = TokenizerState::RawText;
                    Ok(false)
                }
            }
        }
    }

    fn handle_attr_entity(
        &mut self,
        c: char,
        return_state: TokenizerState,
    ) -> Result<bool, ParseError> {
        if c == ';' {
            if let Some(resolved) = resolve_entity(&self.pending_entity) {
                self.push_attr_val_char(resolved)?;
            } else {
                self.push_attr_val_char('&')?;
                for ec in self.pending_entity.clone().chars() {
                    self.push_attr_val_char(ec)?;
                }
                self.push_attr_val_char(';')?;
            }
            self.pending_entity.clear();
            self.state = return_state;
            Ok(true)
        } else if c.is_alphanumeric() || c == '#' {
            if self.pending_entity.len() < 16 {
                self.pending_entity.push(c);
                Ok(true)
            } else {
                self.push_attr_val_char('&')?;
                for ec in self.pending_entity.clone().chars() {
                    self.push_attr_val_char(ec)?;
                }
                self.pending_entity.clear();
                self.state = return_state;
                Ok(false)
            }
        } else {
            self.push_attr_val_char('&')?;
            for ec in self.pending_entity.clone().chars() {
                self.push_attr_val_char(ec)?;
            }
            self.pending_entity.clear();
            self.state = return_state;
            Ok(false)
        }
    }

    /// Signals end-of-stream. Flushes any trailing buffered state as tokens.
    pub fn finish(
        &mut self,
        out_tokens: &mut Vec<HtmlToken>,
        budget: &mut WorkBudgetTracker,
    ) -> Result<(), ParseError> {
        match self.state {
            TokenizerState::CharacterReference => {
                self.emit_token(HtmlToken::Character('&'), out_tokens, budget)?;
                for ec in self.pending_entity.clone().chars() {
                    self.emit_token(HtmlToken::Character(ec), out_tokens, budget)?;
                }
                self.pending_entity.clear();
            }
            TokenizerState::Comment | TokenizerState::CommentEndDash | TokenizerState::CommentEnd => {
                let comment_text = core::mem::take(&mut self.pending_comment);
                self.emit_token(HtmlToken::Comment(comment_text), out_tokens, budget)?;
            }
            TokenizerState::RawText | TokenizerState::RawTextEndTagOpen | TokenizerState::RawTextEndTagName => {
                let closing = core::mem::take(&mut self.raw_text_closing_buf);
                for bc in closing.chars() {
                    self.emit_token(HtmlToken::Character(bc), out_tokens, budget)?;
                }
            }
            TokenizerState::TagName | TokenizerState::BeforeAttributeName | TokenizerState::AttributeName
            | TokenizerState::AfterAttributeName | TokenizerState::BeforeAttributeValue
            | TokenizerState::AttributeValueDoubleQuoted | TokenizerState::AttributeValueSingleQuoted
            | TokenizerState::AttributeValueUnquoted | TokenizerState::AfterAttributeValueQuoted
            | TokenizerState::SelfClosingStartTag => {
                let _ = self.flush_pending_attribute();
                let _ = self.finish_current_tag(out_tokens, budget);
            }
            _ => {}
        }

        self.emit_token(HtmlToken::EndOfFile, out_tokens, budget)?;
        Ok(())
    }
}

fn resolve_entity(entity: &str) -> Option<char> {
    match entity {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        s if s.starts_with("#x") || s.starts_with("#X") => {
            u32::from_str_radix(&s[2..], 16).ok().and_then(core::char::from_u32)
        }
        s if s.starts_with('#') => {
            u32::from_str_radix(&s[1..], 10).ok().and_then(core::char::from_u32)
        }
        _ => None,
    }
}
