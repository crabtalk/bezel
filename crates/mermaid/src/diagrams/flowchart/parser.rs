//! Pele's `diagrams/flowchart/parser.ts`.

use indexmap::IndexMap;

use super::{
    db::FlowDb,
    lexer::{Tokens, tokenize},
    tokens::{T, TOKEN_NAMES},
    types::{DocItem, FlowText, LabelType, LinkInfo},
};
use crate::{detect::Kind, error::Error, js::Lazy};

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
const NO_TAGS: [usize; 21] = [
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
    T::SUBGRAPH,
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
const TEXT_TOKENS: &str = "'UNICODE_TEXT', 'TEXT', 'TAGSTART'";
const DIRECTIONS: [&str; 5] = ["TB", "BT", "RL", "LR", "TD"];
const STATEMENT_START: &str = "'SEMI', 'NEWLINE', 'SPACE', 'EOF', 'subgraph', 'acc_title', 'acc_descr', 'acc_descr_multiline_value', 'STYLE', 'LINKSTYLE', 'CLASSDEF', 'CLASS', 'CLICK', 'NODE_STRING'";

/// Mermaid trims whitespace between a closing brace and the end of its line
/// before parsing.
static RE_BRACE_TRAIL: Lazy = Lazy::new(r"}\s*\n", "g");

struct Frame {
    id: Option<FlowText>,
    title: Option<FlowText>,
    /// The title is the id's own text.
    same: bool,
    list: Vec<DocItem>,
    collect: bool,
}

struct Parser<'a> {
    src: &'a str,
    tokens: &'a Tokens,
    i: usize,
    pending_data: Option<String>,
    db: &'a mut FlowDb,
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
            return Error::at(Kind::Flowchart, self.src, start, "", true);
        }
        Error::at(
            Kind::Flowchart,
            self.src,
            start,
            &format!("Expecting {expected}, got '{}'", TOKEN_NAMES[t]),
            false,
        )
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
        let t = self.t();
        if t != T::NEWLINE && t != T::SEMI && t != T::EOF {
            return Err(self.fail("'SEMI', 'NEWLINE', 'EOF'"));
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
        let mut s = self.take();
        while self.t() == T::SHAPE_DATA {
            s += &self.take();
        }
        s
    }

    fn add(&mut self, id: &str, label: &FlowText, kind: &str) -> Result<(), Error> {
        self.db
            .add_vertex(id, Some(label), Some(kind), None, None, None, None, None)
    }

    fn add_data(&mut self, id: &str, data: &str) -> Result<(), Error> {
        self.db
            .add_vertex(id, None, None, None, None, None, None, Some(data))
    }

    fn vertex(&mut self) -> Result<String, Error> {
        let id = self.id_string()?;
        match self.t() {
            T::SQS => {
                self.i += 1;
                let label = self.text()?;
                self.closed(T::SQE)?;
                self.add(&id, &label, "square")?;
            }
            T::DOUBLECIRCLESTART => {
                self.i += 1;
                let label = self.text()?;
                self.closed(T::DOUBLECIRCLEEND)?;
                self.add(&id, &label, "doublecircle")?;
            }
            T::PS => {
                self.i += 1;
                if self.t() == T::PS {
                    self.i += 1;
                    let label = self.text()?;
                    self.closed(T::PE)?;
                    self.expect(T::PE, "")?;
                    self.add(&id, &label, "circle")?;
                } else {
                    let label = self.text()?;
                    self.closed(T::PE)?;
                    self.add(&id, &label, "round")?;
                }
            }
            T::ELLIPSE_START => {
                self.i += 1;
                let label = self.text()?;
                self.closed(T::ELLIPSE_END)?;
                self.add(&id, &label, "ellipse")?;
            }
            T::STADIUMSTART => {
                self.i += 1;
                let label = self.text()?;
                self.closed(T::STADIUMEND)?;
                self.add(&id, &label, "stadium")?;
            }
            T::SUBROUTINESTART => {
                self.i += 1;
                let label = self.text()?;
                self.closed(T::SUBROUTINEEND)?;
                self.add(&id, &label, "subroutine")?;
            }
            T::VERTEX_WITH_PROPS_START => {
                self.i += 1;
                let field = self.expect(T::NODE_STRING, "")?;
                self.expect(T::COLON, "")?;
                let value = self.expect(T::NODE_STRING, "")?;
                self.expect(T::PIPE, "")?;
                let label = self.text()?;
                self.closed(T::SQE)?;
                let mut props = IndexMap::new();
                props.insert(field, value);
                self.db.add_vertex(
                    &id,
                    Some(&label),
                    Some("rect"),
                    None,
                    None,
                    None,
                    Some(props),
                    None,
                )?;
            }
            T::CYLINDERSTART => {
                self.i += 1;
                let label = self.text()?;
                self.closed(T::CYLINDEREND)?;
                self.add(&id, &label, "cylinder")?;
            }
            T::DIAMOND_START => {
                self.i += 1;
                if self.t() == T::DIAMOND_START {
                    self.i += 1;
                    let label = self.text()?;
                    self.closed(T::DIAMOND_STOP)?;
                    self.expect(T::DIAMOND_STOP, "")?;
                    self.add(&id, &label, "hexagon")?;
                } else {
                    let label = self.text()?;
                    self.closed(T::DIAMOND_STOP)?;
                    self.add(&id, &label, "diamond")?;
                }
            }
            T::TAGEND => {
                self.i += 1;
                let label = self.text()?;
                self.closed(T::SQE)?;
                self.add(&id, &label, "odd")?;
            }
            T::TRAPSTART => {
                self.i += 1;
                let label = self.text()?;
                if self.t() == T::INVTRAPEND {
                    self.i += 1;
                    self.add(&id, &label, "lean_right")?;
                } else {
                    self.closed(T::TRAPEND)?;
                    self.add(&id, &label, "trapezoid")?;
                }
            }
            T::INVTRAPSTART => {
                self.i += 1;
                let label = self.text()?;
                if self.t() == T::TRAPEND {
                    self.i += 1;
                    self.add(&id, &label, "lean_left")?;
                } else {
                    self.closed(T::INVTRAPEND)?;
                    self.add(&id, &label, "inv_trapezoid")?;
                }
            }
            _ => self
                .db
                .add_vertex(&id, None, None, None, None, None, None, None)?,
        }
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
                self.add_data(&last, &data)?;
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

    fn vertex_statement(&mut self) -> Result<Vec<String>, Error> {
        let mut stmt = self.node()?;
        if let Some(data) = self.pending_data.take() {
            let last = stmt[stmt.len() - 1].clone();
            self.add_data(&last, &data)?;
        }
        let mut nodes = stmt.clone();
        loop {
            let t = self.t();
            if t != T::LINK && t != T::START_LINK && t != T::LINK_ID {
                return Ok(nodes);
            }
            let info = self.link()?;
            let next = self.node()?;
            if let Some(data) = self.pending_data.take() {
                let last = next[next.len() - 1].clone();
                self.add_data(&last, &data)?;
            }
            self.db.add_link(&stmt, &next, Some(&info))?;
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

    fn subgraph_header(&mut self) -> Result<Frame, Error> {
        self.i += 1;
        let t = self.t();
        if t == T::NEWLINE || t == T::SEMI || t == T::EOF {
            self.i += 1;
            return Ok(Frame {
                id: None,
                title: None,
                same: false,
                list: Vec::new(),
                collect: false,
            });
        }
        self.expect(T::SPACE, "'SEMI', 'NEWLINE', 'EOF'")?;
        let id = self.text_no_tags()?;
        let mut title = None;
        if self.t() == T::SQS {
            self.i += 1;
            title = Some(self.text()?);
            self.closed(T::SQE)?;
        }
        let t = self.t();
        if t != T::NEWLINE && t != T::SEMI && t != T::EOF {
            return Err(self.fail("'SEMI', 'NEWLINE', 'EOF', 'SQS'"));
        }
        self.i += 1;
        let same = title.is_none();
        Ok(Frame {
            title: Some(title.unwrap_or_else(|| id.clone())),
            id: Some(id),
            same,
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
                    .add_vertex(&id, None, None, Some(&styles), None, None, None, None)?;
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
                let value =
                    crate::js::trim(&self.expect(T::ACC_TITLE_VALUE, "")?).to_string();
                self.db.set_acc_title(&value);
                Ok(DocItem::Str(value))
            }
            T::ACC_DESCR => {
                self.i += 1;
                let value =
                    crate::js::trim(&self.expect(T::ACC_DESCR_VALUE, "")?).to_string();
                self.db.set_acc_description(&value);
                Ok(DocItem::Str(value))
            }
            T::ACC_DESCR_MULTILINE_VALUE => {
                let value = crate::js::trim(&self.take()).to_string();
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

    /// Subgraphs nest without recursion: each open one keeps the list and
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
                self.i += 1;
                let id = self.db.add_sub_graph(
                    frame.id.as_ref(),
                    &list,
                    frame.title.as_ref(),
                    frame.same,
                );
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
            } else if t == T::SUBGRAPH {
                let mut frame = self.subgraph_header()?;
                frame.list = std::mem::take(&mut list);
                frame.collect = collect;
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

pub fn parse_flowchart(source: &str, db: &mut FlowDb) -> Result<(), Error> {
    let src = if source.contains('}') {
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
        db,
    };

    while p.t() == T::SPACE || p.t() == T::NEWLINE {
        p.i += 1;
    }
    p.expect(T::GRAPH, "'SPACE', 'NEWLINE'")?;
    if p.t() == T::NODIR {
        p.i += 1;
        p.db.set_direction("TB");
    } else {
        let dir = p.expect(T::DIR, "'NODIR'")?;
        let t = p.t();
        if t == T::SPACE {
            p.i += 1;
            while p.t() == T::SPACE {
                p.i += 1;
            }
            if p.t() != T::NEWLINE {
                return Err(p.fail("'NEWLINE', 'SPACE'"));
            }
        } else if t != T::SEMI && t != T::NEWLINE {
            return Err(p.fail("'SEMI', 'NEWLINE', 'SPACE'"));
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
