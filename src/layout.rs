use std::str::CharIndices;
use tui::{Tui, TuiContainerType};
use unsegen::container::{HSplit, Layout, Leaf, VSplit};

#[derive(Debug, PartialEq)]
pub enum LayoutParseError {
    TooShortExpected(&'static [char]),
    ExpectedGotMany(usize, &'static [char], char),
    SplitTypeChangeFromTo(usize, char, char),
}

#[derive(Copy, Clone)]
enum SplitType {
    H,
    V,
    None,
}
struct Input<'a>(std::iter::Peekable<CharIndices<'a>>);

const NODE_START_CHARS: &'static [char] = &['c', 't', 's', 'e', '('];
const CLOSING_BRACKET_CHARS: &'static [char] = &[')'];

impl<'a> Input<'a> {
    fn new(s: &'a str) -> Result<Self, LayoutParseError> {
        let mut ret = Self(s.char_indices().peekable());
        let _ = ret
            .0
            .peek()
            .ok_or(LayoutParseError::TooShortExpected(NODE_START_CHARS))?;
        Ok(ret)
    }
    fn current(&mut self) -> Option<char> {
        self.0.peek().map(|v| v.1)
    }
    fn current_index(&mut self) -> usize {
        self.0.peek().unwrap().0
    }
    fn advance(&mut self) {
        self.0.next();
    }
}

fn try_parse_weight<'a>(i: &mut Input<'a>) -> f64 {
    if !i.current().map(|v| v.is_digit(10)).unwrap_or(false) {
        return 1.0;
    }
    let mut w = 0;
    loop {
        if let Some(i) = i.current() {
            w = match i.to_digit(10) {
                Some(d) => w * 10 + d,
                None => return w as _,
            };
        } else {
            return w as _;
        }
        i.advance();
    }
}
fn try_parse_leaf<'a, 'b>(i: &mut Input<'a>) -> Option<Box<dyn Layout<Tui<'b>> + 'b>> {
    let ret = match i.current()? {
        'c' => Box::new(Leaf::new(TuiContainerType::Console)),
        't' => Box::new(Leaf::new(TuiContainerType::Terminal)),
        's' => Box::new(Leaf::new(TuiContainerType::SrcView)),
        'e' => Box::new(Leaf::new(TuiContainerType::ExpressionTable)),
        _ => return None,
    };
    i.advance();
    Some(ret)
}

fn parse_node<'a, 'b>(
    i: &mut Input<'a>,
) -> Result<Box<dyn Layout<Tui<'b>> + 'b>, LayoutParseError> {
    let mut nodes = Vec::new();
    let mut split_type = SplitType::None;
    loop {
        let weight = try_parse_weight(i);
        if let Some(l) = try_parse_leaf(i) {
            nodes.push((l, weight));
        } else {
            match i.current() {
                Some('(') => {
                    i.advance();
                    nodes.push((parse_node(i)?, weight));
                    match i.current() {
                        Some(')') => {
                            i.advance();
                        }
                        Some(o) => {
                            return Err(LayoutParseError::ExpectedGotMany(
                                i.current_index(),
                                CLOSING_BRACKET_CHARS,
                                o,
                            ));
                        }
                        None => {
                            return Err(LayoutParseError::TooShortExpected(CLOSING_BRACKET_CHARS))
                        }
                    }
                }
                Some(o) => {
                    return Err(LayoutParseError::ExpectedGotMany(
                        i.current_index(),
                        NODE_START_CHARS,
                        o,
                    ));
                }
                None => {
                    return Err(LayoutParseError::TooShortExpected(NODE_START_CHARS));
                }
            }
        }

        let c = if let Some(c) = i.current() {
            c
        } else {
            break;
        };
        split_type = match (split_type, c) {
            (SplitType::None, '|') => SplitType::H,
            (SplitType::H, '|') => SplitType::H,
            (SplitType::V, '|') => {
                return Err(LayoutParseError::SplitTypeChangeFromTo(
                    i.current_index(),
                    '-',
                    '|',
                ))
            }
            (SplitType::None, '-') => SplitType::V,
            (SplitType::V, '-') => SplitType::V,
            (SplitType::H, '-') => {
                return Err(LayoutParseError::SplitTypeChangeFromTo(
                    i.current_index(),
                    '|',
                    '-',
                ))
            }
            (_, _) => break,
        };
        i.advance();
    }
    Ok(match split_type {
        SplitType::H => Box::new(HSplit::new(nodes)),
        SplitType::V => Box::new(VSplit::new(nodes)),
        SplitType::None => {
            assert!(nodes.len() == 1);
            nodes.pop().unwrap().0
        }
    })
}

pub fn parse(s: &str) -> Result<Box<dyn Layout<Tui> + '_>, LayoutParseError> {
    let mut i = Input::new(s)?;
    parse_node(&mut i)
}

#[cfg(test)]
mod test {
    use super::*;

    fn stringify(l: &dyn Layout<Tui<'_>>) -> String {
        format!("{:?}", l)
    }
    #[track_caller]
    fn expect_equal(input: &str, expected: &str) {
        let parsed = parse(input).unwrap();
        assert_eq!(&stringify(&*parsed), expected);
    }
    #[track_caller]
    fn expect_error(input: &str, e: LayoutParseError) {
        let pe = parse(input).unwrap_err();
        assert_eq!(pe, e);
    }
    #[test]
    fn parse_default() {
        expect_equal(
            "(1s-1c)|(1e-1t)",
            "(1(1SrcView-1Console)|1(1ExpressionTable-1Terminal))",
        );
    }
    #[test]
    fn parse_triple_weighted() {
        expect_equal(
            "(s|2t|c)-99e",
            "(1(1SrcView|2Terminal|1Console)-99ExpressionTable)",
        );
    }
    #[test]
    fn parse_empty() {
        expect_error("", LayoutParseError::TooShortExpected(NODE_START_CHARS));
    }
    #[test]
    fn parse_unclosed() {
        expect_error(
            "(c-e",
            LayoutParseError::TooShortExpected(CLOSING_BRACKET_CHARS),
        );
    }
    #[test]
    fn parse_unexpected() {
        expect_error(
            "f",
            LayoutParseError::ExpectedGotMany(0, NODE_START_CHARS, 'f'),
        );
    }
    #[test]
    fn parse_change_split() {
        expect_error(
            "s-e|t",
            LayoutParseError::SplitTypeChangeFromTo(3, '-', '|'),
        );
    }
}
