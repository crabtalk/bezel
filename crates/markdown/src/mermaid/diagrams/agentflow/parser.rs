//! Pele's `diagrams/agentflow/parser.ts`.

use indexmap::IndexMap;

use super::{
    db::AgentflowDb,
    lexer::{Tokens, tokenize},
    tokens::{T, TOKEN_NAMES},
    types::{DocItem, FlowText, LabelType, LinkInfo, Loc},
};
use crate::mermaid::{
    detect::Kind,
    error::Error,
    js::{self, Lazy},
};

const ID: [usize; 11] = [
    T::NUM,
    T::NODE_STRING,
    T::DOWN,
    T::MINUS,
    T::DEFAULT,
    T::COMMA,
    T::COLON,
    T::AMP,
    T::BRKT,
    T::MULT,
    T::UNICODE_TEXT,
];
const ALPHANUM: [usize; 11] = [
    T::NUM,
    T::UNICODE_TEXT,
    T::NODE_STRING,
    T::DIR,
    T::DOWN,
    T::MINUS,
    T::COMMA,
    T::COLON,
    T::AMP,
    T::BRKT,
    T::MULT,
];
const NO_TAGS: [usize; 23] = [
    T::NUM,
    T::NODE_STRING,
    T::SPACE,
    T::MINUS,
    T::AMP,
    T::UNICODE_TEXT,
    T::COLON,
    T::MULT,
    T::BRKT,
    T::STYLE,
    T::LINKSTYLE,
    T::CLASSDEF,
    T::CLASS,
    T::CLICK,
    T::GRAPH,
    T::DIR,
    T::FLOW,
    T::CONNECTOR,
    T::GLOBAL,
    T::END_KW,
    T::DOWN,
    T::UP,
    T::START_LINK,
];
const STYLE_PART: [usize; 6] = [
    T::NUM,
    T::NODE_STRING,
    T::COLON,
    T::SPACE,
    T::BRKT,
    T::STYLE,
];
const SEPARATOR: [usize; 4] = [T::NEWLINE, T::SEMI, T::EOF, T::COMMENT];
const TEXT_TOKENS: &str = "'UNICODE_TEXT', 'TEXT', 'TAGSTART'";
const SEPARATORS: &str = "'COMMENT', 'SEMI', 'NEWLINE', 'EOF'";
const DIRECTIONS: [&str; 5] = ["TB", "BT", "RL", "LR", "TD"];
const STATEMENT_START: &str = "'COMMENT', 'SEMI', 'NEWLINE', 'SPACE', 'EOF', 'flow', 'global', 'connector', 'acc_title', 'acc_descr', 'acc_descr_multiline_value', 'STYLE', 'LINKSTYLE', 'CLASSDEF', 'CLASS', 'CLICK', 'NODE_STRING'";

/// Mermaid trims spaces and tabs between a closing brace and the end of its
/// line before parsing.
static RE_BRACE_TRAIL: Lazy = Lazy::new(r"}[^\S\n]*\n", "g");

struct Frame {
    global: bool,
    /// Token index of the `flow` keyword.
    at: usize,
    id: Option<FlowText>,
    title: Option<FlowText>,
    data: Option<String>,
    data_from: usize,
    data_to: usize,
    list: Vec<DocItem>,
    collect: bool,
}

struct Parser<'a> {
    src: &'a str,
    tokens: &'a Tokens,
    i: usize,
    pending_data: Option<String>,
    data_from: usize,
    data_to: usize,
    db: &'a mut AgentflowDb,
}

impl Parser<'_> {
    fn t(&self) -> usize {
        self.tokens.types.get(self.i).copied().unwrap_or(T::END)
    }

    fn take(&mut self) -> String {
        let text = self.tokens.texts[self.i].clone();
        self.i += 1;
        text
    }

    fn fail(&self, expected: &str) -> Error {
        let t = self.t();
        let start = self
            .tokens
            .starts
            .get(self.i)
            .copied()
            .unwrap_or(self.src.len());
        if t == T::ERROR {
            return Error::at(Kind::Agentflow, self.src, start, "", true);
        }
        Error::at(
            Kind::Agentflow,
            self.src,
            start,
            &format!("Expecting {expected}, got '{}'", TOKEN_NAMES[t]),
            false,
        )
    }

    /// The span from the start of token `a` to the end of token `b`.
    fn span(&self, a: usize, b: usize) -> Option<Loc> {
        let locs = &self.tokens.locs;
        Some(Loc {
            first_line: locs[4 * a],
            first_column: locs[4 * a + 1],
            last_line: locs[4 * b + 2],
            last_column: locs[4 * b + 3],
        })
    }

    fn expect(&mut self, kind: usize, also: &str) -> Result<String, Error> {
        if self.t() != kind {
            let also = if also.is_empty() {
                String::new()
            } else {
                format!(", {also}")
            };
            return Err(self.fail(&format!("'{}'{also}", TOKEN_NAMES[kind])));
        }
        Ok(self.take())
    }

    fn separator(&mut self) -> Result<(), Error> {
        if !SEPARATOR.contains(&self.t()) {
            return Err(self.fail(SEPARATORS));
        }
        self.i += 1;
        Ok(())
    }

    fn run_of(&mut self, set: &[usize], expected: &str) -> Result<String, Error> {
        if !set.contains(&self.t()) {
            return Err(self.fail(expected));
        }
        let mut s = self.take();
        while set.contains(&self.t()) {
            s += &self.take();
        }
        Ok(s)
    }

    fn id_string(&mut self) -> Result<String, Error> {
        self.run_of(&ID, "'NODE_STRING'")
    }

    fn alpha_num(&mut self) -> Result<String, Error> {
        self.run_of(&ALPHANUM, "'NODE_STRING'")
    }

    fn text(&mut self) -> Result<FlowText, Error> {
        let t = self.t();
        let kind = if t == T::STR {
            LabelType::String
        } else if t == T::MD_STR {
            LabelType::Markdown
        } else if t == T::TEXT || t == T::TAGSTART || t == T::TAGEND || t == T::UNICODE_TEXT {
            LabelType::Text
        } else {
            return Err(self.fail(&format!("{TEXT_TOKENS}, 'TAGEND', 'STR', 'MD_STR'")));
        };
        let mut s = self.take();
        loop {
            let n = self.t();
            if n != T::TEXT && n != T::TAGSTART && n != T::TAGEND && n != T::UNICODE_TEXT {
                break;
            }
            s += &self.take();
        }
        Ok(FlowText { text: s, kind })
    }

    fn closed(&mut self, kind: usize) -> Result<(), Error> {
        self.expect(kind, TEXT_TOKENS).map(|_| ())
    }

    fn edge_text(&mut self) -> Result<FlowText, Error> {
        let t = self.t();
        let kind = if t == T::STR {
            LabelType::String
        } else if t == T::MD_STR {
            LabelType::Markdown
        } else if t == T::EDGE_TEXT || t == T::UNICODE_TEXT {
            LabelType::Text
        } else {
            return Err(self.fail("'EDGE_TEXT', 'STR', 'MD_STR'"));
        };
        let mut s = self.take();
        while self.t() == T::EDGE_TEXT || self.t() == T::UNICODE_TEXT {
            s += &self.take();
        }
        Ok(FlowText { text: s, kind })
    }

    fn text_no_tags(&mut self) -> Result<FlowText, Error> {
        let t = self.t();
        let kind = if t == T::STR {
            LabelType::Text
        } else if t == T::MD_STR {
            LabelType::Markdown
        } else if NO_TAGS.contains(&t) {
            LabelType::Text
        } else {
            return Err(self.fail("'NODE_STRING', 'STR', 'MD_STR'"));
        };
        let mut s = self.take();
        while NO_TAGS.contains(&self.t()) {
            s += &self.take();
        }
        Ok(FlowText { text: s, kind })
    }

    fn shape_data(&mut self) -> String {
        self.data_from = self.i;
        let mut s = self.take();
        while self.t() == T::SHAPE_DATA {
            s += &self.take();
        }
        self.data_to = self.i - 1;
        s
    }

    fn attach(&mut self, id: &str, data: &str, from: usize, to: usize) -> Result<(), Error> {
        let loc = self.span(from, to);
        self.db
            .add_vertex(id, None, None, None, None, None, None, Some(data), loc)?;
        self.db.extend_vertex_mapping(id, loc);
        Ok(())
    }

    fn attach_data(&mut self, id: &str, frame: &Frame) -> Result<(), Error> {
        let Some(data) = &frame.data else {
            return Ok(());
        };
        let loc = self.span(frame.data_from, frame.data_to);
        self.db
            .add_vertex(id, None, None, None, None, None, None, Some(data), loc)
    }

    fn vertex(&mut self) -> Result<String, Error> {
        let from = self.i;
        let id = self.id_string()?;
        let mut label: Option<FlowText> = None;
        let mut kind: Option<&str> = None;
        let mut props: Option<IndexMap<String, String>> = None;
        match self.t() {
            T::SQS => {
                self.i += 1;
                label = Some(self.text()?);
                self.closed(T::SQE)?;
                kind = Some("square");
            }
            T::DOUBLECIRCLESTART => {
                self.i += 1;
                label = Some(self.text()?);
                self.closed(T::DOUBLECIRCLEEND)?;
                kind = Some("doublecircle");
            }
            T::PS => {
                self.i += 1;
                if self.t() == T::PS {
                    self.i += 1;
                    label = Some(self.text()?);
                    self.closed(T::PE)?;
                    self.expect(T::PE, "")?;
                    kind = Some("circle");
                } else {
                    label = Some(self.text()?);
                    self.closed(T::PE)?;
                    kind = Some("round");
                }
            }
            T::ELLIPSE_START => {
                self.i += 1;
                label = Some(self.text()?);
                self.closed(T::ELLIPSE_END)?;
                kind = Some("ellipse");
            }
            T::STADIUMSTART => {
                self.i += 1;
                label = Some(self.text()?);
                self.closed(T::STADIUMEND)?;
                kind = Some("stadium");
            }
            T::SUBROUTINESTART => {
                self.i += 1;
                label = Some(self.text()?);
                self.closed(T::SUBROUTINEEND)?;
                kind = Some("subroutine");
            }
            T::VERTEX_WITH_PROPS_START => {
                self.i += 1;
                let field = self.expect(T::NODE_STRING, "")?;
                self.expect(T::COLON, "")?;
                let value = self.expect(T::NODE_STRING, "")?;
                self.expect(T::PIPE, "")?;
                label = Some(self.text()?);
                self.closed(T::SQE)?;
                kind = Some("rect");
                let mut map = IndexMap::new();
                map.insert(field, value);
                props = Some(map);
            }
            T::CYLINDERSTART => {
                self.i += 1;
                label = Some(self.text()?);
                self.closed(T::CYLINDEREND)?;
                kind = Some("cylinder");
            }
            T::DIAMOND_START => {
                self.i += 1;
                if self.t() == T::DIAMOND_START {
                    self.i += 1;
                    label = Some(self.text()?);
                    self.closed(T::DIAMOND_STOP)?;
                    self.expect(T::DIAMOND_STOP, "")?;
                    kind = Some("hexagon");
                } else {
                    label = Some(self.text()?);
                    self.closed(T::DIAMOND_STOP)?;
                    kind = Some("diamond");
                }
            }
            T::TAGEND => {
                self.i += 1;
                label = Some(self.text()?);
                self.closed(T::SQE)?;
                kind = Some("odd");
            }
            T::TRAPSTART => {
                self.i += 1;
                label = Some(self.text()?);
                if self.t() == T::INVTRAPEND {
                    self.i += 1;
                    kind = Some("lean_right");
                } else {
                    self.closed(T::TRAPEND)?;
                    kind = Some("trapezoid");
                }
            }
            T::INVTRAPSTART => {
                self.i += 1;
                label = Some(self.text()?);
                if self.t() == T::TRAPEND {
                    self.i += 1;
                    kind = Some("lean_left");
                } else {
                    self.closed(T::INVTRAPEND)?;
                    kind = Some("inv_trapezoid");
                }
            }
            _ => {}
        }
        if props.is_some() || kind.is_some() {
            self.db.add_vertex(
                &id,
                label.as_ref(),
                kind,
                None,
                None,
                None,
                props,
                None,
                None,
            )?;
        } else {
            self.db
                .add_vertex(&id, None, None, None, None, None, None, None, None)?;
        }
        let loc = self.span(from, self.i - 1);
        self.db.add_vertex_mapping(&id, kind.is_none(), loc);
        Ok(id)
    }

    fn styled_vertex(&mut self) -> Result<String, Error> {
        let id = self.vertex()?;
        if self.t() == T::STYLE_SEPARATOR {
            self.i += 1;
            let class = self.id_string()?;
            self.db.set_class(&id, &class);
        }
        Ok(id)
    }

    fn node(&mut self) -> Result<Vec<String>, Error> {
        let mut ids = vec![self.styled_vertex()?];
        self.pending_data = None;
        loop {
            let data = (self.t() == T::SHAPE_DATA).then(|| self.shape_data());
            let from = self.data_from;
            let to = self.data_to;
            if self.t() != T::SPACE {
                self.pending_data = data;
                return Ok(ids);
            }
            self.i += 1;
            while self.t() == T::SPACE {
                self.i += 1;
            }
            if self.t() != T::AMP {
                if data.is_some() {
                    return Err(self.fail("'AMP'"));
                }
                return Ok(ids);
            }
            self.i += 1;
            if self.t() != T::SPACE {
                return Err(self.fail("'SPACE'"));
            }
            self.i += 1;
            while self.t() == T::SPACE {
                self.i += 1;
            }
            let next = self.styled_vertex()?;
            if let Some(data) = data {
                let last = ids[ids.len() - 1].clone();
                self.attach(&last, &data, from, to)?;
            }
            ids.push(next);
        }
    }

    fn link(&mut self) -> Result<LinkInfo, Error> {
        let mut id = None;
        if self.t() == T::LINK_ID {
            id = Some(self.take());
        }
        if self.t() == T::LINK {
            let text = self.take();
            let mut info = self.db.destruct_link(&text, None);
            if id.is_some() {
                info.id = id;
            }
            if self.t() == T::PIPE {
                self.i += 1;
                info.text = Some(self.text()?);
                self.closed(T::PIPE)?;
                if self.t() == T::SPACE {
                    self.i += 1;
                }
            }
            return Ok(info);
        }
        if self.t() == T::START_LINK {
            let start = self.take();
            let label = self.edge_text()?;
            let end = self.expect(T::LINK, "'EDGE_TEXT'")?;
            let mut info = self.db.destruct_link(&end, Some(&start));
            info.text = Some(label);
            if id.is_some() {
                info.id = id;
            }
            return Ok(info);
        }
        Err(self.fail("'LINK', 'START_LINK'"))
    }

    fn attach_pending(&mut self, ids: &[String]) -> Result<(), Error> {
        if let Some(data) = self.pending_data.clone() {
            let last = ids[ids.len() - 1].clone();
            self.attach(&last, &data, self.data_from, self.data_to)?;
        }
        Ok(())
    }

    fn vertex_statement(&mut self) -> Result<Vec<String>, Error> {
        let from = self.i;
        let mut stmt = self.node()?;
        self.attach_pending(&stmt)?;
        let mut nodes = stmt.clone();
        loop {
            let t = self.t();
            if t != T::LINK && t != T::START_LINK && t != T::LINK_ID {
                return Ok(nodes);
            }
            let info = self.link()?;
            let next = self.node()?;
            self.attach_pending(&next)?;
            self.db.add_link(&stmt, &next, Some(&info))?;
            let loc = self.span(from, self.i - 1);
            self.db.add_edge_mapping(&next, loc);
            let mut joined = next.clone();
            joined.extend(nodes);
            nodes = joined;
            stmt = next;
        }
    }

    fn style(&mut self) -> Result<String, Error> {
        self.run_of(&STYLE_PART, "'NODE_STRING', 'NUM', 'COLON', 'BRKT'")
    }

    fn styles_opt(&mut self) -> Result<Vec<String>, Error> {
        let mut out = vec![self.style()?];
        while self.t() == T::COMMA {
            self.i += 1;
            out.push(self.style()?);
        }
        Ok(out)
    }

    fn link_style_statement(&mut self) -> Result<(), Error> {
        self.i += 1;
        self.expect(T::SPACE, "")?;
        let mut positions;
        if self.t() == T::DEFAULT {
            positions = vec![self.take()];
        } else {
            positions = vec![self.expect(T::NUM, "'DEFAULT'")?];
            while self.t() == T::COMMA {
                self.i += 1;
                positions.push(self.expect(T::NUM, "")?);
            }
        }
        self.expect(T::SPACE, "")?;
        if self.t() != T::INTERPOLATE {
            let styles = self.styles_opt()?;
            return self.db.update_link(&positions, styles);
        }
        self.i += 1;
        self.expect(T::SPACE, "")?;
        let interpolate = self.alpha_num()?;
        if self.t() == T::SPACE {
            self.i += 1;
            let styles = self.styles_opt()?;
            self.db.update_link_interpolate(&positions, &interpolate)?;
            self.db.update_link(&positions, styles)
        } else {
            self.db.update_link_interpolate(&positions, &interpolate)
        }
    }

    fn link_tail(&mut self, id: &str, url: &str) -> Result<(), Error> {
        if self.t() != T::SPACE {
            self.db.set_link(id, url, None);
            return Ok(());
        }
        self.i += 1;
        if self.t() == T::LINK_TARGET {
            let target = self.take();
            self.db.set_link(id, url, Some(&target));
            return Ok(());
        }
        let tooltip = self.expect(T::STR, "'LINK_TARGET'")?;
        if self.t() == T::SPACE {
            self.i += 1;
            let target = self.expect(T::LINK_TARGET, "")?;
            self.db.set_link(id, url, Some(&target));
        } else {
            self.db.set_link(id, url, None);
        }
        self.db.set_tooltip(id, Some(&tooltip));
        Ok(())
    }

    fn click_statement(&mut self) -> Result<(), Error> {
        let id = self.take();
        match self.t() {
            T::CALLBACKNAME => {
                self.i += 1;
                if self.t() == T::CALLBACKARGS {
                    self.i += 1;
                }
                let mut tooltip = None;
                if self.t() == T::SPACE {
                    self.i += 1;
                    tooltip = Some(self.expect(T::STR, "")?);
                }
                self.db.set_click_event(&id);
                if tooltip.is_some() {
                    self.db.set_tooltip(&id, tooltip.as_deref());
                }
            }
            T::HREF => {
                self.i += 1;
                let url = self.expect(T::STR, "")?;
                self.link_tail(&id, &url)?;
            }
            T::STR => {
                let url = self.take();
                self.link_tail(&id, &url)?;
            }
            _ => {
                self.alpha_num()?;
                if self.t() == T::SPACE {
                    self.i += 1;
                    let tooltip = self.expect(T::STR, "")?;
                    self.db.set_click_event(&id);
                    self.db.set_tooltip(&id, Some(&tooltip));
                } else {
                    self.db.set_click_event(&id);
                }
            }
        }
        Ok(())
    }

    /// Reads `flow` or `connector` up to the separator: an id, an optional
    /// bracketed title, optional metadata.
    fn header(&mut self, at: usize) -> Result<Frame, Error> {
        self.expect(T::SPACE, SEPARATORS)?;
        let id = self.text_no_tags()?;
        let mut title = FlowText {
            text: String::new(),
            kind: LabelType::Text,
        };
        if self.t() == T::SQS {
            self.i += 1;
            title = self.text()?;
            self.closed(T::SQE)?;
        }
        let data = (self.t() == T::SHAPE_DATA).then(|| self.shape_data());
        if !SEPARATOR.contains(&self.t()) {
            return Err(self.fail(&format!("{SEPARATORS}, 'SHAPE_DATA'")));
        }
        Ok(Frame {
            global: false,
            at,
            id: Some(id),
            title: Some(title),
            data,
            data_from: self.data_from,
            data_to: self.data_to,
            list: Vec::new(),
            collect: false,
        })
    }

    fn statement(&mut self) -> Result<DocItem, Error> {
        let t = self.t();
        let none = DocItem::List(Vec::new());
        match t {
            T::STYLE => {
                self.i += 1;
                self.expect(T::SPACE, "")?;
                let id = self.id_string()?;
                self.expect(T::SPACE, "")?;
                let styles = self.styles_opt()?;
                self.db
                    .add_vertex(&id, None, None, Some(&styles), None, None, None, None, None)?;
                self.separator()?;
                Ok(none)
            }
            T::LINKSTYLE => {
                self.link_style_statement()?;
                self.separator()?;
                Ok(none)
            }
            T::CLASSDEF => {
                self.i += 1;
                self.expect(T::SPACE, "")?;
                let id = self.id_string()?;
                self.expect(T::SPACE, "")?;
                let styles = self.styles_opt()?;
                self.db.add_class(&id, &styles);
                self.separator()?;
                Ok(none)
            }
            T::CLASS => {
                self.i += 1;
                self.expect(T::SPACE, "")?;
                let ids = self.id_string()?;
                self.expect(T::SPACE, "")?;
                let class = self.id_string()?;
                self.db.set_class(&ids, &class);
                self.separator()?;
                Ok(none)
            }
            T::CLICK => {
                self.click_statement()?;
                self.separator()?;
                Ok(none)
            }
            T::CONNECTOR => {
                let at = self.i;
                self.i += 1;
                let frame = self.header(at)?;
                let sep = self.i;
                self.i += 1;
                let id = self
                    .db
                    .add_connector(frame.id.as_ref(), frame.title.as_ref());
                self.attach_data(&id, &frame)?;
                let (start, end) = (self.span(frame.at, frame.at), self.span(sep, sep));
                self.db.add_connector_mapping(frame.id.as_ref(), start, end);
                Ok(DocItem::Str(id))
            }
            T::DIRECTION_TB
            | T::DIRECTION_BT
            | T::DIRECTION_RL
            | T::DIRECTION_LR
            | T::DIRECTION_TD => {
                self.i += 1;
                Ok(DocItem::Dir(DIRECTIONS[t - T::DIRECTION_TB].to_string()))
            }
            T::ACC_TITLE => {
                self.i += 1;
                let value = js::trim(&self.expect(T::ACC_TITLE_VALUE, "")?).to_string();
                self.db.set_acc_title(&value);
                Ok(DocItem::Str(value))
            }
            T::ACC_DESCR => {
                self.i += 1;
                let value = js::trim(&self.expect(T::ACC_DESCR_VALUE, "")?).to_string();
                self.db.set_acc_description(&value);
                Ok(DocItem::Str(value))
            }
            T::ACC_DESCR_MULTILINE_VALUE => {
                let value = js::trim(&self.take()).to_string();
                self.db.set_acc_description(&value);
                Ok(DocItem::Str(value))
            }
            _ => {
                if !ID.contains(&t) {
                    return Err(self.fail(STATEMENT_START));
                }
                let nodes = self.vertex_statement()?;
                self.separator()?;
                Ok(DocItem::List(nodes))
            }
        }
    }

    /// Blocks nest without recursion: each open one keeps the list and
    /// collect flag of its parent.
    fn document(&mut self) -> Result<(), Error> {
        let mut open: Vec<Frame> = Vec::new();
        let mut list: Vec<DocItem> = Vec::new();
        let mut collect = false;
        loop {
            let t = self.t();
            if t == T::END {
                break;
            }
            if t == T::END_KW {
                let Some(frame) = open.pop() else {
                    break;
                };
                let at = self.i;
                self.i += 1;
                if frame.global {
                    self.db.add_global(&list);
                    list = frame.list;
                    collect = frame.collect;
                    continue;
                }
                let id = self
                    .db
                    .add_sub_graph(frame.id.as_ref(), &list, frame.title.as_ref());
                self.attach_data(&id, &frame)?;
                let (start, end) = (self.span(frame.at, frame.at), self.span(at, at));
                self.db.add_subgraph_mapping(frame.id.as_ref(), start, end);
                list = frame.list;
                collect = frame.collect;
                if collect {
                    list.push(DocItem::Str(id));
                }
            } else if t == T::SEMI || t == T::NEWLINE || t == T::SPACE || t == T::EOF {
                if collect {
                    list.push(DocItem::Str(self.tokens.texts[self.i].clone()));
                }
                self.i += 1;
            } else if t == T::COMMENT {
                self.i += 1;
            } else if t == T::FLOW || t == T::GLOBAL {
                let at = self.i;
                self.i += 1;
                let frame = if t == T::GLOBAL || SEPARATOR.contains(&self.t()) {
                    let frame = Frame {
                        global: t == T::GLOBAL,
                        at,
                        id: None,
                        title: None,
                        data: None,
                        data_from: 0,
                        data_to: 0,
                        list: std::mem::take(&mut list),
                        collect,
                    };
                    self.separator()?;
                    frame
                } else {
                    let mut frame = self.header(at)?;
                    frame.list = std::mem::take(&mut list);
                    frame.collect = collect;
                    self.i += 1;
                    frame
                };
                open.push(frame);
                collect = true;
            } else {
                let item = self.statement()?;
                if collect && !matches!(&item, DocItem::List(items) if items.is_empty()) {
                    list.push(item);
                }
            }
        }
        if !open.is_empty() {
            return Err(self.fail("'end'"));
        }
        Ok(())
    }
}

/// `raw` parses the text as it is, the way Mermaid's generated parser does
/// without its wrapper.
pub fn parse_agentflow(source: &str, db: &mut AgentflowDb, raw: bool) -> Result<(), Error> {
    let src = if !raw && source.contains('}') {
        RE_BRACE_TRAIL.replace(source, |_| "}\n".into())
    } else {
        source.to_string()
    };
    let tokens = tokenize(&src);
    let mut p = Parser {
        src: &src,
        tokens: &tokens,
        i: 0,
        pending_data: None,
        data_from: 0,
        data_to: 0,
        db,
    };

    while matches!(p.t(), T::SPACE | T::NEWLINE | T::COMMENT) {
        p.i += 1;
    }
    p.expect(T::GRAPH, "'SPACE', 'NEWLINE', 'COMMENT'")?;
    if p.t() == T::NODIR {
        p.i += 1;
        p.db.set_direction("TB");
    } else {
        let dir = p.expect(T::DIR, "'NODIR'")?;
        let mut t = p.t();
        if t == T::SPACE {
            p.i += 1;
            while p.t() == T::SPACE {
                p.i += 1;
            }
            t = p.t();
            if t != T::NEWLINE {
                return Err(p.fail("'NEWLINE', 'SPACE'"));
            }
        } else if t != T::SEMI && t != T::NEWLINE && t != T::COMMENT {
            return Err(p.fail("'COMMENT', 'SEMI', 'NEWLINE', 'SPACE'"));
        }
        p.i += 1;
        p.db.set_direction(&dir);
    }
    p.document()?;
    if p.t() != T::END {
        return Err(p.fail(STATEMENT_START));
    }
    Ok(())
}
