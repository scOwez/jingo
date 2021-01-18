//! Scanner/lexer stage of parsing, the first main step to parse raw characters
//! into further parsable tokens

use crate::meta::{Meta, MetaPos};

use std::num::{ParseFloatError, ParseIntError};
use std::{fmt, iter::Peekable};

/// Error enumeration representing errors whilst scanning; see the [fmt::Display]
/// trait impl for documentation on each case
#[derive(Debug, Clone, PartialEq)]
pub enum ScanError {
    TokenInnerNotFound(String),
    UnexpectedEof,
    EmptyCharLiteral,
    InvalidCharEscape(char),
    UnknownStrEscape(char),
    MultipleDots,
    InvalidFloat(ParseFloatError),
    InvalidInt(ParseIntError),
}

impl fmt::Display for ScanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScanError::TokenInnerNotFound(input) => {
                write!(f, "Input '{}' is not a known keyword or identifier", input)
            }
            ScanError::UnexpectedEof => {
                write!(f, "File ended abruptly whilst scanning, unexpected EOF")
            }
            ScanError::EmptyCharLiteral => write!(f, "Character literals must not be empty"),
            ScanError::InvalidCharEscape(c) => write!(f, "Invalid char escape '{}'", c),
            ScanError::UnknownStrEscape(c) => write!(f, "Unknown string escape '{}'", c),
            ScanError::MultipleDots => write!(f, "Number given as multiple dots"),
            ScanError::InvalidFloat(err) => write!(f, "Could not parse float, {}", err),
            ScanError::InvalidInt(err) => write!(f, "Could not parse int, {}", err),
        }
    }
}

/// Type enumeration of a token, defining the possible types for a token, along
/// with any data (such as in string literals) the token may use
#[derive(Debug, Clone, PartialEq)]
pub enum TokenInner {
    // single-char
    ParenLeft,
    ParenRight,
    BraceLeft,
    BraceRight,
    Comma,
    Dot,
    Semicolon,
    FwdSlash,
    Star,
    Newline,
    Whitespace,

    // math-only symbols
    Plus,
    Minus,
    Equals,
    EqualsEquals,
    Exclaim,
    ExclaimEquals,
    Less,
    LessEquals,
    Greater,
    GreaterEquals,

    // keywords
    If,
    And,
    Or,
    Else,
    True,
    False,
    None,
    Class,
    For,
    While,
    Return,
    This,
    Var,

    // literals
    Id(String),
    Str(String),
    Char(char),
    Int(i64),
    Float(f64),

    // comments
    Comment(String),
    DocStr(String),

    // phantom (special; not added to output)
    Eof,
}

/// Represents a token with a token type + data (i.e. [TokenInner]) along with
/// positional data (i.e. [MetaPos]) where the token starts
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    /// Type + data of this token
    pub inner: TokenInner,

    /// Positional data for where this token occurs
    pub pos: MetaPos,
}

impl Token {
    /// Creates a new [Token] from initial positional data and input string
    pub fn new(
        pos: &mut MetaPos,
        input: &mut Peekable<impl Iterator<Item = char>>,
    ) -> Result<Self, ScanError> {
        pos.col += 1;

        Ok(Self {
            pos: pos.clone(),
            inner: match input.next() {
                Some(c) => match c {
                    '(' => Ok(TokenInner::ParenLeft),
                    ')' => Ok(TokenInner::ParenRight),
                    '{' => Ok(TokenInner::BraceLeft),
                    '}' => Ok(TokenInner::BraceRight),
                    ',' => Ok(TokenInner::Comma),
                    '.' => Ok(TokenInner::Dot),
                    ';' => Ok(TokenInner::Semicolon),
                    '/' => Ok(TokenInner::FwdSlash),
                    '*' => Ok(TokenInner::Star),
                    '\n' => {
                        pos.newline(1);
                        Ok(TokenInner::Newline)
                    }
                    ' ' | '\t' => Ok(TokenInner::Whitespace),
                    '+' => Ok(TokenInner::Plus),
                    '-' => get_dash_content(pos, input),
                    '=' => match input.peek() {
                        Some(&'=') => {
                            input.next();
                            Ok(TokenInner::EqualsEquals)
                        }
                        _ => Ok(TokenInner::Equals),
                    },
                    '!' => match input.peek() {
                        Some(&'=') => {
                            input.next();
                            Ok(TokenInner::ExclaimEquals)
                        }
                        _ => Ok(TokenInner::Exclaim),
                    },
                    '<' => match input.peek() {
                        Some(&'=') => {
                            input.next();
                            Ok(TokenInner::LessEquals)
                        }
                        _ => Ok(TokenInner::Less),
                    },
                    '>' => match input.peek() {
                        Some(&'=') => {
                            input.next();
                            Ok(TokenInner::GreaterEquals)
                        }
                        _ => Ok(TokenInner::Greater),
                    },
                    '"' => Ok(TokenInner::Str(get_str_content(pos, input)?)),
                    '\'' => match input.next().ok_or(ScanError::UnexpectedEof)? {
                        '\'' => Err(ScanError::EmptyCharLiteral),
                        c => match input.next().ok_or(ScanError::UnexpectedEof)? {
                            '\'' => {
                                pos.col += 2;
                                Ok(TokenInner::Char(c))
                            }
                            err_c => Err(ScanError::InvalidCharEscape(err_c)),
                        },
                    },
                    '0'..='9' => get_num_content(pos, input, c),
                    _ => todo!("identifiers"),
                },
                None => Ok(TokenInner::Eof),
            }?,
        })
    }
}

impl From<Token> for TokenInner {
    fn from(token: Token) -> Self {
        token.inner
    }
}

impl From<Token> for MetaPos {
    fn from(token: Token) -> Self {
        token.pos
    }
}

/// Scans a raw char input for a valid [TokenInner::Comment] or [TokenInner::DocStr]
fn get_comment_content(
    pos: &mut MetaPos,
    input: &mut Peekable<impl Iterator<Item = char>>,
) -> Result<String, ScanError> {
    let mut output = String::new();

    // .take_while() can't do newline
    loop {
        match input.next() {
            Some('\n') => {
                pos.newline(1);
                break;
            }
            Some(other) => output.push(other),
            None => break,
        }
    }

    Ok(output.trim().to_string())
}

/// Scans a raw char input for a valid [TokenInner::Comment] or [TokenInner::DocStr]
fn get_dash_content(
    pos: &mut MetaPos,
    input: &mut Peekable<impl Iterator<Item = char>>,
) -> Result<TokenInner, ScanError> {
    let peeked = input.peek();

    match peeked {
        Some('-') => {
            input.next();

            match input.peek() {
                Some('-') => {
                    input.next();
                    Ok(TokenInner::DocStr(get_comment_content(pos, input)?))
                }
                _ => Ok(TokenInner::Comment(get_comment_content(pos, input)?)),
            }
        }
        _ => Ok(TokenInner::Minus),
    }
}

/// Scans a raw char input for a valid [TokenInner::Str]
fn get_str_content(
    pos: &mut MetaPos,
    input: &mut Peekable<impl Iterator<Item = char>>,
) -> Result<String, ScanError> {
    let mut output = String::new();
    let mut backslash_active = false;

    loop {
        match input.next().ok_or(ScanError::UnexpectedEof)? {
            '\\' => {
                if backslash_active {
                    output.push('\\');
                    backslash_active = false;
                } else {
                    backslash_active = true;
                }
            }
            '"' => {
                if backslash_active {
                    output.push('"');
                    backslash_active = false;
                } else {
                    break;
                }
            }
            other => {
                if backslash_active {
                    match other {
                        't' | 'n' | 'r' => {
                            output.push(other); // TODO: fix
                            backslash_active = false;
                        }
                        esc => return Err(ScanError::UnknownStrEscape(esc)),
                    }
                } else {
                    output.push(other)
                }
            }
        }
    }

    pos.col += output.len();

    Ok(output)
}

/// Scans a raw char input for a valid [TokenInner::Int] or [TokenInner::Float]
fn get_num_content(
    pos: &mut MetaPos,
    input: &mut Peekable<impl Iterator<Item = char>>,
    start: char,
) -> Result<TokenInner, ScanError> {
    let mut numstr = String::from(start);
    let mut is_float = false;

    loop {
        let cur_char = match input.peek() {
            Some(c) => c,
            None => break,
        };

        match cur_char {
            '0'..='9' => (),
            '.' => {
                if is_float {
                    return Err(ScanError::MultipleDots);
                } else {
                    is_float = true;
                }
            }
            _ => break,
        }

        numstr.push(*cur_char);
        input.next();
    }

    pos.col += numstr.len() - 1;

    Ok(if is_float {
        TokenInner::Float(
            numstr
                .parse::<f64>()
                .map_err(|err| ScanError::InvalidFloat(err))?,
        )
    } else {
        TokenInner::Int(
            numstr
                .parse::<i64>()
                .map_err(|err| ScanError::InvalidInt(err))?,
        )
    })
}

/// Scan given input into a vector of [Token] for further compilation
pub fn launch(mut meta: Meta, input: impl AsRef<str>) -> Result<Vec<Token>, (ScanError, Meta)> {
    let mut input = input.as_ref().chars().into_iter().peekable();
    let mut output = vec![];

    loop {
        match Token::new(&mut meta.pos, &mut input) {
            Ok(token) => match token.inner {
                TokenInner::Eof => break,
                _ => output.push(token),
            },
            Err(err) => return Err((err, meta)),
        };
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eqeq() {
        assert_eq!(
            Token::new(&mut MetaPos::new(), &mut "==".chars().peekable())
                .unwrap()
                .inner,
            TokenInner::EqualsEquals
        )
    }

    #[test]
    fn neeq() {
        assert_eq!(
            Token::new(&mut MetaPos::new(), &mut "!=".chars().peekable())
                .unwrap()
                .inner,
            TokenInner::ExclaimEquals
        )
    }

    #[test]
    fn lesseq() {
        assert_eq!(
            Token::new(&mut MetaPos::new(), &mut "<=".chars().peekable())
                .unwrap()
                .inner,
            TokenInner::LessEquals
        )
    }

    #[test]
    fn greatereq() {
        assert_eq!(
            Token::new(&mut MetaPos::new(), &mut ">=".chars().peekable())
                .unwrap()
                .inner,
            TokenInner::GreaterEquals
        )
    }

    #[test]
    fn scan_basic() {
        let tokens = launch(Meta::new(None), "=!==!=!!=").unwrap();
        let exp = vec![
            TokenInner::Equals,
            TokenInner::ExclaimEquals,
            TokenInner::Equals,
            TokenInner::ExclaimEquals,
            TokenInner::Exclaim,
            TokenInner::ExclaimEquals,
        ];

        for (ind, token) in tokens.iter().enumerate() {
            assert_eq!(token.inner, exp[ind]);
        }
    }

    #[test]
    fn scan_token() {
        let tokens = launch(Meta::new(None), "'h''i'").unwrap();
        let exp = vec![
            Token {
                inner: TokenInner::Char('h'),
                pos: MetaPos { line: 1, col: 1 },
            },
            Token {
                inner: TokenInner::Char('i'),
                pos: MetaPos { line: 1, col: 4 },
            },
        ];

        for (ind, token) in tokens.iter().enumerate() {
            assert_eq!(token, &exp[ind]);
        }
    }

    #[test]
    fn invalid_char_escape() {
        assert_eq!(
            launch(Meta::new(None), "'h;"),
            Err((
                ScanError::InvalidCharEscape(';'),
                Meta {
                    pos: MetaPos { line: 1, col: 1 },
                    path: None
                }
            ))
        );
        assert_eq!(
            launch(Meta::new(None), "'h\""),
            Err((
                ScanError::InvalidCharEscape('"'),
                Meta {
                    pos: MetaPos { line: 1, col: 1 },
                    path: None
                }
            ))
        );
    }

    #[test]
    fn empty_char() {
        assert_eq!(
            launch(Meta::new(None), "''"),
            Err((
                ScanError::EmptyCharLiteral,
                Meta {
                    pos: MetaPos { line: 1, col: 1 },
                    path: None
                }
            ))
        )
    }

    #[test]
    fn basic_strings() {
        assert_eq!(
            launch(Meta::new(None), r#""Hello there!""#).unwrap()[0],
            Token {
                inner: TokenInner::Str(r#"Hello there!"#.to_string()),
                pos: MetaPos { line: 1, col: 1 }
            }
        );
        assert_eq!(
            launch(Meta::new(None), r#""Hello th\\ere!""#).unwrap()[0],
            Token {
                inner: TokenInner::Str(r#"Hello th\ere!"#.to_string()),
                pos: MetaPos { line: 1, col: 1 }
            }
        )
    }

    #[test]
    fn basic_int() {
        assert_eq!(
            launch(Meta::new(None), "45635463465").unwrap()[0],
            Token {
                inner: TokenInner::Int(45635463465),
                pos: MetaPos { line: 1, col: 1 }
            }
        );
        assert_eq!(
            launch(Meta::new(None), "0").unwrap()[0],
            Token {
                inner: TokenInner::Int(0),
                pos: MetaPos { line: 1, col: 1 }
            }
        );
    }

    #[test]
    fn int_int_combo() {
        assert_eq!(
            launch(Meta::new(None), "78956456+87685446+324345345").unwrap(),
            vec![
                Token {
                    inner: TokenInner::Int(78956456),
                    pos: MetaPos { line: 1, col: 1 }
                },
                Token {
                    inner: TokenInner::Plus,
                    pos: MetaPos { line: 1, col: 9 }
                },
                Token {
                    inner: TokenInner::Int(87685446),
                    pos: MetaPos { line: 1, col: 10 }
                },
                Token {
                    inner: TokenInner::Plus,
                    pos: MetaPos { line: 1, col: 18 }
                },
                Token {
                    inner: TokenInner::Int(324345345),
                    pos: MetaPos { line: 1, col: 19 }
                },
            ]
        )
    }

    #[test]
    fn basic_float() {
        assert_eq!(
            launch(Meta::new(None), "45.34234").unwrap()[0],
            Token {
                inner: TokenInner::Float(45.34234),
                pos: MetaPos { line: 1, col: 1 }
            }
        );
        assert_eq!(
            launch(Meta::new(None), "0.0").unwrap()[0],
            Token {
                inner: TokenInner::Float(0.0),
                pos: MetaPos { line: 1, col: 1 }
            }
        );
    }

    #[test]
    fn float_int_combo() {
        assert_eq!(
            launch(Meta::new(None), "453495.344294394+342342").unwrap(),
            vec![
                Token {
                    inner: TokenInner::Float(453495.344294394),
                    pos: MetaPos { line: 1, col: 1 }
                },
                Token {
                    inner: TokenInner::Plus,
                    pos: MetaPos { line: 1, col: 17 }
                },
                Token {
                    inner: TokenInner::Int(342342),
                    pos: MetaPos { line: 1, col: 18 }
                },
            ]
        );
        assert_eq!(
            launch(Meta::new(None), "4534342+3435345.3453-32324").unwrap(),
            vec![
                Token {
                    inner: TokenInner::Int(4534342),
                    pos: MetaPos { line: 1, col: 1 }
                },
                Token {
                    inner: TokenInner::Plus,
                    pos: MetaPos { line: 1, col: 8 }
                },
                Token {
                    inner: TokenInner::Float(3435345.3453),
                    pos: MetaPos { line: 1, col: 9 }
                },
                Token {
                    inner: TokenInner::Minus,
                    pos: MetaPos { line: 1, col: 21 }
                },
                Token {
                    inner: TokenInner::Int(32324),
                    pos: MetaPos { line: 1, col: 22 }
                },
            ]
        )
    }

    #[test]
    fn comment_docstr() {
        assert_eq!(
            launch(
                Meta::new(None),
                "--comment\n---         docstr\n+--- docstr\n----docstr\n- --    comment"
            )
            .unwrap(),
            vec![
                Token {
                    inner: TokenInner::Comment("comment".to_string()),
                    pos: MetaPos { line: 1, col: 1 }
                },
                Token {
                    inner: TokenInner::DocStr("docstr".to_string()),
                    pos: MetaPos { line: 2, col: 1 }
                },
                Token {
                    inner: TokenInner::Plus,
                    pos: MetaPos { line: 3, col: 1 }
                },
                Token {
                    inner: TokenInner::DocStr("docstr".to_string()),
                    pos: MetaPos { line: 3, col: 2 }
                },
                Token {
                    inner: TokenInner::DocStr("-docstr".to_string()),
                    pos: MetaPos { line: 4, col: 1 }
                },
                Token {
                    inner: TokenInner::Minus,
                    pos: MetaPos { line: 5, col: 1 }
                },
                Token {
                    inner: TokenInner::Whitespace,
                    pos: MetaPos { line: 5, col: 2 }
                },
                Token {
                    inner: TokenInner::Comment("comment".to_string()),
                    pos: MetaPos { line: 5, col: 3 }
                },
            ]
        );
    }
}
