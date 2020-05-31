use rustc_ast::ast::{Lit, LitFloatType, LitIntType, LitKind};

#[derive(Debug, PartialEq)]
pub enum Radix {
    Binary,
    Octal,
    Decimal,
    Hexadecimal,
}

impl Radix {
    /// Returns a reasonable digit group size for this radix.
    #[must_use]
    fn suggest_grouping(&self) -> usize {
        match *self {
            Self::Binary | Self::Hexadecimal => 4,
            Self::Octal | Self::Decimal => 3,
        }
    }
}

/// A helper method to format numeric literals with digit grouping.
/// `lit` must be a valid numeric literal without suffix.
pub fn format(lit: &str, type_suffix: Option<&str>, float: bool) -> String {
    NumericLiteral::new(lit, type_suffix, float).format()
}

#[derive(Debug)]
pub struct NumericLiteral<'a> {
    /// Which radix the literal was represented in.
    pub radix: Radix,
    /// The radix prefix, if present.
    pub prefix: Option<&'a str>,

    /// The integer part of the number.
    pub integer: &'a str,
    /// The fraction part of the number.
    pub fraction: Option<&'a str>,
    /// The character used as exponent seperator (b'e' or b'E') and the exponent part.
    pub exponent: Option<(char, &'a str)>,

    /// The type suffix, including preceding underscore if present.
    pub suffix: Option<&'a str>,
}

impl<'a> NumericLiteral<'a> {
    pub fn from_lit(src: &'a str, lit: &Lit) -> Option<NumericLiteral<'a>> {
        NumericLiteral::from_lit_kind(src, &lit.kind)
    }

    pub fn from_lit_kind(src: &'a str, lit_kind: &LitKind) -> Option<NumericLiteral<'a>> {
        if lit_kind.is_numeric() && src.chars().next().map_or(false, |c| c.is_digit(10)) {
            let (unsuffixed, suffix) = split_suffix(&src, lit_kind);
            let float = if let LitKind::Float(..) = lit_kind { true } else { false };
            Some(NumericLiteral::new(unsuffixed, suffix, float))
        } else {
            None
        }
    }

    #[must_use]
    pub fn new(lit: &'a str, suffix: Option<&'a str>, float: bool) -> Self {
        // Determine delimiter for radix prefix, if present, and radix.
        let radix = if lit.starts_with("0x") {
            Radix::Hexadecimal
        } else if lit.starts_with("0b") {
            Radix::Binary
        } else if lit.starts_with("0o") {
            Radix::Octal
        } else {
            Radix::Decimal
        };

        // Grab part of the literal after prefix, if present.
        let (prefix, mut sans_prefix) = if let Radix::Decimal = radix {
            (None, lit)
        } else {
            let (p, s) = lit.split_at(2);
            (Some(p), s)
        };

        if suffix.is_some() && sans_prefix.ends_with('_') {
            // The '_' before the suffix isn't part of the digits
            sans_prefix = &sans_prefix[..sans_prefix.len() - 1];
        }

        let (integer, fraction, exponent) = Self::split_digit_parts(sans_prefix, float);

        Self {
            radix,
            prefix,
            integer,
            fraction,
            exponent,
            suffix,
        }
    }

    pub fn is_decimal(&self) -> bool {
        self.radix == Radix::Decimal
    }

    pub fn split_digit_parts(digits: &str, float: bool) -> (&str, Option<&str>, Option<(char, &str)>) {
        let mut integer = digits;
        let mut fraction = None;
        let mut exponent = None;

        if float {
            for (i, c) in digits.char_indices() {
                match c {
                    '.' => {
                        integer = &digits[..i];
                        fraction = Some(&digits[i + 1..]);
                    },
                    'e' | 'E' => {
                        if integer.len() > i {
                            integer = &digits[..i];
                        } else {
                            fraction = Some(&digits[integer.len() + 1..i]);
                        };
                        exponent = Some((c, &digits[i + 1..]));
                        break;
                    },
                    _ => {},
                }
            }
        }

        (integer, fraction, exponent)
    }

    /// Returns literal formatted in a sensible way.
    pub fn format(&self) -> String {
        let mut output = String::new();

        if let Some(prefix) = self.prefix {
            output.push_str(prefix);
        }

        let group_size = self.radix.suggest_grouping();

        Self::group_digits(
            &mut output,
            self.integer,
            group_size,
            true,
            self.radix == Radix::Hexadecimal,
        );

        if let Some(fraction) = self.fraction {
            output.push('.');
            Self::group_digits(&mut output, fraction, group_size, false, false);
        }

        if let Some((separator, exponent)) = self.exponent {
            output.push(separator);
            Self::group_digits(&mut output, exponent, group_size, true, false);
        }

        if let Some(suffix) = self.suffix {
            output.push('_');
            output.push_str(suffix);
        }

        output
    }

    pub fn group_digits(output: &mut String, input: &str, group_size: usize, partial_group_first: bool, pad: bool) {
        debug_assert!(group_size > 0);

        let mut digits = input.chars().filter(|&c| c != '_');

        let first_group_size;

        if partial_group_first {
            first_group_size = (digits.clone().count() - 1) % group_size + 1;
            if pad {
                for _ in 0..group_size - first_group_size {
                    output.push('0');
                }
            }
        } else {
            first_group_size = group_size;
        }

        for _ in 0..first_group_size {
            if let Some(digit) = digits.next() {
                output.push(digit);
            }
        }

        for (c, i) in digits.zip((0..group_size).cycle()) {
            if i == 0 {
                output.push('_');
            }
            output.push(c);
        }
    }
}

fn split_suffix<'a>(src: &'a str, lit_kind: &LitKind) -> (&'a str, Option<&'a str>) {
    debug_assert!(lit_kind.is_numeric());
    lit_suffix_length(lit_kind).map_or((src, None), |suffix_length| {
        let (unsuffixed, suffix) = src.split_at(src.len() - suffix_length);
        (unsuffixed, Some(suffix))
    })
}

fn lit_suffix_length(lit_kind: &LitKind) -> Option<usize> {
    debug_assert!(lit_kind.is_numeric());
    let suffix = match lit_kind {
        LitKind::Int(_, int_lit_kind) => match int_lit_kind {
            LitIntType::Signed(int_ty) => Some(int_ty.name_str()),
            LitIntType::Unsigned(uint_ty) => Some(uint_ty.name_str()),
            LitIntType::Unsuffixed => None,
        },
        LitKind::Float(_, float_lit_kind) => match float_lit_kind {
            LitFloatType::Suffixed(float_ty) => Some(float_ty.name_str()),
            LitFloatType::Unsuffixed => None,
        },
        _ => None,
    };

    suffix.map(str::len)
}
