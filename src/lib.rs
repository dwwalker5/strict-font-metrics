//! Reads vertical metrics (units per em, ascender, descender, line gap) out
//! of TrueType and OpenType files by walking the sfnt table directory and
//! the `head`/`hhea` tables directly.
//!
//! By default every check the format allows for is enforced: table
//! checksums must match the directory, `head`'s magic number must be
//! correct, and table versions must be ones this parser was written
//! against. Fonts seen in the wild routinely fail one of these (hand
//! patched, subset by a lazy tool, or just old), so [`ParseOptions::lenient`]
//! turns all of that off and extracts whatever the byte layout will still
//! yield.

mod error;
mod reader;
#[cfg(test)]
mod tests;

pub use error::Error;

use reader::Reader;

const HEAD_TAG: [u8; 4] = *b"head";
const HHEA_TAG: [u8; 4] = *b"hhea";
const HEAD_MAGIC: u32 = 0x5F0F_3CF5;
const SFNT_TRUETYPE: u32 = 0x0001_0000;
const SFNT_OPENTYPE_CFF: u32 = 0x4F54_544F; // 'OTTO'
const SFNT_APPLE_TRUE: u32 = 0x7472_7565; // 'true', old Apple TrueType tag

/// Controls how much a malformed font is tolerated.
///
/// The default is strict. Call [`ParseOptions::lenient`] to get a parser
/// that skips checksum verification, magic number checks, and table
/// version checks, and instead tries to read the fields it needs anyway.
#[derive(Debug, Clone, Copy, Default)]
pub struct ParseOptions {
    lenient: bool,
}

impl ParseOptions {
    /// The default: reject fonts with bad checksums, bad magic numbers, or
    /// unrecognized table versions.
    pub fn strict() -> Self {
        ParseOptions { lenient: false }
    }

    /// Skip the checks above and extract metrics on a best-effort basis.
    pub fn lenient() -> Self {
        ParseOptions { lenient: true }
    }

    pub fn is_lenient(&self) -> bool {
        self.lenient
    }
}

/// The subset of a font's vertical metrics needed for line layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontMetrics {
    /// Size of the em square, in font design units. Every other field here
    /// is expressed in these units.
    pub units_per_em: u16,
    /// Distance from the baseline to the top of the em square (`hhea.ascender`).
    pub ascender: i16,
    /// Distance from the baseline to the bottom of the em square (`hhea.descender`,
    /// typically negative).
    pub descender: i16,
    /// Extra spacing a typesetter should add between lines (`hhea.lineGap`).
    pub line_gap: i16,
    /// Widest advance width of any glyph in the font.
    pub advance_width_max: u16,
    /// Number of entries in the `hmtx` table's advance-width array.
    pub number_of_h_metrics: u16,
}

impl FontMetrics {
    /// `ascender - descender + line_gap`, in design units: the recommended
    /// distance between successive baselines.
    pub fn line_height(&self) -> i32 {
        self.ascender as i32 - self.descender as i32 + self.line_gap as i32
    }
}

struct TableRecord {
    tag: [u8; 4],
    checksum: u32,
    offset: u32,
    length: u32,
}

/// Parse the sfnt table directory and `head`/`hhea` tables of `data` and
/// return the metrics they describe.
///
/// ```no_run
/// let data = std::fs::read("some-font.ttf").unwrap();
/// let metrics = strict_font_metrics::parse(&data, &strict_font_metrics::ParseOptions::strict());
/// ```
pub fn parse(data: &[u8], options: &ParseOptions) -> Result<FontMetrics, Error> {
    let mut r = Reader::new(data);

    let sfnt_version = r.u32()?;
    let recognized = matches!(sfnt_version, SFNT_TRUETYPE | SFNT_OPENTYPE_CFF)
        || (options.lenient && sfnt_version == SFNT_APPLE_TRUE);
    if !recognized {
        return Err(Error::BadSfntVersion(sfnt_version));
    }

    let num_tables = r.u16()?;
    r.u16()?; // searchRange
    r.u16()?; // entrySelector
    r.u16()?; // rangeShift

    let mut records = Vec::with_capacity(num_tables as usize);
    for _ in 0..num_tables {
        records.push(TableRecord {
            tag: r.tag()?,
            checksum: r.u32()?,
            offset: r.u32()?,
            length: r.u32()?,
        });
    }

    let head_record = find_table(&records, &HEAD_TAG).ok_or(Error::MissingTable("head"))?;
    let hhea_record = find_table(&records, &HHEA_TAG).ok_or(Error::MissingTable("hhea"))?;

    let head_bytes = table_bytes(data, head_record)?;
    let hhea_bytes = table_bytes(data, hhea_record)?;

    if !options.lenient {
        verify_checksum(head_record, head_bytes)?;
        verify_checksum(hhea_record, hhea_bytes)?;
    }

    let mut head = Reader::new(head_bytes);
    let head_major = head.u16()?;
    let head_minor = head.u16()?;
    if !options.lenient && (head_major, head_minor) != (1, 0) {
        return Err(Error::UnsupportedTableVersion {
            table: "head",
            major: head_major,
            minor: head_minor,
        });
    }
    head.u32()?; // fontRevision
    head.u32()?; // checkSumAdjustment
    let magic = head.u32()?;
    if !options.lenient && magic != HEAD_MAGIC {
        return Err(Error::BadMagicNumber(magic));
    }
    head.u16()?; // flags
    let units_per_em = head.u16()?;

    let mut hhea = Reader::new(hhea_bytes);
    let hhea_major = hhea.u16()?;
    let hhea_minor = hhea.u16()?;
    if !options.lenient && (hhea_major, hhea_minor) != (1, 0) {
        return Err(Error::UnsupportedTableVersion {
            table: "hhea",
            major: hhea_major,
            minor: hhea_minor,
        });
    }
    let ascender = hhea.i16()?;
    let descender = hhea.i16()?;
    let line_gap = hhea.i16()?;
    let advance_width_max = hhea.u16()?;
    hhea.i16()?; // minLeftSideBearing
    hhea.i16()?; // minRightSideBearing
    hhea.i16()?; // xMaxExtent
    hhea.i16()?; // caretSlopeRise
    hhea.i16()?; // caretSlopeRun
    hhea.i16()?; // caretOffset
    for _ in 0..4 {
        hhea.i16()?; // reserved
    }
    hhea.i16()?; // metricDataFormat
    let number_of_h_metrics = hhea.u16()?;

    Ok(FontMetrics {
        units_per_em,
        ascender,
        descender,
        line_gap,
        advance_width_max,
        number_of_h_metrics,
    })
}

fn find_table<'a>(records: &'a [TableRecord], tag: &[u8; 4]) -> Option<&'a TableRecord> {
    records.iter().find(|r| &r.tag == tag)
}

fn table_bytes<'a>(data: &'a [u8], record: &TableRecord) -> Result<&'a [u8], Error> {
    let start = record.offset as usize;
    let end = start
        .checked_add(record.length as usize)
        .filter(|&e| e <= data.len());
    match end {
        Some(end) => Ok(&data[start..end]),
        None => Err(Error::TableOutOfBounds {
            tag: record.tag,
            offset: record.offset,
            length: record.length,
            data_len: data.len(),
        }),
    }
}

fn verify_checksum(record: &TableRecord, bytes: &[u8]) -> Result<(), Error> {
    let actual = table_checksum(bytes);
    if actual != record.checksum {
        return Err(Error::ChecksumMismatch {
            tag: record.tag,
            expected: record.checksum,
            actual,
        });
    }
    Ok(())
}

/// sfnt table checksum: sum of the table's bytes read as big-endian u32
/// words, wrapping, with the final word zero-padded if the length isn't a
/// multiple of four.
fn table_checksum(data: &[u8]) -> u32 {
    let mut sum: u32 = 0;
    for chunk in data.chunks(4) {
        let mut word = [0u8; 4];
        word[..chunk.len()].copy_from_slice(chunk);
        sum = sum.wrapping_add(u32::from_be_bytes(word));
    }
    sum
}
