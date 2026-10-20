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
