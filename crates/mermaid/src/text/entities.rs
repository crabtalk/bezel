//! HTML entity codes and Mermaid's `#name;` placeholders, read as characters.

use std::fmt::Write as _;
use std::{collections::HashMap, sync::OnceLock};

use super::super::js::Lazy;

/// Every entity name Pele reads, with its code point.
const NAMED: &str = concat!(
    "amp:38,lt:60,gt:62,quot:34,apos:39,nbsp:160,iexcl:161,cent:162,pound:163,curren:164,yen:16",
    "5,sect:167,copy:169,laquo:171,not:172,reg:174,deg:176,plusmn:177,sup2:178,sup3:179,micro:1",
    "81,para:182,middot:183,sup1:185,raquo:187,frac14:188,frac12:189,frac34:190,iquest:191,time",
    "s:215,divide:247,ndash:8211,mdash:8212,lsquo:8216,rsquo:8217,sbquo:8218,ldquo:8220,rdquo:8",
    "221,bdquo:8222,dagger:8224,Dagger:8225,bull:8226,hellip:8230,permil:8240,prime:8242,Prime:",
    "8243,lsaquo:8249,rsaquo:8250,euro:8364,trade:8482,larr:8592,uarr:8593,rarr:8594,darr:8595,",
    "harr:8596,lArr:8656,uArr:8657,rArr:8658,dArr:8659,hArr:8660,forall:8704,part:8706,exist:87",
    "07,empty:8709,nabla:8711,isin:8712,notin:8713,ni:8715,prod:8719,sum:8721,minus:8722,radic:",
    "8730,infin:8734,and:8743,or:8744,cap:8745,cup:8746,int:8747,there4:8756,sim:8764,cong:8773",
    ",asymp:8776,ne:8800,equiv:8801,le:8804,ge:8805,sub:8834,sup:8835,sube:8838,supe:8839,oplus",
    ":8853,otimes:8855,perp:8869,sdot:8901,loz:9674,spades:9824,clubs:9827,hearts:9829,diams:98",
    "30,check:10003,cross:10007,equals:61,colon:58,semi:59,num:35,percnt:37,lpar:40,rpar:41,lbr",
    "ack:91,rbrack:93,lbrace:123,rbrace:125,vert:124,sol:47,bsol:92,quest:63,excl:33,ast:42,plu",
    "s:43,comma:44,period:46,dollar:36,commat:64,grave:96,hat:94,lowbar:95,tilde:126,Alpha:913,",
    "Beta:914,Gamma:915,Delta:916,Epsilon:917,Zeta:918,Eta:919,Theta:920,Iota:921,Kappa:922,Lam",
    "bda:923,Mu:924,Nu:925,Xi:926,Omicron:927,Pi:928,Rho:929,Sigma:931,Tau:932,Upsilon:933,Phi:",
    "934,Chi:935,Psi:936,Omega:937,alpha:945,beta:946,gamma:947,delta:948,epsilon:949,zeta:950,",
    "eta:951,theta:952,iota:953,kappa:954,lambda:955,mu:956,nu:957,xi:958,omicron:959,pi:960,rh",
    "o:961,sigmaf:962,sigma:963,tau:964,upsilon:965,phi:966,chi:967,psi:968,omega:969",
);

const OPEN: &str = "ﬂ°";
const CLOSE: &str = "¶ß";

static RE_ENTITY: Lazy = Lazy::new(r"&(?:#(\d+)|#[xX]([0-9a-fA-F]+)|(\w+));", "g");
static NAMES: OnceLock<HashMap<&'static str, char>> = OnceLock::new();

fn lookup(name: &str) -> Option<char> {
    NAMES
        .get_or_init(|| {
            let mut names = HashMap::new();
            for pair in NAMED.split(',') {
                if let Some((name, code)) = pair.split_once(':')
                    && let Some(c) = code.parse().ok().and_then(char::from_u32)
                {
                    names.insert(name, c);
                }
            }
            names
        })
        .get(name)
        .copied()
}

fn code_point(n: f64, fallback: &str) -> String {
    if n > 0.0
        && n <= 1_114_111.0
        && !(55296.0..=57343.0).contains(&n)
        && let Some(c) = char::from_u32(n as u32)
    {
        return c.to_string();
    }
    fallback.to_string()
}

/// Mermaid's placeholders run from `ﬂ°` to `¶ß`, with a second `°` marking a
/// numeric code.
fn decode_placeholders(text: &str) -> String {
    let mut out = String::new();
    let mut last = 0;
    let mut from = text.find(OPEN);
    while let Some(at) = from {
        let body_at = at + OPEN.len();
        let Some(stop) = text[body_at..].find('¶').map(|i| body_at + i) else {
            break;
        };
        if text[stop..].starts_with(CLOSE) {
            let numeric = text[body_at..].starts_with('°');
            let body = &text[body_at + if numeric { '°'.len_utf8() } else { 0 }..stop];
            let raw = &text[at..stop + CLOSE.len()];
            out.push_str(&text[last..at]);
            if numeric {
                let _ = write!(out, "{}", code_point(super::super::js::number(body), raw));
            } else {
                match lookup(body) {
                    Some(c) => out.push(c),
                    None => {
                        out.push('&');
                        out.push_str(body);
                        out.push(';');
                    }
                }
            }
            last = stop + CLOSE.len();
        }
        let next = stop + '¶'.len_utf8();
        from = text[next..].find(OPEN).map(|i| next + i);
    }
    if last == 0 {
        return text.to_string();
    }
    out + &text[last..]
}

/// Turns Mermaid's `#35;` / `#quot;` codes, as hidden by
/// [`super::super::preprocess::encode_entities`], and HTML entities into
/// characters.
pub fn decode_entities(text: &str) -> String {
    decoded(text).into_owned()
}

/// [`decode_entities`], borrowing `text` when it holds nothing to decode.
pub fn decoded(text: &str) -> std::borrow::Cow<'_, str> {
    if !text.contains(CLOSE) && !text.contains('&') {
        return std::borrow::Cow::Borrowed(text);
    }
    std::borrow::Cow::Owned(decode_owned(text))
}

fn decode_owned(text: &str) -> String {
    let text = if text.contains(CLOSE) {
        decode_placeholders(text)
    } else {
        text.to_string()
    };
    if !text.contains('&') {
        return text;
    }
    RE_ENTITY.replace(&text, |caps| {
        let whole = caps.str(0);
        if let Some(dec) = caps.get(1) {
            return code_point(super::super::js::number(dec), whole);
        }
        if let Some(hex) = caps.get(2) {
            return code_point(super::super::js::parse_int(hex, 16), whole);
        }
        match lookup(caps.str(3)) {
            Some(c) => c.to_string(),
            None => whole.to_string(),
        }
    })
}
