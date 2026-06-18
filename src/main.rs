#![feature(trim_prefix_suffix)]
use rand::Rng;
use regex_syntax::ast;

type CharRanges<'a> = &'a [(char, char)];
include!(concat!(env!("OUT_DIR"), "/ucd_tables.rs"));
const UNIVERSE_UNICODE: CharRanges = &[('\0', '\u{D7FF}'), ('\u{E000}', '\u{10FFFF}')];
const UNIVERSE_ASCII: CharRanges = &[('\0', '\x7F')];
const CLASS_ASCII_ALNUM: CharRanges = &[('0', '9'), ('A', 'Z'), ('a', 'z')];
const CLASS_ASCII_ALPHA: CharRanges = &[('A', 'Z'), ('a', 'z')];
const CLASS_ASCII_ASCII: CharRanges = &[('\x00', '\x7F')];
const CLASS_ASCII_BLANK: CharRanges = &[('\t', '\t'), (' ', ' ')];
const CLASS_ASCII_CNTRL: CharRanges = &[('\x00', '\x1F'), ('\x7F', '\x7F')];
const CLASS_ASCII_DIGIT: CharRanges = &[('0', '9')];
const CLASS_ASCII_GRAPH: CharRanges = &[('!', '~')];
const CLASS_ASCII_LOWER: CharRanges = &[('a', 'z')];
const CLASS_ASCII_PRINT: CharRanges = &[(' ', '~')];
const CLASS_ASCII_PUNCT: CharRanges = &[('!', '/'), (':', '@'), ('[', '`'), ('{', '~')];
const CLASS_ASCII_SPACE: CharRanges = &[('\t', '\r'), (' ', ' ')];
const CLASS_ASCII_UPPER: CharRanges = &[('A', 'Z')];
const CLASS_ASCII_WORD: CharRanges = &[('0', '9'), ('A', 'Z'), ('a', 'z'), ('_', '_')];
const CLASS_ASCII_XDIGIT: CharRanges = &[('0', '9'), ('A', 'F'), ('a', 'f')];

#[derive(Clone, Debug, PartialEq, Eq)]
struct RandomString {
    ast: ast::Ast,
}

impl RandomString {
    fn from_regex(pattern: &str) -> Result<Self, ast::Error> {
        Ok(Self {
            ast: ast::parse::Parser::new().parse(pattern)?,
        })
    }

    fn generate(&self) -> Result<String, String> {
        Ok(RandomStringVisitor::new().into_output(&self.ast)?)
    }
}

struct RandomStringVisitor {
    rng: rand::rngs::ThreadRng,
    buf: String,
}

impl RandomStringVisitor {
    fn new() -> Self {
        Self {
            rng: rand::rng(),
            buf: String::new(),
        }
    }

    fn into_output(mut self, _ast: &ast::Ast) -> Result<String, String> {
        self.visit(_ast)?;
        Ok(self.buf)
    }

    fn random_dot(&mut self) -> char {
        self.rng.random_range(' '..='~')
    }

    fn sample_repetition(&mut self, _op: &ast::RepetitionOp) -> u32 {
        use ast::{RepetitionKind::*, RepetitionRange::*};
        const REP_MAX: u32 = u8::MAX as u32;
        let (min, max) = match _op.kind {
            ZeroOrOne => (0, 1),
            ZeroOrMore => (0, REP_MAX),
            OneOrMore => (1, REP_MAX),
            Range(Exactly(n)) => (n, n),
            Range(AtLeast(n)) => (n, REP_MAX),
            Range(Bounded(n, m)) => (n, m),
        };
        if min == max {
            min
        } else {
            self.rng.random_range(min..=max)
        }
    }

    fn sample_char_from_ranges(&mut self, ranges: CharRanges) -> Result<char, String> {
        let range_len: fn(&(char, char)) -> u32 = |&(lo, hi)| (hi as u32) - (lo as u32) + 1;
        let total = ranges.iter().map(range_len).sum();
        let mut idx = self.rng.random_range(0..total);
        for r in ranges {
            let len = range_len(r);
            if idx < len {
                let cp = (r.0 as u32) + idx;
                return Ok(char::from_u32(cp).unwrap());
            }
            idx -= len;
        }
        unreachable!("idx must fall within one of the ranges");
    }

    fn visit_class_unicode(&mut self, _ast: &ast::ClassUnicode) -> Result<(), String> {
        let ranges = class_unicode_ranges(_ast)?;
        self.sample_char_from_ranges(&ranges)
            .map(|c| self.buf.push(c))?;
        Ok(())
    }

    fn visit_class_perl(&mut self, _ast: &ast::ClassPerl) -> Result<(), String> {
        let ranges = class_perl_ranges(_ast);
        self.sample_char_from_ranges(&ranges)
            .map(|c| self.buf.push(c))?;
        Ok(())
    }

    fn visit_class_bracketed(&mut self, _ast: &ast::ClassBracketed) -> Result<(), String> {
        let ranges = class_bracketed_ranges(_ast)?;
        self.sample_char_from_ranges(&ranges)
            .map(|c| self.buf.push(c))?;
        Ok(())
    }

    fn visit(&mut self, _ast: &ast::Ast) -> Result<(), String> {
        use ast::Ast::*;
        match _ast {
            Literal(l) => self.buf.push(l.c),
            Dot(_) => {
                let c = self.random_dot();
                self.buf.push(c);
            }
            ClassUnicode(cu) => self.visit_class_unicode(cu)?,
            ClassPerl(cp) => self.visit_class_perl(cp)?,
            ClassBracketed(cb) => self.visit_class_bracketed(cb)?,
            Repetition(r) => {
                for _ in 0..self.sample_repetition(&r.op) {
                    self.visit(&r.ast)?;
                }
            }
            Group(g) => self.visit(&g.ast)?,
            Alternation(alt) => {
                let idx = self.rng.random_range(0..alt.asts.len());
                self.visit(&alt.asts[idx])?;
            }
            Concat(xs) => {
                for x in &xs.asts {
                    self.visit(x)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}

fn class_set_item_ranges(_ast: &ast::ClassSetItem) -> Result<Vec<(char, char)>, String> {
    use ast::ClassSetItem::*;
    Ok(match _ast {
        Empty(_) => vec![],
        Literal(l) => vec![(l.c, l.c)],
        Range(r) => vec![(r.start.c, r.end.c)],
        Ascii(ca) => {
            use ast::ClassAsciiKind::*;
            let ranges = match ca.kind {
                Alnum => CLASS_ASCII_ALNUM,
                Alpha => CLASS_ASCII_ALPHA,
                Ascii => CLASS_ASCII_ASCII,
                Blank => CLASS_ASCII_BLANK,
                Cntrl => CLASS_ASCII_CNTRL,
                Digit => CLASS_ASCII_DIGIT,
                Graph => CLASS_ASCII_GRAPH,
                Lower => CLASS_ASCII_LOWER,
                Print => CLASS_ASCII_PRINT,
                Punct => CLASS_ASCII_PUNCT,
                Space => CLASS_ASCII_SPACE,
                Upper => CLASS_ASCII_UPPER,
                Word => CLASS_ASCII_WORD,
                Xdigit => CLASS_ASCII_XDIGIT,
            };
            if ca.negated {
                get_negated(ranges)
            } else {
                ranges.to_vec()
            }
        }
        Unicode(cu) => class_unicode_ranges(cu)?,
        Perl(cp) => class_perl_ranges(cp),
        Bracketed(cb) => class_bracketed_ranges(cb)?,
        Union(u) => merge_char_ranges(
            u.items
                .iter()
                .map(class_set_item_ranges)
                .collect::<Result<Vec<Vec<(char, char)>>, _>>()?
                .into_iter()
                .flatten()
                .collect(),
        ),
    })
}

fn class_set_ranges(_ast: &ast::ClassSet) -> Result<Vec<(char, char)>, String> {
    use ast::{ClassSet::*, ClassSetBinaryOpKind::*};
    Ok(match _ast {
        Item(item) => class_set_item_ranges(item)?,
        BinaryOp(op) => {
            let a = class_set_ranges(&op.lhs)?;
            let b = class_set_ranges(&op.rhs)?;
            match &op.kind {
                Intersection => ranges_intersection(&a, &b),
                Difference => ranges_difference(&a, &b),
                SymmetricDifference => ranges_symmetric_difference(&a, &b),
            }
        }
    })
}

fn class_bracketed_ranges(_ast: &ast::ClassBracketed) -> Result<Vec<(char, char)>, String> {
    let ranges = class_set_ranges(&_ast.kind)?;
    Ok(if _ast.negated {
        get_negated(&ranges)
    } else {
        ranges
    })
}

fn class_perl_ranges(_ast: &ast::ClassPerl) -> Vec<(char, char)> {
    use ast::ClassPerlKind::*;
    let ranges = match &_ast.kind {
        Digit => CLASS_ASCII_DIGIT,
        Space => CLASS_ASCII_SPACE,
        Word => CLASS_ASCII_WORD,
    };
    if _ast.negated {
        get_negated(ranges)
    } else {
        ranges.to_vec()
    }
}

fn class_unicode_ranges(_ast: &ast::ClassUnicode) -> Result<Vec<(char, char)>, String> {
    use ast::ClassUnicodeKind::*;
    let ranges = match &_ast.kind {
        OneLetter(c) => match c.to_ascii_lowercase() {
            'l' => PROP_GC_LETTER.to_vec(),
            'm' => PROP_GC_MARK.to_vec(),
            'n' => PROP_GC_NUMBER.to_vec(),
            'p' => PROP_GC_PUNCTUATION.to_vec(),
            's' => PROP_GC_SYMBOL.to_vec(),
            'z' => PROP_GC_SEPARATOR.to_vec(),
            'c' => PROP_GC_OTHER.to_vec(),
            _ => return Err(format!("invalid unicode general category letter {c:?}")),
        },
        Named(name) => ucd_ranges_from_name(name)?.to_vec(),
        NamedValue { name, value, .. } => ucd_ranges_from_category(name, value)?.to_vec(),
    };
    Ok(if _ast.is_negated() {
        get_negated(&ranges)
    } else {
        ranges
    })
}

fn merge_char_ranges(ranges: Vec<(char, char)>) -> Vec<(char, char)> {
    merge_ranges(
        ranges
            .into_iter()
            .map(|(lo, hi)| (lo as u32, hi as u32))
            .collect(),
    )
    .into_iter()
    .map(|(lo, hi)| (char::from_u32(lo).unwrap(), char::from_u32(hi).unwrap()))
    .collect()
}

fn merge_ranges(mut ranges: Vec<(u32, u32)>) -> Vec<(u32, u32)> {
    if ranges.is_empty() {
        return Vec::new();
    }
    ranges.sort_unstable_by_key(|&(lo, _)| lo);
    let mut res = Vec::with_capacity(ranges.len());
    let mut cur = ranges[0];
    for &(lo, hi) in &ranges[1..] {
        if lo <= cur.1.saturating_add(1) {
            cur.1 = cur.1.max(hi);
        } else {
            res.push(cur);
            cur = (lo, hi);
        }
    }
    res.push(cur);
    res
}

fn normalize_ranges(ranges: CharRanges, universe: CharRanges) -> Vec<(u32, u32)> {
    use core::cmp::{max, min};
    let mut v = Vec::new();
    for &(r_lo, r_hi) in ranges {
        let r_lo = r_lo as u32;
        let r_hi = r_hi as u32;
        for &(u_lo, u_hi) in universe {
            let u_lo = u_lo as u32;
            let u_hi = u_hi as u32;
            if r_hi < u_lo || r_lo > u_hi {
                continue;
            }
            let lo = max(r_lo, u_lo);
            let hi = min(r_hi, u_hi);
            if lo <= hi {
                v.push((lo, hi));
            }
        }
    }
    if v.is_empty() {
        return v;
    }
    merge_ranges(v)
}

fn get_negated(ranges: CharRanges) -> Vec<(char, char)> {
    let mut negated = ranges_negation(ranges, UNIVERSE_ASCII);
    if negated.is_empty() {
        negated = ranges_negation(ranges, UNIVERSE_UNICODE);
    }
    negated
}

fn ranges_negation(ranges: CharRanges, universe: CharRanges) -> Vec<(char, char)> {
    let merged = normalize_ranges(ranges, universe);
    if merged.is_empty() {
        return universe.to_vec();
    }
    let mut res = Vec::new();
    let mut idx = 0;
    for &(u_lo, u_hi) in universe {
        let u_lo = u_lo as u32;
        let u_hi = u_hi as u32;
        if u_lo > u_hi {
            continue;
        }
        while idx < merged.len() && merged[idx].1 < u_lo {
            idx += 1;
        }
        let mut cur = u_lo;
        let mut j = idx;
        while j < merged.len() && merged[j].0 <= u_hi {
            let (r_lo, r_hi) = merged[j];
            if r_lo > cur {
                let gap_lo = cur;
                let gap_hi = r_lo - 1;
                if gap_lo <= gap_hi {
                    res.push((
                        char::from_u32(gap_lo).unwrap(),
                        char::from_u32(gap_hi).unwrap(),
                    ));
                }
            }
            cur = cur.max(r_hi.saturating_add(1));
            j += 1;
        }
        if cur <= u_hi {
            res.push((char::from_u32(cur).unwrap(), char::from_u32(u_hi).unwrap()));
        }
        idx = j;
    }
    res
}

fn ranges_intersection(a: CharRanges, b: CharRanges) -> Vec<(char, char)> {
    use core::cmp::{max, min};
    let mut res = Vec::new();
    let [mut i, mut j] = [0usize; 2];
    while i < a.len() && j < b.len() {
        let (a_lo, a_hi) = a[i];
        let (b_lo, b_hi) = b[j];
        let lo = max(a_lo, b_lo);
        let hi = min(a_hi, b_hi);
        if lo <= hi {
            res.push((lo, hi));
        }
        if a_hi < b_hi {
            i += 1;
        } else {
            j += 1;
        }
    }
    res
}

fn ranges_difference(a: CharRanges, b: CharRanges) -> Vec<(char, char)> {
    use core::cmp::min;
    let mut res = Vec::new();
    let [mut i, mut j] = [0usize; 2];
    while i < a.len() {
        let (a_lo, a_hi) = a[i];
        let mut cur_start = a_lo as u32;
        let a_hi = a_hi as u32;
        while j < b.len() && (b[j].1 as u32) < cur_start {
            j += 1;
        }
        let mut k = j;
        while k < b.len() && (b[k].0 as u32) <= a_hi {
            let (b_lo, b_hi) = b[k];
            let b_lo = b_lo as u32;
            let b_hi = b_hi as u32;
            if b_lo > cur_start {
                let gap_lo = cur_start;
                let gap_hi = min(a_hi, b_lo - 1);
                if gap_lo <= gap_hi {
                    res.push((
                        char::from_u32(gap_lo).unwrap(),
                        char::from_u32(gap_hi).unwrap(),
                    ));
                }
            }
            if b_hi >= a_hi {
                cur_start = a_hi.saturating_add(1);
                break;
            } else {
                cur_start = b_hi.saturating_add(1);
                if cur_start > a_hi {
                    break;
                }
            }
            k += 1;
        }
        if cur_start <= a_hi {
            res.push((
                char::from_u32(cur_start).unwrap(),
                char::from_u32(a_hi).unwrap(),
            ));
        }
        j = k;
        i += 1;
    }
    res
}

fn ranges_symmetric_difference(a: CharRanges, b: CharRanges) -> Vec<(char, char)> {
    if a.is_empty() {
        b.to_vec()
    } else if b.is_empty() {
        a.to_vec()
    } else {
        merge_char_ranges(
            [(a, b), (b, a)]
                .into_iter()
                .flat_map(|(i, j)| ranges_difference(i, j))
                .collect(),
        )
    }
}

fn ucd_category(
    name: &str,
) -> Result<&'static phf::Map<&'static str, CharRanges<'static>>, String> {
    let name = &*name.to_lowercase();
    Ok(*CATEGORY_NAMES
        .get(name)
        .ok_or_else(|| format!("unknown category {name:?}"))?)
}

fn ucd_ranges_from_category(property: &str, value: &str) -> Result<CharRanges<'static>, String> {
    let value = &*value.to_lowercase();
    Ok(*ucd_category(property)?
        .get(value)
        .ok_or_else(|| format!("unknown value for property {property:?}: {value:?}"))?)
}

fn ucd_ranges_from_name(name: &str) -> Result<CharRanges<'static>, String> {
    if name.is_empty() {
        return Err("property name is empty".to_string());
    };
    let name = &*name.to_lowercase();
    Ok(*BINARY_NAMES
        .get(name)
        .or_else(|| {
            CAT_GENERAL_CATEGORY
                .get(name)
                .or_else(|| CAT_SCRIPT.get(name))
        })
        .ok_or_else(|| format!("unknown property: {name:?}"))?)
}

fn main() {
    let mut argv = std::env::args();
    let prog = argv.next().unwrap();
    let pattern = match (argv.next(), argv.next()) {
        (Some(p), None) => p,
        _ => {
            eprintln!("usage: {prog} REGEX");
            std::process::exit(1);
        }
    };

    println!(
        "{}",
        RandomString::from_regex(&pattern)
            .unwrap_or_else(|e| {
                eprintln!("{e}");
                std::process::exit(1);
            })
            .generate()
            .unwrap_or_else(|e| {
                eprintln!("error: {e}");
                std::process::exit(1);
            })
            .trim_suffix('\n')
    );
}
