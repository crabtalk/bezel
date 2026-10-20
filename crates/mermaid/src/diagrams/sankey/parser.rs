//! Pele's `diagrams/sankey/parser.ts`.

use super::{
    db::SankeyModel,
    lexer::{T, tokenize},
};
use crate::{detect::Kind, error::Error, js, token::Stream, util::chars::is_inline_space};

/// Mermaid's sankeyUtils: strip blanks at both ends of the text, then
/// collapse runs of line breaks so that no row is empty.
pub fn prepare_text_for_parsing(text: &str) -> String {
    let text = text.trim_matches(|c: char| is_inline_space(c));
    let rows: Vec<&str> = text
        .split(['\n', '\r'])
        .filter(|row| !row.is_empty())
        .collect();
    js::trim(&rows.join("\n")).to_string()
}

pub fn parse_sankey(src: &str, db: &mut SankeyModel) -> Result<(), Error> {
    let tokens = tokenize(src);
    let mut p = Parser {
        s: Stream::new(Kind::Sankey, src, &tokens),
    };

    p.s.expect(T::Sankey, "")?;
    p.s.expect(T::Newline, "")?;
    loop {
        let from = p.name()?;
        p.s.expect(T::Comma, "'NEWLINE', 'EOF'")?;
        let to = p.name()?;
        p.s.expect(T::Comma, "'NEWLINE', 'EOF'")?;
        let amount = p.field()?;
        let source = db.find_or_create_node(&from);
        let target = db.find_or_create_node(&to);
        db.add_link(source, target, js::parse_float(js::trim(amount)));
        if !p.s.eat(T::Newline) {
            break;
        }
    }
    p.s.eat(T::Eof);
    if !p.s.is(T::End) {
        return Err(p.s.fail("'NEWLINE', 'EOF'"));
    }
    Ok(())
}

struct Parser<'t, 's> {
    s: Stream<'t, 's, T>,
}

impl<'t> Parser<'t, '_> {
    fn field(&mut self) -> Result<&'t str, Error> {
        if self.s.is(T::NonEscapedText) {
            return Ok(self.s.take());
        }
        if !self.s.eat(T::Dquote) {
            return Err(self.s.fail("'DQUOTE', 'NON_ESCAPED_TEXT'"));
        }
        let text = self.s.expect(T::EscapedText, "")?;
        self.s.expect(T::Dquote, "")?;
        Ok(text)
    }

    fn name(&mut self) -> Result<String, Error> {
        Ok(js::trim(self.field()?).replace("\"\"", "\""))
    }
}
