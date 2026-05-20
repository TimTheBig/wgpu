/*!
    # Printf parsing
    This parses the format string into segments
*/

use core::{fmt::{Display, Write}, str::FromStr, num::NonZeroU8};
use alloc::{borrow::ToOwned, string::String, vec::Vec};

#[cfg(feature = "arbitrary")]
use arbitrary::Arbitrary;
#[cfg(feature = "deserialize")]
use serde::Deserialize;
#[cfg(feature = "serialize")]
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "deserialize", derive(Deserialize))]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary))]
pub enum FormatElement {
    Verbatim(String),
    Format(ConversionSpecifier),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "deserialize", derive(Deserialize))]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary))]
pub(crate) struct ConversionSpecifier {
    /// The precision to display the value with
    // for Vulkan this is the literal digits immediately after `%`
    pub(crate) precision: PrecisionParam,
    /// The data type to print as
    pub(crate) conversion_type: ConversionType,
    /// If the specifier was a vector (e.g. %v3f), vector_len = Some(2|3|4) \
    /// This may be lowered to a collection of component prints in certain APIs
    pub(crate) vector_len: Option<NonZeroU8>,
}

impl Display for ConversionSpecifier {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.precision {
            PrecisionParam::Literal(p) => write!(f, "%{p}"),
            PrecisionParam::FromArgument => write!(f, "%"),
        }?;
        // todo don't emit vec if vulkan_portibility || not(vulkan), instead lower to manual vec print
        if let Some(vec_len) = self.vector_len {
            write!(f, "v{}{}", vec_len, self.conversion_type)
        } else {
            write!(f, "{}", self.conversion_type)
        }?;

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "deserialize", derive(Deserialize))]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary))]
pub(crate) enum PrecisionParam {
    Literal(u8),
    /// Use the default precision for the conversion type,
    /// will never be present in fully parsed format strings
    FromArgument,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "deserialize", derive(Deserialize))]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary))]
pub(crate) enum ConversionType {
    /// d, i
    DecInt,
    /// o
    OctInt,
    /// x
    HexIntLower,
    /// X
    HexIntUpper,
    /// e
    SciFloatLower,
    /// E
    SciFloatUpper,
    /// f
    DecFloatLower,
    /// F
    DecFloatUpper,
    /// g
    CompactFloatLower,
    /// G
    CompactFloatUpper,
    /// %
    PercentSign,
    // todo check if this is prortible, if not remove
    /// p  (PhysicalStorageBuffer pointer)
    Pointer,
}

impl Display for ConversionType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_char(match self {
            ConversionType::DecInt => 'd',
            ConversionType::OctInt => 'o',
            ConversionType::HexIntLower => 'x',
            ConversionType::HexIntUpper => 'X',
            ConversionType::SciFloatLower => 'e',
            ConversionType::SciFloatUpper => 'E',
            ConversionType::DecFloatLower => 'f',
            ConversionType::DecFloatUpper => 'F',
            ConversionType::CompactFloatLower => 'g',
            ConversionType::CompactFloatUpper => 'G',
            ConversionType::PercentSign => '%',
            ConversionType::Pointer => 'p',
        })
    }
}

pub(crate) const VALID_FORMAT_SPECIFIER: &str = "d, i, o, x, X, e, E, f, F, g, G, %, p";

#[derive(Clone, Debug, thiserror::Error)]
pub struct PrintfParseError {
    pub kind: PrintfParseErrorKind,
    /// The start of format specifier in question, relitive to the start of the string
    pub(crate) specifier_offset: u32,
    /// Length of the format specifier in question, relitive to the offset of the specifier
    pub(crate) specifier_length: u32,
}

impl Display for PrintfParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.kind)
    }
}

#[derive(Clone, Debug, thiserror::Error)]
pub enum PrintfParseErrorKind {
    #[error("{} is not a valid format specifier, valid are: {}", .0, VALID_FORMAT_SPECIFIER)]
    InvalidFormatSpecifier(char),
    /// If '%' is the last character, it's a parse error.
    #[error("You must put a format specifier after '%'")]
    NoSpecifierAfterPercent,
    /// min is 2, max is 4
    #[error("{} is an invalid vector length, must be in range 2-4", .0)]
    InvalidVectorLength(char),
    #[error("No vector length specified, must be an int in range 2-4")]
    NoVectorLengthSpecified,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "deserialize", derive(Deserialize))]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary))]
pub struct PrintfString(pub(crate) Vec<FormatElement>);

impl FromStr for PrintfString {
    type Err = PrintfParseError;

    fn from_str(fmt: &str) -> Result<PrintfString, PrintfParseError> {
        parse_format_string(fmt)
    }
}

impl Display for PrintfString {
    // should print a valid WGSL format string,
    // note this is not athoritive only the parser is
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for fmt_elem in &self.0 {
            match fmt_elem {
                FormatElement::Verbatim(vr_str) => f.write_str(vr_str),
                FormatElement::Format(conversion_specifier) => write!(f, "{conversion_specifier}"),
            }?
        }

        Ok(())
    }
}

/// Parses a Vulkan debugPrintf style format string into elements.
/// Accepts patterns described above (precision right after `%`, optional `vN` vector, `ul/lu/lx`).
fn parse_format_string(fmt: &str) -> Result<PrintfString, PrintfParseError> {
    // todo get size from number of unescaped '%'
    let mut res = Vec::new();
    let mut rem = fmt;
    let mut str_offset = 0;

    while !rem.is_empty() {
        if let Some((verbatim_prefix, rest)) = rem.split_once('%') {
            str_offset += (rem.len() - rest.len()) as u32;
            let get_sep_len = || {
                rest.find(|c: char| -> bool {
                c.is_ascii_whitespace()
            }).unwrap_or(rest.len()) as u32 };

            if !verbatim_prefix.is_empty() {
                res.push(FormatElement::Verbatim(verbatim_prefix.to_owned()));
            }
            // If '%' is the last character, it's a parse error.
            if rest.is_empty() {
                // todo no need for index to next space as there is no specifier
                return Err(PrintfParseError {
                    kind: PrintfParseErrorKind::NoSpecifierAfterPercent,
                    specifier_offset: str_offset,
                    specifier_length: get_sep_len(),
                });
            }
            let (spec, rest) = take_conversion_specifier(rest,(str_offset, get_sep_len))?;
            res.push(FormatElement::Format(spec));
            rem = rest;
        } else {
            res.push(FormatElement::Verbatim(rem.to_owned()));
            break;
        }
    }

    Ok(PrintfString(res))
}

fn take_conversion_specifier(s: &str, (specifier_offset, find_specifier_len): (u32, impl Fn() -> u32)) -> Result<(ConversionSpecifier, &str), PrintfParseError> {
    // default initializer, most flags aren't actually used in Vulkan debugPrintf,
    // but we preserve fields for compatibility
    let mut spec = ConversionSpecifier {
        precision: PrecisionParam::FromArgument, // todo placeholder
        conversion_type: ConversionType::DecInt,
        vector_len: None,
    };

    let mut s = s;

    // Vulkan debugPrintf's grammar is restricted compared to full printf:
    // - it allows an optional decimal precision immediately after '%'
    // - optional vector specifier 'v' + [2|3|4]
    // - then a type token which may be single-char (d,i,o,u,x,X,a,A,e,E,f,F,g,G,c,s,%) or
    //   two-letter 'ul'/'lu'/'lx' (unsigned-long / long-unsigned / long-hex) or 'p'

    // Vulkan uses precision as digits right after '%', but your original parser handled .precision.
    // We'll support BOTH: either `.N` or just `N` immediately after `%`. Prefer the digits
    // immediately after '%' if present (we're already in that context).
    if matches!(s.chars().next(), Some('.')) {
        // classical form ".N"
        s = &s[1..];
        // todo share parser for both forms
        let (p, s2) = take_numeric_param(s);
        spec.precision = p;
        s = s2;
    } else {
        // try to parse a sequence of digits as Vulkan-style "precision" immediately here.
        // Only accept as precision if there are digits and the next char isn't a length/type char we expect.
        let mut chars = s.chars();
        if let Some(next) = chars.next() {
            if next.is_ascii_digit() {
                // parse literal digits
                let mut s_lit = s;
                let mut p = 0u32;
                // FIXME: once the toolchain is updated to support if let chains change this to: while let Some(d) = s_lit.chars().next() && d.is_ascii_digit()
                // precision must fit in u8
                for _ in 0..3 {
                    match s_lit.chars().next() {
                        // Convert ASCII digit to its integer value
                        Some(d) if d.is_ascii_digit() => {
                            p = 10 * p + ((d as u8) - (b'0')) as u32;
                            s_lit = &s_lit[1..];
                        }
                        _ => break,
                    }
                }
                if p > 255 {
                    // todo error must be less then 255
                }
                spec.precision = PrecisionParam::Literal(p as u8);
                s = s_lit;
            }
        }
    }

    // Check for vector specifier: 'v' followed by 2/3/4
    if matches!(s.chars().next(), Some('v')) {
        let rest = &s[1..];
        match rest.chars().next() {
            Some('2') => {
                spec.vector_len = Some(const { NonZeroU8::new(2).unwrap() });
                s = &rest[1..];
            }
            Some('3') => {
                spec.vector_len = Some(const { NonZeroU8::new(3).unwrap() });
                s = &rest[1..];
            }
            Some('4') => {
                spec.vector_len = Some(const { NonZeroU8::new(4).unwrap() });
                s = &rest[1..];
            }
            // invalid vector length
            Some(ln_c) => {
                return Err(PrintfParseError {
                    kind: PrintfParseErrorKind::InvalidVectorLength(ln_c),
                    specifier_offset,
                    specifier_length: find_specifier_len(),
                });
            }
            None => {
                return Err(PrintfParseError {
                    kind: PrintfParseErrorKind::NoVectorLengthSpecified,
                    specifier_offset,
                    specifier_length: find_specifier_len(),
                });
            }
        }
    }

    // Now check for the special two-letter specifiers Vulkan allows: "ul", "lu", "lx"
    if s.starts_with("ul") || s.starts_with("lu") {
        spec.conversion_type = ConversionType::DecInt;
        s = &s[2..];
    } else if s.starts_with("lx") {
        // todo in vulkan is this upper or lower
        spec.conversion_type = ConversionType::HexIntLower;
        s = &s[2..];
    } else {
        // single-character conversion types (or '%', 'p')
        spec.conversion_type = match s.chars().next() {
            Some('i') | Some('d') => ConversionType::DecInt,
            Some('o') => ConversionType::OctInt,
            Some('x') => ConversionType::HexIntLower,
            Some('X') => ConversionType::HexIntUpper,
            Some('a') | Some('A') => {
                // treat 'a'/'A' as floats (hex-float), map to float family
                ConversionType::DecFloatLower
            }
            Some('e') => ConversionType::SciFloatLower,
            Some('E') => ConversionType::SciFloatUpper,
            Some('f') => ConversionType::DecFloatLower,
            Some('F') => ConversionType::DecFloatUpper,
            Some('g') => ConversionType::CompactFloatLower,
            Some('G') => ConversionType::CompactFloatUpper,
            Some('u') => {
                // plain 'u' is unsigned 32-bit unless long flag applied (handled above)
                ConversionType::DecInt // reuse DecInt, but codegen should treat as unsigned; old code used DecInt for d/i/u - you may want a dedicated Unsigned variant
            }
            Some('p') => ConversionType::Pointer,
            Some('%') => ConversionType::PercentSign,
            Some(c) => return Err(PrintfParseError {
                kind: PrintfParseErrorKind::InvalidFormatSpecifier(c),
                specifier_offset,
                specifier_length: find_specifier_len(),
            }),
            None => return Err(PrintfParseError {
                kind: PrintfParseErrorKind::NoSpecifierAfterPercent,
                specifier_offset,
                specifier_length: find_specifier_len(),
            }),
        };
        // consume the conversion character
        s = &s[1..];
    }

    Ok((spec, s))
}

fn take_numeric_param(s: &str) -> (PrecisionParam, &str) {
    match s.chars().next() {
        Some('*') => (PrecisionParam::FromArgument, &s[1..]),
        Some(digit) if digit.is_ascii_digit() => {
            let mut s = s;
            let mut w: i32 = 0;
            loop {
                match s.chars().next() {
                    Some(d) if d.is_ascii_digit() => {
                        w = 10 * w + ((d as i32) - ('0' as i32));
                    }
                    _ => break,
                }
                s = &s[1..];
            }
            if w > 255 {
                // todo error
            }
            (PrecisionParam::Literal(w as u8), s)
        }
        _ => (PrecisionParam::Literal(0), s),
    }
}
