// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

use anyhow::{anyhow, bail, Result};
use move_core_types::account_address::AccountAddress;
use num_bigint::BigUint;
use std::{collections::BTreeMap, fmt::Display, iter::Peekable, num::ParseIntError};

use crate::{
    address::{NumericalAddress, ParsedAddress},
    types::{ParsedStructType, ParsedType, TypeToken},
    values::{ParsableValue, ParsedValue, ValueToken},
};

pub trait Token: Display + Copy + Eq {
    fn is_whitespace(&self) -> bool;
    fn next_token(s: &str) -> Result<Option<(Self, usize)>>;
    fn tokenize(mut s: &str) -> Result<Vec<(Self, &str)>> {
        let mut v = vec![];
        while let Some((tok, n)) = Self::next_token(s)? {
            v.push((tok, &s[..n]));
            s = &s[n..];
        }
        Ok(v)
    }
}

pub struct Parser<'a, Tok: Token, I: Iterator<Item = (Tok, &'a str)>> {
    it: Peekable<I>,
}

impl<'a, Tok: Token, I: Iterator<Item = (Tok, &'a str)>> Parser<'a, Tok, I> {
    pub fn new<T: IntoIterator<Item = (Tok, &'a str), IntoIter = I>>(v: T) -> Self {
        Self {
            it: v.into_iter().peekable(),
        }
    }

    pub fn next(&mut self) -> Result<(Tok, &'a str)> {
        match self.it.next() {
            Some(tok) => Ok(tok),
            None => bail!("unexpected end of tokens"),
        }
    }

    pub fn peek(&mut self) -> Option<(Tok, &'a str)> {
        self.it.peek().copied()
    }

    pub fn peek_tok(&mut self) -> Option<Tok> {
        self.it.peek().map(|(tok, _)| *tok)
    }

    pub fn consume(&mut self, tok: Tok) -> Result<&'a str> {
        let (t, contents) = self.next()?;
        if t != tok {
            bail!("expected token {}, got {}", tok, t)
        }
        Ok(contents)
    }

    pub fn parse_list<R>(
        &mut self,
        parse_list_item: impl Fn(&mut Self) -> Result<R>,
        delim: Tok,
        end_token: Tok,
        allow_trailing_delim: bool,
    ) -> Result<Vec<R>> {
        let is_end =
            |tok_opt: Option<Tok>| -> bool { tok_opt.map(|tok| tok == end_token).unwrap_or(true) };
        let mut v = vec![];
        while !is_end(self.peek_tok()) {
            v.push(parse_list_item(self)?);
            if is_end(self.peek_tok()) {
                break;
            }
            self.consume(delim)?;
            if is_end(self.peek_tok()) && allow_trailing_delim {
                break;
            }
        }
        Ok(v)
    }
}

impl<'a, I: Iterator<Item = (TypeToken, &'a str)>> Parser<'a, TypeToken, I> {
    pub fn parse_type(&mut self) -> Result<ParsedType> {
        let (tok, contents) = self.next()?;
        Ok(match (tok, contents) {
            (TypeToken::Ident, "u8") => ParsedType::U8,
            (TypeToken::Ident, "u64") => ParsedType::U64,
            (TypeToken::Ident, "u128") => ParsedType::U128,
            (TypeToken::Ident, "bool") => ParsedType::Bool,
            (TypeToken::Ident, "address") => ParsedType::Address,
            (TypeToken::Ident, "signer") => ParsedType::Signer,
            (TypeToken::Ident, "vector") => {
                self.consume(TypeToken::Lt)?;
                let ty = self.parse_type()?;
                self.consume(TypeToken::Gt)?;
                ParsedType::Vector(Box::new(ty))
            }
            (TypeToken::Ident, _) | (TypeToken::AddressIdent, _) => {
                let addr_tok = match tok {
                    TypeToken::Ident => ValueToken::Ident,
                    TypeToken::AddressIdent => ValueToken::Number,
                    _ => unreachable!(),
                };
                let address = parse_address_impl(addr_tok, contents)?;
                self.consume(TypeToken::ColonColon)?;
                let module_contents = self.consume(TypeToken::Ident)?;
                self.consume(TypeToken::ColonColon)?;
                let struct_contents = self.consume(TypeToken::Ident)?;
                let type_args = match self.peek_tok() {
                    Some(TypeToken::Lt) => {
                        self.next()?;
                        let type_args = self.parse_list(
                            |parser| parser.parse_type(),
                            TypeToken::Comma,
                            TypeToken::Gt,
                            true,
                        )?;
                        self.consume(TypeToken::Gt)?;
                        type_args
                    }
                    _ => vec![],
                };
                ParsedType::Struct(ParsedStructType {
                    address,
                    module: module_contents.to_owned(),
                    name: struct_contents.to_owned(),
                    type_args,
                })
            }
            _ => bail!("unexpected token {}, expected type", tok),
        })
    }
}

impl<'a, I: Iterator<Item = (ValueToken, &'a str)>> Parser<'a, ValueToken, I> {
    pub fn parse_value<Extra: ParsableValue>(&mut self) -> Result<ParsedValue<Extra>> {
        if let Some(extra) = Extra::parse_value(self) {
            return Ok(ParsedValue::Custom(extra));
        }
        let (tok, contents) = self.next()?;
        Ok(match tok {
            ValueToken::Number if !matches!(self.peek_tok(), Some(ValueToken::ColonColon)) => {
                let (u, _) = parse_u128(contents)?;
                ParsedValue::InferredNum(u)
            }
            ValueToken::NumberTyped => {
                if let Some(s) = contents.strip_suffix("u8") {
                    let (u, _) = parse_u8(s)?;
                    ParsedValue::U8(u)
                } else if let Some(s) = contents.strip_suffix("u64") {
                    let (u, _) = parse_u64(s)?;
                    ParsedValue::U64(u)
                } else {
                    let (u, _) = parse_u128(contents.strip_suffix("u128").unwrap())?;
                    ParsedValue::U128(u)
                }
            }
            ValueToken::True => ParsedValue::Bool(true),
            ValueToken::False => ParsedValue::Bool(false),

            ValueToken::ByteString => {
                let contents = contents
                    .strip_prefix("b\"")
                    .unwrap()
                    .strip_suffix("\"")
                    .unwrap();
                ParsedValue::Vector(
                    contents
                        .as_bytes()
                        .iter()
                        .copied()
                        .map(ParsedValue::U8)
                        .collect(),
                )
            }
            ValueToken::HexString => {
                let contents = contents
                    .strip_prefix("x\"")
                    .unwrap()
                    .strip_suffix("\"")
                    .unwrap()
                    .to_ascii_lowercase();
                ParsedValue::Vector(
                    hex::decode(contents)
                        .unwrap()
                        .into_iter()
                        .map(ParsedValue::U8)
                        .collect(),
                )
            }

            ValueToken::AtSign => ParsedValue::Address(self.parse_address()?),

            ValueToken::Ident if contents == "vector" => {
                self.consume(ValueToken::LBracket)?;
                let values = self.parse_list(
                    |parser| parser.parse_value(),
                    ValueToken::Comma,
                    ValueToken::RBracket,
                    true,
                )?;
                self.consume(ValueToken::RBracket)?;
                ParsedValue::Vector(values)
            }

            ValueToken::Number | ValueToken::Ident => {
                let addr_ident = parse_address_impl(tok, contents)?;
                self.consume(ValueToken::ColonColon)?;
                let module_name = self.consume(ValueToken::Ident)?.to_owned();
                self.consume(ValueToken::ColonColon)?;
                let struct_name = self.consume(ValueToken::Ident)?.to_owned();
                self.consume(ValueToken::LBrace)?;
                let values_vec = self.parse_list(
                    |parser| {
                        let field = parser.consume(ValueToken::Ident)?.to_owned();
                        parser.consume(ValueToken::Colon)?;
                        let value = parser.parse_value()?;
                        Ok((field, value))
                    },
                    ValueToken::Comma,
                    ValueToken::RBracket,
                    true,
                )?;
                self.consume(ValueToken::RBrace)?;
                let mut values = BTreeMap::new();
                for (field, value) in values_vec {
                    if let Some(_prev) = values.insert(field.clone(), value) {
                        // TODO should this be done in here? Seems useful for most tools though...
                        bail!("Duplicate field binding for field: {}", field)
                    }
                }
                ParsedValue::Struct(addr_ident, module_name, struct_name, values)
            }

            _ => bail!("unexpected token {}, expected type", tok),
        })
    }

    pub fn parse_address(&mut self) -> Result<ParsedAddress> {
        let (tok, contents) = self.next()?;
        parse_address_impl(tok, contents)
    }
}

pub fn parse_address_impl(tok: ValueToken, contents: &str) -> Result<ParsedAddress> {
    Ok(match tok {
        ValueToken::Number => ParsedAddress::Numerical(
            NumericalAddress::parse_str(contents)
                .map_err(|s| anyhow!("Failed to parse numerical address: {}", s))?,
        ),
        ValueToken::Ident => ParsedAddress::Named(contents.to_owned()),
        _ => bail!("unexpected token {}, expected identifier or number", tok),
    })
}

fn parse<'a, Tok: Token, R>(
    s: &'a str,
    f: impl FnOnce(&mut Parser<'a, Tok, std::vec::IntoIter<(Tok, &'a str)>>) -> Result<R>,
) -> Result<R> {
    let tokens: Vec<_> = Tok::tokenize(s)?
        .into_iter()
        .filter(|(tok, _)| !tok.is_whitespace())
        .collect();
    let mut parser = Parser::new(tokens);
    let res = f(&mut parser)?;
    if let Ok((_, contents)) = parser.next() {
        bail!("Expected end of token stream. Got: {}", contents)
    }
    Ok(res)
}

impl ParsedType {
    pub fn parse(s: &str) -> Result<ParsedType> {
        parse(s, |parser| parser.parse_type())
    }
}

impl ParsedStructType {
    pub fn parse(s: &str) -> Result<ParsedStructType> {
        let ty = parse(s, |parser| parser.parse_type())
            .map_err(|e| anyhow!("Invalid struct type: {}. Got error: {}", s, e))?;
        match ty {
            ParsedType::Struct(s) => Ok(s),
            _ => bail!("Invalid struct type: {}", s),
        }
    }
}

impl ParsedAddress {
    pub fn parse(s: &str) -> Result<ParsedAddress> {
        parse(s, |parser| parser.parse_address())
    }
}

impl<Extra: ParsableValue> ParsedValue<Extra> {
    pub fn parse(s: &str) -> Result<ParsedValue<Extra>> {
        parse(s, |parser| parser.parse_value())
    }
}

#[derive(Ord, PartialOrd, Eq, PartialEq, Hash, Clone, Copy)]
#[repr(u32)]
/// Number format enum, the u32 value represents the base
pub enum NumberFormat {
    Decimal = 10,
    Hex = 16,
}

// Determines the base of the number literal, depending on the prefix
pub(crate) fn determine_num_text_and_base(s: &str) -> (&str, NumberFormat) {
    match s.strip_prefix("0x") {
        Some(s_hex) => (s_hex, NumberFormat::Hex),
        None => (s, NumberFormat::Decimal),
    }
}

// Parse a u8 from a decimal or hex encoding
pub fn parse_u8(s: &str) -> Result<(u8, NumberFormat), ParseIntError> {
    let (txt, base) = determine_num_text_and_base(s);
    Ok((u8::from_str_radix(txt, base as u32)?, base))
}

// Parse a u64 from a decimal or hex encoding
pub fn parse_u64(s: &str) -> Result<(u64, NumberFormat), ParseIntError> {
    let (txt, base) = determine_num_text_and_base(s);
    Ok((u64::from_str_radix(txt, base as u32)?, base))
}

// Parse a u128 from a decimal or hex encoding
pub fn parse_u128(s: &str) -> Result<(u128, NumberFormat), ParseIntError> {
    let (txt, base) = determine_num_text_and_base(s);
    Ok((u128::from_str_radix(txt, base as u32)?, base))
}

// Parse an address from a decimal or hex encoding
pub fn parse_address_number(s: &str) -> Option<([u8; AccountAddress::LENGTH], NumberFormat)> {
    let (txt, base) = determine_num_text_and_base(s);
    let parsed = BigUint::parse_bytes(
        txt.as_bytes(),
        match base {
            NumberFormat::Hex => 16,
            NumberFormat::Decimal => 10,
        },
    )?;
    let bytes = parsed.to_bytes_be();
    if bytes.len() > AccountAddress::LENGTH {
        return None;
    }
    let mut result = [0u8; AccountAddress::LENGTH];
    result[(AccountAddress::LENGTH - bytes.len())..].clone_from_slice(&bytes);
    Some((result, base))
}

// pub fn parse_raw_address(s: &str) -> Result<RawAddress> {}

// pub fn parse_struct_tag(s: &str) -> Result<StructTag> {
//     let type_tag = parse(s, |parser| parser.parse_type_tag())
//         .map_err(|e| format_err!("invalid struct tag: {}, {}", s, e))?;
//     if let TypeTag::Struct(struct_tag) = type_tag {
//         Ok(struct_tag)
//     } else {
//         bail!("invalid struct tag: {}", s)
//     }
// }

// #[cfg(test)]
// mod tests {
//     use crate::{
//         account_address::AccountAddress,
//         parser::{parse_struct_tag, parse_transaction_argument, parse_type_tag},
//         transaction_argument::TransactionArgument,
//     };

//     #[allow(clippy::unreadable_literal)]
//     #[test]
//     fn tests_parse_transaction_argument_positive() {
//         use TransactionArgument as T;

//         for (s, expected) in &[
//             ("  0u8", T::U8(0)),
//             ("0u8", T::U8(0)),
//             ("255u8", T::U8(255)),
//             ("0", T::U64(0)),
//             ("0123", T::U64(123)),
//             ("0u64", T::U64(0)),
//             ("18446744073709551615", T::U64(18446744073709551615)),
//             ("18446744073709551615u64", T::U64(18446744073709551615)),
//             ("0u128", T::U128(0)),
//             (
//                 "340282366920938463463374607431768211455u128",
//                 T::U128(340282366920938463463374607431768211455),
//             ),
//             ("true", T::Bool(true)),
//             ("false", T::Bool(false)),
//             (
//                 "0x0",
//                 T::Address(AccountAddress::from_hex_literal("0x0").unwrap()),
//             ),
//             (
//                 "0x54afa3526",
//                 T::Address(AccountAddress::from_hex_literal("0x54afa3526").unwrap()),
//             ),
//             (
//                 "0X54afa3526",
//                 T::Address(AccountAddress::from_hex_literal("0x54afa3526").unwrap()),
//             ),
//             ("x\"7fff\"", T::U8Vector(vec![0x7f, 0xff])),
//             ("x\"\"", T::U8Vector(vec![])),
//             ("x\"00\"", T::U8Vector(vec![0x00])),
//             ("x\"deadbeef\"", T::U8Vector(vec![0xde, 0xad, 0xbe, 0xef])),
//         ] {
//             assert_eq!(&parse_transaction_argument(s).unwrap(), expected)
//         }
//     }

//     #[test]
//     fn tests_parse_transaction_argument_negative() {
//         for s in &[
//             "-3",
//             "0u42",
//             "0u645",
//             "0u64x",
//             "0u6 4",
//             "0u",
//             "256u8",
//             "18446744073709551616",
//             "18446744073709551616u64",
//             "340282366920938463463374607431768211456u128",
//             "0xg",
//             "0x00g0",
//             "0x",
//             "0x_",
//             "",
//             "x\"ffff",
//             "x\"a \"",
//             "x\" \"",
//             "x\"0g\"",
//             "x\"0\"",
//             "garbage",
//             "true3",
//             "3false",
//             "3 false",
//             "",
//         ] {
//             assert!(parse_transaction_argument(s).is_err())
//         }
//     }

//     #[test]
//     fn test_type_tag() {
//         for s in &[
//             "u64",
//             "bool",
//             "vector<u8>",
//             "vector<vector<u64>>",
//             "signer",
//             "0x1::M::S",
//             "0x2::M::S_",
//             "0x3::M_::S",
//             "0x4::M_::S_",
//             "0x00000000004::M::S",
//             "0x1::M::S<u64>",
//             "0x1::M::S<0x2::P::Q>",
//             "vector<0x1::M::S>",
//             "vector<0x1::M_::S_>",
//             "vector<vector<0x1::M_::S_>>",
//             "0x1::M::S<vector<u8>>",
//         ] {
//             assert!(parse_type_tag(s).is_ok(), "Failed to parse tag {}", s);
//         }
//     }

//     #[test]
//     fn test_parse_valid_struct_tag() {
//         let valid = vec![
//             "0x1::Diem::Diem",
//             "0x1::Diem_Type::Diem",
//             "0x1::Diem_::Diem",
//             "0x1::X_123::X32_",
//             "0x1::Diem::Diem_Type",
//             "0x1::Diem::Diem<0x1::XDX::XDX>",
//             "0x1::Diem::Diem<0x1::XDX::XDX_Type>",
//             "0x1::Diem::Diem<u8>",
//             "0x1::Diem::Diem<u64>",
//             "0x1::Diem::Diem<u128>",
//             "0x1::Diem::Diem<bool>",
//             "0x1::Diem::Diem<address>",
//             "0x1::Diem::Diem<signer>",
//             "0x1::Diem::Diem<vector<0x1::XDX::XDX>>",
//             "0x1::Diem::Diem<u8,bool>",
//             "0x1::Diem::Diem<u8,   bool>",
//             "0x1::Diem::Diem<u8  ,bool>",
//             "0x1::Diem::Diem<u8 , bool  ,    vector<u8>,address,signer>",
//             "0x1::Diem::Diem<vector<0x1::Diem::Struct<0x1::XUS::XUS>>>",
//             "0x1::Diem::Diem<0x1::Diem::Struct<vector<0x1::XUS::XUS>, 0x1::Diem::Diem<vector<0x1::Diem::Struct<0x1::XUS::XUS>>>>>",
//         ];
//         for text in valid {
//             let st = parse_struct_tag(text).expect("valid StructTag");
//             assert_eq!(
//                 st.to_string().replace(" ", ""),
//                 text.replace(" ", ""),
//                 "text: {:?}, StructTag: {:?}",
//                 text,
//                 st
//             );
//         }
//     }
// }
