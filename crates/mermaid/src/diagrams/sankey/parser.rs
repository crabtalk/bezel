//! Pele's `diagrams/sankey/parser.ts`.

use super::{
    db::SankeyModel,
    lexer::{T, tokenize},
};
use crate::{
    detect::Kind,
    error::Error,
    js::{self, Lazy},
    token::Stream,
};

static RE_LINE_BREAKS: Lazy = Lazy::new(r"[\n\r]+", "g");

fn is_blank(c: char) -> bool {
    let c = c as u32;
    if c == 10 || c == 13 {
        return false;
    }
    if c < 128 {
        return c == 32 || c == 9 || c == 11 || c == 12;
    }
    c == 0xa0
        || c == 0x1680
        || (0x2000..=0x200a).contains(&c)
        || c == 0x2028
        || c == 0x2029
        || c == 0x202f
        || c == 0x205f
        || c == 0x3000
        || c == 0xfeff
}

/// Mermaid's sankeyUtils: strip blanks at both ends of the text, then
/// collapse runs of line breaks so that no row is empty.
pub fn prepare_text_for_parsing(text: &str) -> String {
    let text = text.trim_matches(is_blank);
    js::trim(&RE_LINE_BREAKS.replace(text, |_| "\n".into())).to_string()
}

pub fn parse_sankey(src: &str, db: &mut SankeyModel) -> Result<(), Error> {
    let tokens = tokenize(src);
    let mut s = Stream::new(Kind::Sankey, src, &tokens);

    s.expect(T::Sankey, "")?;
    s.expect(T::Newline, "")?;
    loop {
        let from = name(&mut s)?;
        s.expect(T::Comma, "'NEWLINE', 'EOF'")?;
        let to = name(&mut s)?;
        s.expect(T::Comma, "'NEWLINE', 'EOF'")?;
        let amount = field(&mut s)?;
        let source = db.find_or_create_node(&from);
        let target = db.find_or_create_node(&to);
        db.add_link(source, target, js::parse_float(js::trim(amount)));
        if !s.eat(T::Newline) {
            break;
        }
    }
    s.eat(T::Eof);
    if !s.is(T::End) {
        return Err(s.fail("'NEWLINE', 'EOF'"));
    }
    Ok(())
}

fn field<'t>(s: &mut Stream<'t, '_, T>) -> Result<&'t str, Error> {
    if s.is(T::NonEscapedText) {
        return Ok(s.take());
    }
    if !s.eat(T::Dquote) {
        return Err(s.fail("'DQUOTE', 'NON_ESCAPED_TEXT'"));
    }
    let text = s.expect(T::EscapedText, "")?;
    s.expect(T::Dquote, "")?;
    Ok(text)
}

fn name(s: &mut Stream<'_, '_, T>) -> Result<String, Error> {
    Ok(js::trim(field(s)?).replace("\"\"", "\""))
}
