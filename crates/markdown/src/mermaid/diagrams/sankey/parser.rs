//! Pele's `diagrams/sankey/parser.ts`.

use super::{
    db::SankeyModel,
    lexer::{
        COMMA, DQUOTE, END, EOF, ESCAPED_TEXT, NEWLINE, NON_ESCAPED_TEXT, SANKEY, STUCK,
        TOKEN_NAMES, Tokens, tokenize,
    },
};
use crate::mermaid::{
    detect::Kind,
    error::Error,
    js::{self, Lazy},
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

struct Parser<'t, 's> {
    tokens: &'t Tokens<'s>,
    src: &'s str,
    i: usize,
}

impl<'s> Parser<'_, 's> {
    fn fail(&self, expected: &str) -> Error {
        let t = self.tokens.types[self.i];
        let got = if t == STUCK {
            self.i.checked_sub(1).map_or("undefined", |at| {
                TOKEN_NAMES[self.tokens.types[at] as usize]
            })
        } else {
            TOKEN_NAMES[t as usize]
        };
        Error::at(
            Kind::Sankey,
            self.src,
            self.tokens.starts[self.i],
            &format!("Expecting {expected}, got '{got}'"),
            false,
        )
    }

    fn expect(&mut self, kind: u8, also: &str) -> Result<(), Error> {
        if self.tokens.types[self.i] != kind {
            return Err(self.fail(&format!("'{}'{also}", TOKEN_NAMES[kind as usize])));
        }
        self.i += 1;
        Ok(())
    }

    fn field(&mut self) -> Result<&'s str, Error> {
        let types = &self.tokens.types;
        if types[self.i] == NON_ESCAPED_TEXT {
            self.i += 1;
            return Ok(self.tokens.texts[self.i - 1]);
        }
        if types[self.i] != DQUOTE {
            return Err(self.fail("'DQUOTE', 'NON_ESCAPED_TEXT'"));
        }
        self.i += 1;
        if types[self.i] != ESCAPED_TEXT {
            return Err(self.fail("'ESCAPED_TEXT'"));
        }
        let text = self.tokens.texts[self.i];
        self.i += 1;
        self.expect(DQUOTE, "")?;
        Ok(text)
    }

    fn name(&mut self) -> Result<String, Error> {
        Ok(js::trim(self.field()?).replace("\"\"", "\""))
    }
}

pub fn parse_sankey(src: &str, db: &mut SankeyModel) -> Result<(), Error> {
    let tokens = tokenize(src);
    let mut r = Parser {
        tokens: &tokens,
        src,
        i: 0,
    };

    r.expect(SANKEY, "")?;
    r.expect(NEWLINE, "")?;
    loop {
        let from = r.name()?;
        r.expect(COMMA, ", 'NEWLINE', 'EOF'")?;
        let to = r.name()?;
        r.expect(COMMA, ", 'NEWLINE', 'EOF'")?;
        let amount = r.field()?;
        let source = db.find_or_create_node(&from);
        let target = db.find_or_create_node(&to);
        db.add_link(source, target, js::parse_float(js::trim(amount)));
        if tokens.types[r.i] != NEWLINE {
            break;
        }
        r.i += 1;
    }
    if tokens.types[r.i] == EOF {
        r.i += 1;
    }
    if tokens.types[r.i] != END {
        return Err(r.fail("'NEWLINE', 'EOF'"));
    }
    Ok(())
}
