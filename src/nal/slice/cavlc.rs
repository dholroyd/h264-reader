//! CAVLC (Context-Adaptive Variable-Length Coding) entropy decoding for H.264
//! residual blocks, as specified in ITU-T H.264 section 9.2.

use std::fmt;

use crate::rbsp::{BitRead, BitReaderError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CavlcContext {
    NC(u8),
    ChromaDC,
}

#[derive(Debug)]
pub enum CavlcError {
    IoError(BitReaderError),
    InvalidCode(&'static str),
    OutOfRange { field: &'static str, value: i64 },
}

impl fmt::Display for CavlcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CavlcError::IoError(e) => write!(f, "bitstream I/O: {e:?}"),
            CavlcError::InvalidCode(ctx) => write!(f, "invalid VLC code: {ctx}"),
            CavlcError::OutOfRange { field, value } => {
                write!(f, "{field} out of range: {value}")
            }
        }
    }
}

impl std::error::Error for CavlcError {}

impl From<BitReaderError> for CavlcError {
    fn from(e: BitReaderError) -> Self {
        CavlcError::IoError(e)
    }
}

type CoeffTokenTable = &'static [(u8, u16, u8, u8)];

fn read_coeff_token_vlc<R: BitRead>(
    r: &mut R,
    table: CoeffTokenTable,
    name: &'static str,
) -> Result<(u8, u8), CavlcError> {
    let mut accum: u16 = 0;
    let mut bits_read: u8 = 0;
    let mut table_idx = 0;

    loop {
        let bit = r.read_bit(name)?;
        accum = (accum << 1) | (bit as u16);
        bits_read += 1;

        while table_idx < table.len() && table[table_idx].0 == bits_read {
            let (_, code_val, tc, to) = table[table_idx];
            if code_val == accum {
                return Ok((tc, to));
            }
            table_idx += 1;
        }

        if table_idx >= table.len() || bits_read > 16 {
            return Err(CavlcError::InvalidCode(name));
        }
    }
}

type ValueVlcTable = &'static [(u8, u16, u8)];

fn read_value_vlc<R: BitRead>(
    r: &mut R,
    table: ValueVlcTable,
    name: &'static str,
) -> Result<u8, CavlcError> {
    let mut accum: u16 = 0;
    let mut bits_read: u8 = 0;
    let mut table_idx = 0;

    loop {
        let bit = r.read_bit(name)?;
        accum = (accum << 1) | (bit as u16);
        bits_read += 1;

        while table_idx < table.len() && table[table_idx].0 == bits_read {
            let (_, code_val, val) = table[table_idx];
            if code_val == accum {
                return Ok(val);
            }
            table_idx += 1;
        }

        if table_idx >= table.len() || bits_read > 16 {
            return Err(CavlcError::InvalidCode(name));
        }
    }
}

fn read_coeff_token<R: BitRead>(r: &mut R, nc: CavlcContext) -> Result<(u8, u8), CavlcError> {
    match nc {
        CavlcContext::NC(n) if n >= 8 => read_coeff_token_fixed6(r),
        CavlcContext::NC(4..=7) => read_coeff_token_vlc(r, COEFF_TOKEN_4, "coeff_token (4<=nC<8)"),
        CavlcContext::NC(2..=3) => read_coeff_token_vlc(r, COEFF_TOKEN_2, "coeff_token (2<=nC<4)"),
        CavlcContext::NC(_) => read_coeff_token_vlc(r, COEFF_TOKEN_0, "coeff_token (nC<2)"),
        CavlcContext::ChromaDC => {
            read_coeff_token_vlc(r, COEFF_TOKEN_CHROMA_DC, "coeff_token (ChromaDC)")
        }
    }
}

fn read_coeff_token_fixed6<R: BitRead>(r: &mut R) -> Result<(u8, u8), CavlcError> {
    let code: u8 = r.read::<6, u8>("coeff_token")?;
    if code == 3 {
        return Ok((0, 0));
    }
    let total_coeff = (code >> 2) + 1;
    let trailing_ones = code & 3;
    if trailing_ones > core::cmp::min(3, total_coeff) {
        return Err(CavlcError::InvalidCode(
            "coeff_token nC>=8: trailing_ones > total_coeff",
        ));
    }
    Ok((total_coeff, trailing_ones))
}

// Table 9-5(a): 0 <= nC < 2
#[rustfmt::skip]
static COEFF_TOKEN_0: CoeffTokenTable = &[
    // len, code, TC, TO
    ( 1, 0b1,                    0, 0),
    ( 2, 0b01,                   1, 1),
    ( 3, 0b001,                  2, 2),
    ( 5, 0b00011,                3, 3),
    ( 6, 0b000101,               1, 0),
    ( 6, 0b000100,               2, 1),
    ( 6, 0b000011,               4, 3),
    ( 7, 0b0000101,              3, 2),
    ( 7, 0b0000100,              5, 3),
    ( 8, 0b00000111,             2, 0),
    ( 8, 0b00000110,             3, 1),
    ( 8, 0b00000101,             4, 2),
    ( 8, 0b00000100,             6, 3),
    ( 9, 0b000000111,            3, 0),
    ( 9, 0b000000110,            4, 1),
    ( 9, 0b000000101,            5, 2),
    ( 9, 0b000000100,            7, 3),
    (10, 0b0000000111,           4, 0),
    (10, 0b0000000110,           5, 1),
    (10, 0b0000000101,           6, 2),
    (10, 0b0000000100,           8, 3),
    (11, 0b00000000111,          5, 0),
    (11, 0b00000000110,          6, 1),
    (11, 0b00000000101,          7, 2),
    (11, 0b00000000100,          9, 3),
    (13, 0b0000000001111,        6, 0),
    (13, 0b0000000001110,        7, 1),
    (13, 0b0000000001101,        8, 2),
    (13, 0b0000000001100,       10, 3),
    (13, 0b0000000001011,        7, 0),
    (13, 0b0000000001010,        8, 1),
    (13, 0b0000000001001,        9, 2),
    (13, 0b0000000001000,        8, 0),
    (14, 0b00000000001111,       9, 0),
    (14, 0b00000000001110,       9, 1),
    (14, 0b00000000001101,      10, 2),
    (14, 0b00000000001100,      11, 3),
    (14, 0b00000000001011,      10, 0),
    (14, 0b00000000001010,      10, 1),
    (14, 0b00000000001001,      11, 2),
    (14, 0b00000000001000,      12, 3),
    (15, 0b000000000001111,     11, 0),
    (15, 0b000000000001110,     11, 1),
    (15, 0b000000000001101,     12, 2),
    (15, 0b000000000001100,     13, 3),
    (15, 0b000000000001011,     12, 0),
    (15, 0b000000000001010,     12, 1),
    (15, 0b000000000001001,     13, 2),
    (15, 0b000000000001000,     14, 3),
    (15, 0b000000000000001,     13, 1),
    (16, 0b0000000000001111,    13, 0),
    (16, 0b0000000000001110,    14, 1),
    (16, 0b0000000000001101,    14, 2),
    (16, 0b0000000000001100,    15, 3),
    (16, 0b0000000000001011,    14, 0),
    (16, 0b0000000000001010,    15, 1),
    (16, 0b0000000000001001,    15, 2),
    (16, 0b0000000000001000,    16, 3),
    (16, 0b0000000000000111,    15, 0),
    (16, 0b0000000000000110,    16, 1),
    (16, 0b0000000000000101,    16, 2),
    (16, 0b0000000000000100,    16, 0),
];

// Table 9-5(b): 2 <= nC < 4
#[rustfmt::skip]
static COEFF_TOKEN_2: CoeffTokenTable = &[
    ( 2, 0b11,                   0, 0),
    ( 2, 0b10,                   1, 1),
    ( 3, 0b011,                  2, 2),
    ( 4, 0b0101,                 3, 3),
    ( 4, 0b0100,                 4, 3),
    ( 5, 0b00111,                2, 1),
    ( 5, 0b00110,                5, 3),
    ( 6, 0b001011,               1, 0),
    ( 6, 0b001010,               3, 1),
    ( 6, 0b001001,               3, 2),
    ( 6, 0b001000,               6, 3),
    ( 6, 0b000111,               2, 0),
    ( 6, 0b000110,               4, 1),
    ( 6, 0b000101,               4, 2),
    ( 6, 0b000100,               7, 3),
    ( 7, 0b0000111,              3, 0),
    ( 7, 0b0000110,              5, 1),
    ( 7, 0b0000101,              5, 2),
    ( 7, 0b0000100,              8, 3),
    ( 8, 0b00000111,             4, 0),
    ( 8, 0b00000110,             6, 1),
    ( 8, 0b00000101,             6, 2),
    ( 8, 0b00000100,             5, 0),
    ( 9, 0b000000111,            6, 0),
    ( 9, 0b000000110,            7, 1),
    ( 9, 0b000000101,            7, 2),
    ( 9, 0b000000100,            9, 3),
    (11, 0b00000001111,          7, 0),
    (11, 0b00000001110,          8, 1),
    (11, 0b00000001101,          8, 2),
    (11, 0b00000001100,         10, 3),
    (11, 0b00000001011,          8, 0),
    (11, 0b00000001010,          9, 1),
    (11, 0b00000001001,          9, 2),
    (11, 0b00000001000,         11, 3),
    (12, 0b000000001111,         9, 0),
    (12, 0b000000001110,        10, 1),
    (12, 0b000000001101,        10, 2),
    (12, 0b000000001100,        12, 3),
    (12, 0b000000001011,        10, 0),
    (12, 0b000000001010,        11, 1),
    (12, 0b000000001001,        11, 2),
    (12, 0b000000001000,        11, 0),
    (13, 0b0000000001111,       12, 0),
    (13, 0b0000000001110,       12, 1),
    (13, 0b0000000001101,       12, 2),
    (13, 0b0000000001100,       13, 3),
    (13, 0b0000000001011,       13, 0),
    (13, 0b0000000001010,       13, 1),
    (13, 0b0000000001001,       13, 2),
    (13, 0b0000000001000,       14, 3),
    (13, 0b0000000000111,       14, 0),
    (13, 0b0000000000110,       14, 2),
    (13, 0b0000000000001,       15, 3),
    (14, 0b00000000001011,      14, 1),
    (14, 0b00000000001010,      15, 2),
    (14, 0b00000000001001,      15, 0),
    (14, 0b00000000001000,      15, 1),
    (14, 0b00000000000111,      16, 0),
    (14, 0b00000000000110,      16, 1),
    (14, 0b00000000000101,      16, 2),
    (14, 0b00000000000100,      16, 3),
];

// Table 9-5(c): 4 <= nC < 8
#[rustfmt::skip]
static COEFF_TOKEN_4: CoeffTokenTable = &[
    ( 4, 0b1111,                 0, 0),
    ( 4, 0b1110,                 1, 1),
    ( 4, 0b1101,                 2, 2),
    ( 4, 0b1100,                 3, 3),
    ( 4, 0b1011,                 4, 3),
    ( 4, 0b1010,                 5, 3),
    ( 4, 0b1001,                 6, 3),
    ( 4, 0b1000,                 7, 3),
    ( 5, 0b01111,                2, 1),
    ( 5, 0b01110,                3, 2),
    ( 5, 0b01101,                8, 3),
    ( 5, 0b01100,                3, 1),
    ( 5, 0b01011,                4, 2),
    ( 5, 0b01010,                4, 1),
    ( 5, 0b01001,                5, 2),
    ( 5, 0b01000,                5, 1),
    ( 6, 0b001111,               1, 0),
    ( 6, 0b001110,               6, 1),
    ( 6, 0b001101,               6, 2),
    ( 6, 0b001100,               9, 3),
    ( 6, 0b001011,               2, 0),
    ( 6, 0b001010,               7, 1),
    ( 6, 0b001001,               7, 2),
    ( 6, 0b001000,               3, 0),
    ( 7, 0b0001111,              4, 0),
    ( 7, 0b0001110,              8, 1),
    ( 7, 0b0001101,              8, 2),
    ( 7, 0b0001100,             10, 3),
    ( 7, 0b0001011,              5, 0),
    ( 7, 0b0001010,              9, 2),
    ( 7, 0b0001001,              6, 0),
    ( 7, 0b0001000,              7, 0),
    ( 8, 0b00001111,             8, 0),
    ( 8, 0b00001110,             9, 1),
    ( 8, 0b00001101,            10, 2),
    ( 8, 0b00001100,            11, 3),
    ( 8, 0b00001011,             9, 0),
    ( 8, 0b00001010,            10, 1),
    ( 8, 0b00001001,            11, 2),
    ( 8, 0b00001000,            12, 3),
    ( 9, 0b000001111,           10, 0),
    ( 9, 0b000001110,           11, 1),
    ( 9, 0b000001101,           12, 2),
    ( 9, 0b000001100,           13, 3),
    ( 9, 0b000001011,           11, 0),
    ( 9, 0b000001010,           12, 1),
    ( 9, 0b000001001,           13, 2),
    ( 9, 0b000001000,           12, 0),
    ( 9, 0b000000111,           13, 1),
    (10, 0b0000001101,          13, 0),
    (10, 0b0000001100,          14, 1),
    (10, 0b0000001011,          14, 2),
    (10, 0b0000001010,          14, 3),
    (10, 0b0000001001,          14, 0),
    (10, 0b0000001000,          15, 1),
    (10, 0b0000000111,          15, 2),
    (10, 0b0000000110,          15, 3),
    (10, 0b0000000101,          15, 0),
    (10, 0b0000000100,          16, 1),
    (10, 0b0000000011,          16, 2),
    (10, 0b0000000010,          16, 3),
    (10, 0b0000000001,          16, 0),
];

// ChromaDC table for nC == -1 (4:2:0, max 4 coeffs)
#[rustfmt::skip]
static COEFF_TOKEN_CHROMA_DC: CoeffTokenTable = &[
    ( 1, 0b1,         1, 1),
    ( 2, 0b01,        0, 0),
    ( 3, 0b001,       2, 2),
    ( 6, 0b000111,    1, 0),
    ( 6, 0b000110,    2, 1),
    ( 6, 0b000101,    3, 3),
    ( 6, 0b000100,    2, 0),
    ( 6, 0b000011,    3, 0),
    ( 6, 0b000010,    4, 0),
    ( 7, 0b0000011,   3, 1),
    ( 7, 0b0000010,   3, 2),
    ( 7, 0b0000000,   4, 3),
    ( 8, 0b00000011,  4, 1),
    ( 8, 0b00000010,  4, 2),
];

fn read_level_prefix<R: BitRead>(r: &mut R) -> Result<u32, CavlcError> {
    let mut prefix = 0u32;
    loop {
        if r.read_bit("level_prefix")? {
            return Ok(prefix);
        }
        prefix += 1;
        if prefix > 32 {
            return Err(CavlcError::InvalidCode("level_prefix exceeds 32"));
        }
    }
}

fn read_level<R: BitRead>(r: &mut R, suffix_length: u32) -> Result<i32, CavlcError> {
    let level_prefix = read_level_prefix(r)?;

    let level_suffix_size = if level_prefix == 14 && suffix_length == 0 {
        4
    } else if level_prefix >= 15 {
        level_prefix - 3
    } else {
        suffix_length
    };

    let level_suffix: u32 = if level_suffix_size > 0 {
        r.read_var(level_suffix_size, "level_suffix")?
    } else {
        0
    };

    let level_code = {
        let prefix_part = core::cmp::min(15, level_prefix) << suffix_length;
        let mut code = prefix_part + level_suffix;
        if level_prefix >= 15 && suffix_length == 0 {
            code += 15;
        }
        if level_prefix >= 16 {
            code += (1u32 << (level_prefix - 3)) - 4096;
        }
        code
    };

    let level_val = if level_code & 1 == 0 {
        (level_code as i32 >> 1) + 1
    } else {
        -(level_code as i32 >> 1) - 1
    };

    Ok(level_val)
}

fn update_suffix_length(suffix_length: &mut u32, level_val: i32) {
    let abs_level = level_val.unsigned_abs();
    if *suffix_length == 0 {
        *suffix_length = 1;
    }
    if abs_level > (3u32 << (*suffix_length - 1)) && *suffix_length < 6 {
        *suffix_length += 1;
    }
}

#[rustfmt::skip]
static TOTAL_ZEROS_1: ValueVlcTable = &[
    (1, 0b1,          0),
    (3, 0b011,        1),
    (3, 0b010,        2),
    (4, 0b0011,       3),
    (4, 0b0010,       4),
    (5, 0b00011,      5),
    (5, 0b00010,      6),
    (6, 0b000011,     7),
    (6, 0b000010,     8),
    (7, 0b0000011,    9),
    (7, 0b0000010,   10),
    (8, 0b00000011,  11),
    (8, 0b00000010,  12),
    (9, 0b000000011, 13),
    (9, 0b000000010, 14),
    (9, 0b000000001, 15),
];

#[rustfmt::skip]
static TOTAL_ZEROS_2: ValueVlcTable = &[
    (3, 0b111, 0),
    (3, 0b110, 1),
    (3, 0b101, 2),
    (3, 0b100, 3),
    (3, 0b011, 4),
    (4, 0b0101, 5),
    (4, 0b0100, 6),
    (4, 0b0011, 7),
    (4, 0b0010, 8),
    (5, 0b00011, 9),
    (5, 0b00010, 10),
    (6, 0b000011, 11),
    (6, 0b000010, 12),
    (6, 0b000001, 13),
    (6, 0b000000, 14),
];

#[rustfmt::skip]
static TOTAL_ZEROS_3: ValueVlcTable = &[
    (3, 0b111,  1),
    (3, 0b110,  2),
    (3, 0b101,  3),
    (3, 0b100,  6),
    (3, 0b011,  7),
    (4, 0b0101,  0),
    (4, 0b0100,  4),
    (4, 0b0011,  5),
    (4, 0b0010,  8),
    (5, 0b00011,  9),
    (5, 0b00010, 10),
    (5, 0b00001, 12),
    (6, 0b000001, 11),
    (6, 0b000000, 13),
];

#[rustfmt::skip]
static TOTAL_ZEROS_4: ValueVlcTable = &[
    (3, 0b111,  1),
    (3, 0b110,  4),
    (3, 0b101,  5),
    (3, 0b100,  6),
    (3, 0b011,  8),
    (4, 0b0101,  2),
    (4, 0b0100,  3),
    (4, 0b0011,  7),
    (4, 0b0010,  9),
    (5, 0b00011,  0),
    (5, 0b00010, 10),
    (5, 0b00001, 11),
    (5, 0b00000, 12),
];

#[rustfmt::skip]
static TOTAL_ZEROS_5: ValueVlcTable = &[
    (3, 0b111,  3),
    (3, 0b110,  4),
    (3, 0b101,  5),
    (3, 0b100,  6),
    (3, 0b011,  7),
    (4, 0b0101,  0),
    (4, 0b0100,  1),
    (4, 0b0011,  2),
    (4, 0b0010,  8),
    (4, 0b0001, 10),
    (5, 0b00001,  9),
    (5, 0b00000, 11),
];

#[rustfmt::skip]
static TOTAL_ZEROS_6: ValueVlcTable = &[
    (3, 0b111,  2),
    (3, 0b110,  3),
    (3, 0b101,  4),
    (3, 0b100,  5),
    (3, 0b011,  6),
    (3, 0b010,  7),
    (3, 0b001,  9),
    (4, 0b0001,  8),
    (5, 0b00001,  1),
    (6, 0b000001,  0),
    (6, 0b000000, 10),
];

#[rustfmt::skip]
static TOTAL_ZEROS_7: ValueVlcTable = &[
    (2, 0b11,  5),
    (3, 0b101,  2),
    (3, 0b100,  3),
    (3, 0b011,  4),
    (3, 0b010,  6),
    (3, 0b001,  8),
    (4, 0b0001,  7),
    (5, 0b00001,  1),
    (6, 0b000001,  0),
    (6, 0b000000,  9),
];

#[rustfmt::skip]
static TOTAL_ZEROS_8: ValueVlcTable = &[
    (2, 0b11,  4),
    (2, 0b10,  5),
    (3, 0b011,  3),
    (3, 0b010,  6),
    (3, 0b001,  7),
    (4, 0b0001,  1),
    (5, 0b00001,  2),
    (6, 0b000001,  0),
    (6, 0b000000,  8),
];

#[rustfmt::skip]
static TOTAL_ZEROS_9: ValueVlcTable = &[
    (2, 0b11,  3),
    (2, 0b10,  4),
    (2, 0b01,  6),
    (3, 0b001,  5),
    (4, 0b0001,  2),
    (5, 0b00001,  7),
    (6, 0b000001,  0),
    (6, 0b000000,  1),
];

#[rustfmt::skip]
static TOTAL_ZEROS_10: ValueVlcTable = &[
    (2, 0b11,  3),
    (2, 0b10,  4),
    (2, 0b01,  5),
    (3, 0b001,  2),
    (4, 0b0001,  6),
    (5, 0b00001,  0),
    (5, 0b00000,  1),
];

#[rustfmt::skip]
static TOTAL_ZEROS_11: ValueVlcTable = &[
    (1, 0b1,  4),
    (3, 0b011,  5),
    (3, 0b010,  3),
    (3, 0b001,  2),
    (4, 0b0001,  1),
    (4, 0b0000,  0),
];

#[rustfmt::skip]
static TOTAL_ZEROS_12: ValueVlcTable = &[
    (1, 0b1,  3),
    (2, 0b01,  2),
    (3, 0b001,  4),
    (4, 0b0001,  1),
    (4, 0b0000,  0),
];

#[rustfmt::skip]
static TOTAL_ZEROS_13: ValueVlcTable = &[
    (1, 0b1,  2),
    (2, 0b01,  3),
    (3, 0b001,  1),
    (3, 0b000,  0),
];

#[rustfmt::skip]
static TOTAL_ZEROS_14: ValueVlcTable = &[
    (1, 0b1,  2),
    (2, 0b01,  1),
    (2, 0b00,  0),
];

#[rustfmt::skip]
static TOTAL_ZEROS_15: ValueVlcTable = &[
    (1, 0b1,  1),
    (1, 0b0,  0),
];

fn total_zeros_table_16(total_coeff: u8) -> Result<ValueVlcTable, CavlcError> {
    match total_coeff {
        1 => Ok(TOTAL_ZEROS_1),
        2 => Ok(TOTAL_ZEROS_2),
        3 => Ok(TOTAL_ZEROS_3),
        4 => Ok(TOTAL_ZEROS_4),
        5 => Ok(TOTAL_ZEROS_5),
        6 => Ok(TOTAL_ZEROS_6),
        7 => Ok(TOTAL_ZEROS_7),
        8 => Ok(TOTAL_ZEROS_8),
        9 => Ok(TOTAL_ZEROS_9),
        10 => Ok(TOTAL_ZEROS_10),
        11 => Ok(TOTAL_ZEROS_11),
        12 => Ok(TOTAL_ZEROS_12),
        13 => Ok(TOTAL_ZEROS_13),
        14 => Ok(TOTAL_ZEROS_14),
        15 => Ok(TOTAL_ZEROS_15),
        _ => Err(CavlcError::OutOfRange {
            field: "total_coeff for total_zeros",
            value: total_coeff as i64,
        }),
    }
}

// Table 9-9(a): total_zeros for maxNumCoeff=4 (Chroma DC 4:2:0)

#[rustfmt::skip]
static TOTAL_ZEROS_CHROMA_DC_1: ValueVlcTable = &[
    (1, 0b1,   0),
    (2, 0b01,  1),
    (3, 0b001, 2),
    (3, 0b000, 3),
];

#[rustfmt::skip]
static TOTAL_ZEROS_CHROMA_DC_2: ValueVlcTable = &[
    (1, 0b1,  0),
    (2, 0b01, 1),
    (2, 0b00, 2),
];

#[rustfmt::skip]
static TOTAL_ZEROS_CHROMA_DC_3: ValueVlcTable = &[
    (1, 0b1, 0),
    (1, 0b0, 1),
];

fn total_zeros_table_4(total_coeff: u8) -> Result<ValueVlcTable, CavlcError> {
    match total_coeff {
        1 => Ok(TOTAL_ZEROS_CHROMA_DC_1),
        2 => Ok(TOTAL_ZEROS_CHROMA_DC_2),
        3 => Ok(TOTAL_ZEROS_CHROMA_DC_3),
        _ => Err(CavlcError::OutOfRange {
            field: "total_coeff for chroma_dc total_zeros",
            value: total_coeff as i64,
        }),
    }
}

fn read_total_zeros<R: BitRead>(
    r: &mut R,
    total_coeff: u8,
    max_num_coeff: usize,
) -> Result<u8, CavlcError> {
    let table = if max_num_coeff == 4 {
        total_zeros_table_4(total_coeff)?
    } else {
        total_zeros_table_16(total_coeff)?
    };
    read_value_vlc(r, table, "total_zeros")
}

// Table 9-10

#[rustfmt::skip]
static RUN_BEFORE_1: ValueVlcTable = &[
    (1, 0b1, 0),
    (1, 0b0, 1),
];

#[rustfmt::skip]
static RUN_BEFORE_2: ValueVlcTable = &[
    (1, 0b1, 0),
    (2, 0b01, 1),
    (2, 0b00, 2),
];

#[rustfmt::skip]
static RUN_BEFORE_3: ValueVlcTable = &[
    (2, 0b11, 0),
    (2, 0b10, 1),
    (2, 0b01, 2),
    (2, 0b00, 3),
];

#[rustfmt::skip]
static RUN_BEFORE_4: ValueVlcTable = &[
    (2, 0b11, 0),
    (2, 0b10, 1),
    (2, 0b01, 2),
    (3, 0b001, 3),
    (3, 0b000, 4),
];

#[rustfmt::skip]
static RUN_BEFORE_5: ValueVlcTable = &[
    (2, 0b11, 0),
    (2, 0b10, 1),
    (3, 0b011, 2),
    (3, 0b010, 3),
    (3, 0b001, 4),
    (3, 0b000, 5),
];

// Table 9-10, zerosLeft=6
#[rustfmt::skip]
static RUN_BEFORE_6: ValueVlcTable = &[
    (2, 0b11,  0),
    (3, 0b101,  5),
    (3, 0b100,  6),
    (3, 0b011,  3),
    (3, 0b010,  4),
    (3, 0b001,  2),
    (3, 0b000,  1),
];

// Table 9-10, zerosLeft > 6
#[rustfmt::skip]
static RUN_BEFORE_GT6: ValueVlcTable = &[
    (3, 0b111,  0),
    (3, 0b110,  1),
    (3, 0b101,  2),
    (3, 0b100,  3),
    (3, 0b011,  4),
    (3, 0b010,  5),
    (3, 0b001,  6),
    (4, 0b0001,  7),
    (5, 0b00001,  8),
    (6, 0b000001,  9),
    (7, 0b0000001, 10),
    (8, 0b00000001, 11),
    (9, 0b000000001, 12),
    (10, 0b0000000001, 13),
    (11, 0b00000000001, 14),
];

fn read_run_before<R: BitRead>(r: &mut R, zeros_left: u8) -> Result<u8, CavlcError> {
    if zeros_left == 0 {
        return Ok(0);
    }

    let table: ValueVlcTable = match zeros_left {
        1 => RUN_BEFORE_1,
        2 => RUN_BEFORE_2,
        3 => RUN_BEFORE_3,
        4 => RUN_BEFORE_4,
        5 => RUN_BEFORE_5,
        6 => RUN_BEFORE_6,
        _ => RUN_BEFORE_GT6,
    };
    read_value_vlc(r, table, "run_before")
}

pub(crate) fn residual_block_cavlc<R: BitRead>(
    r: &mut R,
    coeff_level: &mut [i32],
    start_idx: usize,
    end_idx: usize,
    max_num_coeff: usize,
    nc: CavlcContext,
) -> Result<u8, CavlcError> {
    // Zero the output range.
    for c in coeff_level[start_idx..=end_idx].iter_mut() {
        *c = 0;
    }

    let (total_coeff, trailing_ones) = read_coeff_token(r, nc)?;

    if total_coeff == 0 {
        return Ok(0);
    }

    if total_coeff as usize > max_num_coeff {
        return Err(CavlcError::OutOfRange {
            field: "TotalCoeff",
            value: total_coeff as i64,
        });
    }

    // coefficients in reverse scan order (highest frequency first)
    let mut levels = [0i32; 16];

    for item in levels.iter_mut().take(trailing_ones as usize) {
        let negative = r.read_bit("trailing_ones_sign_flag")?;
        *item = if negative { -1 } else { 1 };
    }

    let remaining = total_coeff as usize - trailing_ones as usize;

    let mut suffix_length: u32 = if total_coeff > 10 && trailing_ones < 3 {
        1
    } else {
        0
    };

    let level_start = trailing_ones as usize;
    for i in 0..remaining {
        let mut level_val = read_level(r, suffix_length)?;

        if i == 0 && trailing_ones < 3 {
            if level_val > 0 {
                level_val += 1;
            } else {
                level_val -= 1;
            }
        }

        levels[level_start + i] = level_val;

        update_suffix_length(&mut suffix_length, level_val);
    }

    let total_zeros = if (total_coeff as usize) < max_num_coeff {
        read_total_zeros(r, total_coeff, max_num_coeff)?
    } else {
        0
    };
    if (total_coeff as usize) + (total_zeros as usize) > max_num_coeff {
        return Err(CavlcError::OutOfRange {
            field: "total_coeff + total_zeros exceeds max_num_coeff",
            value: (total_coeff as i64) + (total_zeros as i64),
        });
    }

    let mut runs = [0u8; 16];
    let mut zeros_left = total_zeros;

    for item in runs.iter_mut().take(total_coeff as usize - 1) {
        if zeros_left > 0 {
            *item = read_run_before(r, zeros_left)?;
            zeros_left = zeros_left
                .checked_sub(*item)
                .ok_or(CavlcError::OutOfRange {
                    field: "zeros_left underflow in run_before",
                    value: -(*item as i64),
                })?;
        }
    }
    // last coefficient absorbs any remaining zeros
    runs[total_coeff as usize - 1] = zeros_left;

    let mut coeff_idx = (total_coeff as usize + total_zeros as usize)
        .checked_sub(1)
        .ok_or(CavlcError::OutOfRange {
            field: "coefficient position",
            value: 0,
        })?;

    for i in 0..total_coeff as usize {
        let pos = start_idx + coeff_idx;
        if pos > end_idx {
            return Err(CavlcError::OutOfRange {
                field: "coefficient position exceeds end_idx",
                value: pos as i64,
            });
        }
        coeff_level[pos] = levels[i];

        if i < total_coeff as usize - 1 {
            coeff_idx =
                coeff_idx
                    .checked_sub(1 + runs[i] as usize)
                    .ok_or(CavlcError::OutOfRange {
                        field: "coeff_idx underflow",
                        value: -(1i64),
                    })?;
        }
    }
    Ok(total_coeff)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check_prefix_free_coeff_token(table: CoeffTokenTable, name: &str) {
        for i in 0..table.len() {
            for j in (i + 1)..table.len() {
                let (len_a, bits_a, tc_a, to_a) = table[i];
                let (len_b, bits_b, tc_b, to_b) = table[j];
                if len_a == len_b {
                    // Same length: just check they differ in bits (no duplicate codes)
                    assert_ne!(
                        bits_a, bits_b,
                        "{name}: duplicate code at entries {i} (TC={tc_a},TO={to_a}) and {j} (TC={tc_b},TO={to_b}), len={len_a} bits={bits_a:#b}"
                    );
                } else {
                    // Different length: check neither is a prefix of the other
                    let (short_len, short_bits, long_len, long_bits) = if len_a < len_b {
                        (len_a, bits_a, len_b, bits_b)
                    } else {
                        (len_b, bits_b, len_a, bits_a)
                    };
                    let shifted = long_bits >> (long_len - short_len);
                    assert_ne!(
                        shifted, short_bits,
                        "{name}: prefix collision between entry {i} (len={len_a},bits={bits_a:#b},TC={tc_a},TO={to_a}) and entry {j} (len={len_b},bits={bits_b:#b},TC={tc_b},TO={to_b})"
                    );
                }
            }
        }
    }

    fn check_coeff_token_coverage(table: CoeffTokenTable, max_num_coeff: u8, name: &str) {
        let mut seen_pairs = std::collections::HashSet::new();
        let mut seen_codes = std::collections::HashSet::new();
        for (idx, &(len, bits, tc, to)) in table.iter().enumerate() {
            assert!(len > 0, "{}[{}]: zero-length code", name, idx);
            assert!(
                (bits as u32) < (1u32 << len),
                "{}[{}]: bits {:#b} don't fit in {} bits",
                name,
                idx,
                bits,
                len
            );
            assert!(
                to <= tc.min(3),
                "{}[{}]: TO={} > min(3, TC={})",
                name,
                idx,
                to,
                tc
            );
            assert!(
                tc <= max_num_coeff,
                "{}[{}]: TC={} > max={}",
                name,
                idx,
                tc,
                max_num_coeff
            );
            assert!(
                seen_pairs.insert((tc, to)),
                "{}[{}]: duplicate (TC={}, TO={})",
                name,
                idx,
                tc,
                to
            );
            assert!(
                seen_codes.insert((len, bits)),
                "{}[{}]: duplicate code (len={}, bits={:#b})",
                name,
                idx,
                len,
                bits
            );
        }
        // Check all valid (TC, TO) pairs are present
        for tc in 0..=max_num_coeff {
            for to in 0..=tc.min(3) {
                assert!(
                    seen_pairs.contains(&(tc, to)),
                    "{}: missing (TC={}, TO={})",
                    name,
                    tc,
                    to
                );
            }
        }
    }

    #[test]
    fn coeff_token_tables_are_prefix_free() {
        check_prefix_free_coeff_token(COEFF_TOKEN_0, "COEFF_TOKEN_0");
        check_prefix_free_coeff_token(COEFF_TOKEN_2, "COEFF_TOKEN_2");
        check_prefix_free_coeff_token(COEFF_TOKEN_4, "COEFF_TOKEN_4");
        check_prefix_free_coeff_token(COEFF_TOKEN_CHROMA_DC, "COEFF_TOKEN_CHROMA_DC");
    }

    #[test]
    fn coeff_token_0_complete_coverage() {
        check_coeff_token_coverage(COEFF_TOKEN_0, 16, "COEFF_TOKEN_0");
    }

    #[test]
    fn coeff_token_2_complete_coverage() {
        check_coeff_token_coverage(COEFF_TOKEN_2, 16, "COEFF_TOKEN_2");
    }

    #[test]
    fn coeff_token_4_complete_coverage() {
        check_coeff_token_coverage(COEFF_TOKEN_4, 16, "COEFF_TOKEN_4");
    }

    #[test]
    fn coeff_token_chroma_dc_complete_coverage() {
        check_coeff_token_coverage(COEFF_TOKEN_CHROMA_DC, 4, "COEFF_TOKEN_CHROMA_DC");
    }

    fn check_prefix_free_value_vlc(table: ValueVlcTable, name: &str) {
        for i in 0..table.len() {
            for j in (i + 1)..table.len() {
                let (len_a, bits_a, val_a) = table[i];
                let (len_b, bits_b, val_b) = table[j];
                if len_a == len_b {
                    assert_ne!(
                        bits_a, bits_b,
                        "{name}: duplicate code at entries {i} (val={val_a}) and {j} (val={val_b}), len={len_a} bits={bits_a:#b}"
                    );
                } else {
                    let (short_len, short_bits, long_len, long_bits) = if len_a < len_b {
                        (len_a, bits_a, len_b, bits_b)
                    } else {
                        (len_b, bits_b, len_a, bits_a)
                    };
                    let shifted = long_bits >> (long_len - short_len);
                    assert_ne!(
                        shifted, short_bits,
                        "{name}: prefix collision between entry {i} (len={len_a},bits={bits_a:#b},val={val_a}) and entry {j} (len={len_b},bits={bits_b:#b},val={val_b})"
                    );
                }
            }
        }
    }

    fn check_value_vlc_coverage(table: ValueVlcTable, max_val: u8, name: &str) {
        let mut seen_vals = std::collections::HashSet::new();
        let mut seen_codes = std::collections::HashSet::new();
        for (idx, &(len, bits, val)) in table.iter().enumerate() {
            assert!(len > 0, "{}[{}]: zero-length code", name, idx);
            assert!(
                (bits as u32) < (1u32 << len),
                "{}[{}]: bits {:#b} don't fit in {} bits",
                name,
                idx,
                bits,
                len
            );
            assert!(
                val <= max_val,
                "{}[{}]: val={} > max={}",
                name,
                idx,
                val,
                max_val
            );
            assert!(
                seen_vals.insert(val),
                "{}[{}]: duplicate value {}",
                name,
                idx,
                val
            );
            assert!(
                seen_codes.insert((len, bits)),
                "{}[{}]: duplicate code (len={}, bits={:#b})",
                name,
                idx,
                len,
                bits
            );
        }
        for v in 0..=max_val {
            assert!(seen_vals.contains(&v), "{}: missing value {}", name, v);
        }
    }

    #[test]
    fn total_zeros_luma_tables_complete_and_prefix_free() {
        let tables: &[(ValueVlcTable, u8, &str)] = &[
            (TOTAL_ZEROS_1, 15, "TOTAL_ZEROS_1"),
            (TOTAL_ZEROS_2, 14, "TOTAL_ZEROS_2"),
            (TOTAL_ZEROS_3, 13, "TOTAL_ZEROS_3"),
            (TOTAL_ZEROS_4, 12, "TOTAL_ZEROS_4"),
            (TOTAL_ZEROS_5, 11, "TOTAL_ZEROS_5"),
            (TOTAL_ZEROS_6, 10, "TOTAL_ZEROS_6"),
            (TOTAL_ZEROS_7, 9, "TOTAL_ZEROS_7"),
            (TOTAL_ZEROS_8, 8, "TOTAL_ZEROS_8"),
            (TOTAL_ZEROS_9, 7, "TOTAL_ZEROS_9"),
            (TOTAL_ZEROS_10, 6, "TOTAL_ZEROS_10"),
            (TOTAL_ZEROS_11, 5, "TOTAL_ZEROS_11"),
            (TOTAL_ZEROS_12, 4, "TOTAL_ZEROS_12"),
            (TOTAL_ZEROS_13, 3, "TOTAL_ZEROS_13"),
            (TOTAL_ZEROS_14, 2, "TOTAL_ZEROS_14"),
            (TOTAL_ZEROS_15, 1, "TOTAL_ZEROS_15"),
        ];
        for &(table, max_val, name) in tables {
            check_prefix_free_value_vlc(table, name);
            check_value_vlc_coverage(table, max_val, name);
        }
    }

    #[test]
    fn total_zeros_chroma_dc_tables_complete_and_prefix_free() {
        let tables: &[(ValueVlcTable, u8, &str)] = &[
            (TOTAL_ZEROS_CHROMA_DC_1, 3, "TOTAL_ZEROS_CHROMA_DC_1"),
            (TOTAL_ZEROS_CHROMA_DC_2, 2, "TOTAL_ZEROS_CHROMA_DC_2"),
            (TOTAL_ZEROS_CHROMA_DC_3, 1, "TOTAL_ZEROS_CHROMA_DC_3"),
        ];
        for &(table, max_val, name) in tables {
            check_prefix_free_value_vlc(table, name);
            check_value_vlc_coverage(table, max_val, name);
        }
    }

    #[test]
    fn run_before_tables_complete_and_prefix_free() {
        let tables: &[(ValueVlcTable, u8, &str)] = &[
            (RUN_BEFORE_1, 1, "RUN_BEFORE_1"),
            (RUN_BEFORE_2, 2, "RUN_BEFORE_2"),
            (RUN_BEFORE_3, 3, "RUN_BEFORE_3"),
            (RUN_BEFORE_4, 4, "RUN_BEFORE_4"),
            (RUN_BEFORE_5, 5, "RUN_BEFORE_5"),
            (RUN_BEFORE_6, 6, "RUN_BEFORE_6"),
            (RUN_BEFORE_GT6, 14, "RUN_BEFORE_GT6"),
        ];
        for &(table, max_val, name) in tables {
            check_prefix_free_value_vlc(table, name);
            check_value_vlc_coverage(table, max_val, name);
        }
    }

    #[test]
    fn coeff_token_tables_sorted_by_length() {
        for (table, name) in [
            (COEFF_TOKEN_0, "COEFF_TOKEN_0"),
            (COEFF_TOKEN_2, "COEFF_TOKEN_2"),
            (COEFF_TOKEN_4, "COEFF_TOKEN_4"),
            (COEFF_TOKEN_CHROMA_DC, "COEFF_TOKEN_CHROMA_DC"),
        ] {
            for i in 1..table.len() {
                assert!(
                    table[i].0 >= table[i - 1].0,
                    "{name}: not sorted by length at index {i}: {} < {}",
                    table[i].0,
                    table[i - 1].0
                );
            }
        }
    }

    #[test]
    fn value_vlc_tables_sorted_by_length() {
        let tables: &[(ValueVlcTable, &str)] = &[
            (TOTAL_ZEROS_1, "TOTAL_ZEROS_1"),
            (TOTAL_ZEROS_2, "TOTAL_ZEROS_2"),
            (TOTAL_ZEROS_3, "TOTAL_ZEROS_3"),
            (TOTAL_ZEROS_4, "TOTAL_ZEROS_4"),
            (TOTAL_ZEROS_5, "TOTAL_ZEROS_5"),
            (TOTAL_ZEROS_6, "TOTAL_ZEROS_6"),
            (TOTAL_ZEROS_7, "TOTAL_ZEROS_7"),
            (TOTAL_ZEROS_8, "TOTAL_ZEROS_8"),
            (TOTAL_ZEROS_9, "TOTAL_ZEROS_9"),
            (TOTAL_ZEROS_10, "TOTAL_ZEROS_10"),
            (TOTAL_ZEROS_11, "TOTAL_ZEROS_11"),
            (TOTAL_ZEROS_12, "TOTAL_ZEROS_12"),
            (TOTAL_ZEROS_13, "TOTAL_ZEROS_13"),
            (TOTAL_ZEROS_14, "TOTAL_ZEROS_14"),
            (TOTAL_ZEROS_15, "TOTAL_ZEROS_15"),
            (TOTAL_ZEROS_CHROMA_DC_1, "TOTAL_ZEROS_CHROMA_DC_1"),
            (TOTAL_ZEROS_CHROMA_DC_2, "TOTAL_ZEROS_CHROMA_DC_2"),
            (TOTAL_ZEROS_CHROMA_DC_3, "TOTAL_ZEROS_CHROMA_DC_3"),
            (RUN_BEFORE_1, "RUN_BEFORE_1"),
            (RUN_BEFORE_2, "RUN_BEFORE_2"),
            (RUN_BEFORE_3, "RUN_BEFORE_3"),
            (RUN_BEFORE_4, "RUN_BEFORE_4"),
            (RUN_BEFORE_5, "RUN_BEFORE_5"),
            (RUN_BEFORE_6, "RUN_BEFORE_6"),
            (RUN_BEFORE_GT6, "RUN_BEFORE_GT6"),
        ];
        for &(table, name) in tables {
            for i in 1..table.len() {
                assert!(
                    table[i].0 >= table[i - 1].0,
                    "{name}: not sorted by length at index {i}: {} < {}",
                    table[i].0,
                    table[i - 1].0
                );
            }
        }
    }
}
