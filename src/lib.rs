//! Reads vertical metrics (units per em, ascender, descender, line gap) and
//! per-glyph advance widths out of TrueType and OpenType files by walking
//! the sfnt table directory and the `head`/`hhea`/`OS/2`/`maxp`/`hmtx`
//! tables directly.
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
const OS2_TAG: [u8; 4] = *b"OS/2";
const MAXP_TAG: [u8; 4] = *b"maxp";
const HMTX_TAG: [u8; 4] = *b"hmtx";
const HEAD_MAGIC: u32 = 0x5F0F_3CF5;
const OS2_MAX_KNOWN_VERSION: u16 = 5;
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
    /// Recommended ascender for Windows text clipping (`OS/2.sTypoAscender`).
    pub typo_ascender: i16,
    /// Recommended descender for Windows text clipping (`OS/2.sTypoDescender`,
    /// typically negative).
    pub typo_descender: i16,
    /// Recommended line gap to pair with the typo metrics (`OS/2.sTypoLineGap`).
    pub typo_line_gap: i16,
    /// Windows-specific ascent used for glyph clipping (`OS/2.usWinAscent`).
    pub win_ascent: u16,
    /// Windows-specific descent used for glyph clipping (`OS/2.usWinDescent`,
    /// stored unsigned even though it measures below the baseline).
    pub win_descent: u16,
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

/// Parse the sfnt table directory and `head`/`hhea`/`OS/2` tables of `data`
/// and return the metrics they describe.
///
/// ```no_run
/// let data = std::fs::read("some-font.ttf").unwrap();
/// let metrics = strict_font_metrics::parse(&data, &strict_font_metrics::ParseOptions::strict());
/// ```
pub fn parse(data: &[u8], options: &ParseOptions) -> Result<FontMetrics, Error> {
    let records = read_table_directory(data, options)?;

    let head_record = find_table(&records, &HEAD_TAG).ok_or(Error::MissingTable("head"))?;
    let hhea_record = find_table(&records, &HHEA_TAG).ok_or(Error::MissingTable("hhea"))?;
    let os2_record = find_table(&records, &OS2_TAG).ok_or(Error::MissingTable("OS/2"))?;

    let head_bytes = table_bytes(data, head_record)?;
    let hhea_bytes = table_bytes(data, hhea_record)?;
    let os2_bytes = table_bytes(data, os2_record)?;

    if !options.lenient {
        verify_checksum(head_record, head_bytes)?;
        verify_checksum(hhea_record, hhea_bytes)?;
        verify_checksum(os2_record, os2_bytes)?;
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

    let mut os2 = Reader::new(os2_bytes);
    let os2_version = os2.u16()?;
    if !options.lenient && os2_version > OS2_MAX_KNOWN_VERSION {
        return Err(Error::UnsupportedTableVersion {
            table: "OS/2",
            major: os2_version,
            minor: 0,
        });
    }
    os2.skip(2)?; // xAvgCharWidth
    os2.skip(2)?; // usWeightClass
    os2.skip(2)?; // usWidthClass
    os2.skip(2)?; // fsType
    os2.skip(20)?; // sub/superscript and strikeout metrics, 10 int16 fields
    os2.skip(2)?; // sFamilyClass
    os2.skip(10)?; // panose
    os2.skip(16)?; // ulUnicodeRange1..4
    os2.skip(4)?; // achVendID
    os2.skip(2)?; // fsSelection
    os2.skip(2)?; // usFirstCharIndex
    os2.skip(2)?; // usLastCharIndex
    let typo_ascender = os2.i16()?;
    let typo_descender = os2.i16()?;
    let typo_line_gap = os2.i16()?;
    let win_ascent = os2.u16()?;
    let win_descent = os2.u16()?;

    Ok(FontMetrics {
        units_per_em,
        ascender,
        descender,
        line_gap,
        advance_width_max,
        number_of_h_metrics,
        typo_ascender,
        typo_descender,
        typo_line_gap,
        win_ascent,
        win_descent,
    })
}

/// Read the sfnt version and table directory shared by [`parse`] and
/// [`advance_widths`].
fn read_table_directory(data: &[u8], options: &ParseOptions) -> Result<Vec<TableRecord>, Error> {
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
    Ok(records)
}

/// Parse the `hmtx` table and return each glyph's advance width, indexed by
/// glyph ID.
///
/// `hmtx` only stores an explicit `(advanceWidth, lsb)` pair for the first
/// `hhea.numberOfHMetrics` glyphs; every glyph after that reuses the last
/// advance width in the table and stores only its left side bearing. This
/// fills in those trailing entries so the returned `Vec` always has one
/// width per glyph (per `maxp.numGlyphs`).
///
/// ```no_run
/// let data = std::fs::read("some-font.ttf").unwrap();
/// let widths = strict_font_metrics::advance_widths(&data, &strict_font_metrics::ParseOptions::strict());
/// ```
pub fn advance_widths(data: &[u8], options: &ParseOptions) -> Result<Vec<u16>, Error> {
    let records = read_table_directory(data, options)?;

    let hhea_record = find_table(&records, &HHEA_TAG).ok_or(Error::MissingTable("hhea"))?;
    let maxp_record = find_table(&records, &MAXP_TAG).ok_or(Error::MissingTable("maxp"))?;
    let hmtx_record = find_table(&records, &HMTX_TAG).ok_or(Error::MissingTable("hmtx"))?;

    let hhea_bytes = table_bytes(data, hhea_record)?;
    let maxp_bytes = table_bytes(data, maxp_record)?;
    let hmtx_bytes = table_bytes(data, hmtx_record)?;

    if !options.lenient {
        verify_checksum(hhea_record, hhea_bytes)?;
        verify_checksum(maxp_record, maxp_bytes)?;
        verify_checksum(hmtx_record, hmtx_bytes)?;
    }

    let mut hhea = Reader::new(hhea_bytes);
    hhea.skip(34)?; // everything before numberOfHMetrics
    let number_of_h_metrics = hhea.u16()?;

    let mut maxp = Reader::new(maxp_bytes);
    maxp.u32()?; // version (0.5 and 1.0 both start with version, numGlyphs)
    let num_glyphs = maxp.u16()?;

    if !options.lenient && number_of_h_metrics > num_glyphs {
        return Err(Error::InvalidHMetricsCount {
            number_of_h_metrics,
            num_glyphs,
        });
    }

    let mut hmtx = Reader::new(hmtx_bytes);
    let mut widths = Vec::with_capacity(num_glyphs as usize);
    for _ in 0..number_of_h_metrics {
        widths.push(hmtx.u16()?);
        hmtx.skip(2)?; // lsb, not needed for advance widths
    }

    let last_width = widths.last().copied().unwrap_or(0);
    for _ in number_of_h_metrics..num_glyphs {
        hmtx.skip(2)?; // lsb-only entry; advance width repeats the last one
        widths.push(last_width);
    }

    Ok(widths)
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
