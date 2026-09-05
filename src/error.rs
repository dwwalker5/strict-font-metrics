use std::fmt;

/// Everything that can go wrong while reading an sfnt-based font.
///
/// Several of these are only raised in strict mode (the default); see
/// [`crate::ParseOptions`] for which checks `lenient()` turns off.
#[derive(Debug)]
pub enum Error {
    /// The reader ran past the end of the buffer.
    TooShort { needed: usize, available: usize },
    /// The first four bytes of the file are not a version tag we recognize.
    BadSfntVersion(u32),
    /// A table required to compute metrics was not present in the directory.
    MissingTable(&'static str),
    /// A table directory entry points outside the bounds of the file.
    TableOutOfBounds {
        tag: [u8; 4],
        offset: u32,
        length: u32,
        data_len: usize,
    },
    /// The bytes of a table do not hash to the checksum recorded for it.
    ChecksumMismatch {
        tag: [u8; 4],
        expected: u32,
        actual: u32,
    },
    /// The `head` table's magic number is not `0x5F0F3CF5`.
    BadMagicNumber(u32),
    /// A table declares a major/minor version this parser does not accept.
    UnsupportedTableVersion {
        table: &'static str,
        major: u16,
        minor: u16,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::TooShort { needed, available } => write!(
                f,
                "unexpected end of data: needed {needed} bytes, had {available}"
            ),
            Error::BadSfntVersion(v) => write!(f, "unrecognized sfnt version 0x{v:08X}"),
            Error::MissingTable(tag) => write!(f, "font is missing required table '{tag}'"),
            Error::TableOutOfBounds {
                tag,
                offset,
                length,
                data_len,
            } => write!(
                f,
                "table '{}' at offset {offset} length {length} extends past end of file ({data_len} bytes)",
                tag_str(tag)
            ),
            Error::ChecksumMismatch {
                tag,
                expected,
                actual,
            } => write!(
                f,
                "table '{}' checksum mismatch: directory says 0x{expected:08X}, computed 0x{actual:08X}",
                tag_str(tag)
            ),
            Error::BadMagicNumber(v) => {
                write!(f, "head table magic number is 0x{v:08X}, expected 0x5F0F3CF5")
            }
            Error::UnsupportedTableVersion {
                table,
                major,
                minor,
            } => write!(f, "unsupported '{table}' table version {major}.{minor}"),
        }
    }
}

impl std::error::Error for Error {}

fn tag_str(tag: &[u8; 4]) -> String {
    tag.iter()
        .map(|&b| if b.is_ascii_graphic() { b as char } else { '.' })
        .collect()
}
