use super::*;

/// One directory entry plus the bytes it points at, with knobs to make the
/// resulting font invalid in a specific, controlled way.
struct Table {
    tag: [u8; 4],
    data: Vec<u8>,
    checksum_override: Option<u32>,
    offset_override: Option<u32>,
}

impl Table {
    fn new(tag: [u8; 4], data: Vec<u8>) -> Self {
        Table {
            tag,
            data,
            checksum_override: None,
            offset_override: None,
        }
    }
}

fn build_head(units_per_em: u16, magic: u32, major: u16, minor: u16) -> Vec<u8> {
    let mut v = Vec::with_capacity(54);
    v.extend_from_slice(&major.to_be_bytes());
    v.extend_from_slice(&minor.to_be_bytes());
    v.extend_from_slice(&0u32.to_be_bytes()); // fontRevision
    v.extend_from_slice(&0u32.to_be_bytes()); // checkSumAdjustment
    v.extend_from_slice(&magic.to_be_bytes());
    v.extend_from_slice(&0u16.to_be_bytes()); // flags
    v.extend_from_slice(&units_per_em.to_be_bytes());
    v.extend_from_slice(&[0u8; 8]); // created
    v.extend_from_slice(&[0u8; 8]); // modified
    v.extend_from_slice(&0i16.to_be_bytes()); // xMin
    v.extend_from_slice(&0i16.to_be_bytes()); // yMin
    v.extend_from_slice(&0i16.to_be_bytes()); // xMax
    v.extend_from_slice(&0i16.to_be_bytes()); // yMax
    v.extend_from_slice(&0u16.to_be_bytes()); // macStyle
    v.extend_from_slice(&0u16.to_be_bytes()); // lowestRecPPEM
    v.extend_from_slice(&0i16.to_be_bytes()); // fontDirectionHint
    v.extend_from_slice(&0i16.to_be_bytes()); // indexToLocFormat
    v.extend_from_slice(&0i16.to_be_bytes()); // glyphDataFormat
    assert_eq!(v.len(), 54);
    v
}

#[allow(clippy::too_many_arguments)]
fn build_hhea(
    major: u16,
    minor: u16,
    ascender: i16,
    descender: i16,
    line_gap: i16,
    advance_width_max: u16,
    number_of_h_metrics: u16,
) -> Vec<u8> {
    let mut v = Vec::with_capacity(36);
    v.extend_from_slice(&major.to_be_bytes());
    v.extend_from_slice(&minor.to_be_bytes());
    v.extend_from_slice(&ascender.to_be_bytes());
    v.extend_from_slice(&descender.to_be_bytes());
    v.extend_from_slice(&line_gap.to_be_bytes());
    v.extend_from_slice(&advance_width_max.to_be_bytes());
    v.extend_from_slice(&0i16.to_be_bytes()); // minLeftSideBearing
    v.extend_from_slice(&0i16.to_be_bytes()); // minRightSideBearing
    v.extend_from_slice(&0i16.to_be_bytes()); // xMaxExtent
    v.extend_from_slice(&0i16.to_be_bytes()); // caretSlopeRise
    v.extend_from_slice(&0i16.to_be_bytes()); // caretSlopeRun
    v.extend_from_slice(&0i16.to_be_bytes()); // caretOffset
    v.extend_from_slice(&[0u8; 8]); // reserved x4
    v.extend_from_slice(&0i16.to_be_bytes()); // metricDataFormat
    v.extend_from_slice(&number_of_h_metrics.to_be_bytes());
    assert_eq!(v.len(), 36);
    v
}

#[allow(clippy::too_many_arguments)]
fn build_os2(
    version: u16,
    typo_ascender: i16,
    typo_descender: i16,
    typo_line_gap: i16,
    win_ascent: u16,
    win_descent: u16,
) -> Vec<u8> {
    let mut v = Vec::with_capacity(78);
    v.extend_from_slice(&version.to_be_bytes());
    v.extend_from_slice(&0i16.to_be_bytes()); // xAvgCharWidth
    v.extend_from_slice(&0u16.to_be_bytes()); // usWeightClass
    v.extend_from_slice(&0u16.to_be_bytes()); // usWidthClass
    v.extend_from_slice(&0u16.to_be_bytes()); // fsType
    v.extend_from_slice(&[0u8; 20]); // sub/superscript and strikeout metrics
    v.extend_from_slice(&0i16.to_be_bytes()); // sFamilyClass
    v.extend_from_slice(&[0u8; 10]); // panose
    v.extend_from_slice(&[0u8; 16]); // ulUnicodeRange1..4
    v.extend_from_slice(b"NONE"); // achVendID
    v.extend_from_slice(&0u16.to_be_bytes()); // fsSelection
    v.extend_from_slice(&0u16.to_be_bytes()); // usFirstCharIndex
    v.extend_from_slice(&0u16.to_be_bytes()); // usLastCharIndex
    v.extend_from_slice(&typo_ascender.to_be_bytes());
    v.extend_from_slice(&typo_descender.to_be_bytes());
    v.extend_from_slice(&typo_line_gap.to_be_bytes());
    v.extend_from_slice(&win_ascent.to_be_bytes());
    v.extend_from_slice(&win_descent.to_be_bytes());
    assert_eq!(v.len(), 78);
    v
}

fn build_maxp(num_glyphs: u16) -> Vec<u8> {
    let mut v = Vec::with_capacity(6);
    v.extend_from_slice(&0x0000_5000u32.to_be_bytes()); // version 0.5
    v.extend_from_slice(&num_glyphs.to_be_bytes());
    v
}

fn build_hmtx(long_metrics: &[(u16, i16)], trailing_lsbs: &[i16]) -> Vec<u8> {
    let mut v = Vec::with_capacity(long_metrics.len() * 4 + trailing_lsbs.len() * 2);
    for &(advance_width, lsb) in long_metrics {
        v.extend_from_slice(&advance_width.to_be_bytes());
        v.extend_from_slice(&lsb.to_be_bytes());
    }
    for &lsb in trailing_lsbs {
        v.extend_from_slice(&lsb.to_be_bytes());
    }
    v
}

fn build_font(sfnt_version: u32, tables: Vec<Table>) -> Vec<u8> {
    let header_len = 12 + 16 * tables.len();
    let mut body = Vec::new();
    let mut offsets = Vec::with_capacity(tables.len());
    for t in &tables {
        offsets.push(header_len + body.len());
        body.extend_from_slice(&t.data);
        while body.len() % 4 != 0 {
            body.push(0);
        }
    }

    let mut out = Vec::with_capacity(header_len + body.len());
    out.extend_from_slice(&sfnt_version.to_be_bytes());
    out.extend_from_slice(&(tables.len() as u16).to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes()); // searchRange
    out.extend_from_slice(&0u16.to_be_bytes()); // entrySelector
    out.extend_from_slice(&0u16.to_be_bytes()); // rangeShift
    for (i, t) in tables.iter().enumerate() {
        out.extend_from_slice(&t.tag);
        let checksum = t.checksum_override.unwrap_or_else(|| table_checksum(&t.data));
        out.extend_from_slice(&checksum.to_be_bytes());
        let offset = t.offset_override.unwrap_or(offsets[i] as u32);
        out.extend_from_slice(&offset.to_be_bytes());
        out.extend_from_slice(&(t.data.len() as u32).to_be_bytes());
    }
    out.extend_from_slice(&body);
    out
}

fn valid_font() -> Vec<u8> {
    let head = build_head(2048, HEAD_MAGIC, 1, 0);
    let hhea = build_hhea(1, 0, 1900, -500, 90, 1500, 800);
    let os2 = build_os2(4, 1950, -450, 100, 1900, 500);
    build_font(
        SFNT_TRUETYPE,
        vec![
            Table::new(HEAD_TAG, head),
            Table::new(HHEA_TAG, hhea),
            Table::new(OS2_TAG, os2),
        ],
    )
}

#[test]
fn parses_minimal_valid_font() {
    let data = valid_font();
    let metrics = parse(&data, &ParseOptions::strict()).expect("valid font should parse");
    assert_eq!(metrics.units_per_em, 2048);
    assert_eq!(metrics.ascender, 1900);
    assert_eq!(metrics.descender, -500);
    assert_eq!(metrics.line_gap, 90);
    assert_eq!(metrics.advance_width_max, 1500);
    assert_eq!(metrics.number_of_h_metrics, 800);
    assert_eq!(metrics.line_height(), 1900 - (-500) + 90);
    assert_eq!(metrics.typo_ascender, 1950);
    assert_eq!(metrics.typo_descender, -450);
    assert_eq!(metrics.typo_line_gap, 100);
    assert_eq!(metrics.win_ascent, 1900);
    assert_eq!(metrics.win_descent, 500);
}

#[test]
fn accepts_opentype_cff_and_apple_true_tags() {
    let head = build_head(1000, HEAD_MAGIC, 1, 0);
    let hhea = build_hhea(1, 0, 800, -200, 0, 900, 4);
    let os2 = build_os2(4, 850, -150, 0, 800, 150);
    let otto = build_font(
        SFNT_OPENTYPE_CFF,
        vec![
            Table::new(HEAD_TAG, head.clone()),
            Table::new(HHEA_TAG, hhea.clone()),
            Table::new(OS2_TAG, os2.clone()),
        ],
    );
    assert!(parse(&otto, &ParseOptions::strict()).is_ok());

    let apple_true = build_font(
        SFNT_APPLE_TRUE,
        vec![
            Table::new(HEAD_TAG, head),
            Table::new(HHEA_TAG, hhea),
            Table::new(OS2_TAG, os2),
        ],
    );
    assert!(matches!(
        parse(&apple_true, &ParseOptions::strict()),
        Err(Error::BadSfntVersion(_))
    ));
    assert!(parse(&apple_true, &ParseOptions::lenient()).is_ok());
}

#[test]
fn missing_hhea_table_is_an_error() {
    let head = build_head(2048, HEAD_MAGIC, 1, 0);
    let os2 = build_os2(4, 1950, -450, 100, 1900, 500);
    let data = build_font(
        SFNT_TRUETYPE,
        vec![Table::new(HEAD_TAG, head), Table::new(OS2_TAG, os2)],
    );
    assert!(matches!(
        parse(&data, &ParseOptions::strict()),
        Err(Error::MissingTable("hhea"))
    ));
}

#[test]
fn missing_os2_table_is_an_error() {
    let head = build_head(2048, HEAD_MAGIC, 1, 0);
    let hhea = build_hhea(1, 0, 1900, -500, 90, 1500, 800);
    let data = build_font(
        SFNT_TRUETYPE,
        vec![Table::new(HEAD_TAG, head), Table::new(HHEA_TAG, hhea)],
    );
    assert!(matches!(
        parse(&data, &ParseOptions::strict()),
        Err(Error::MissingTable("OS/2"))
    ));
}

#[test]
fn bad_sfnt_version_is_rejected() {
    let data = build_font(
        0xDEAD_BEEF,
        vec![
            Table::new(HEAD_TAG, build_head(2048, HEAD_MAGIC, 1, 0)),
            Table::new(HHEA_TAG, build_hhea(1, 0, 1900, -500, 90, 1500, 800)),
            Table::new(OS2_TAG, build_os2(4, 1950, -450, 100, 1900, 500)),
        ],
    );
    assert!(matches!(
        parse(&data, &ParseOptions::strict()),
        Err(Error::BadSfntVersion(0xDEAD_BEEF))
    ));
    assert!(matches!(
        parse(&data, &ParseOptions::lenient()),
        Err(Error::BadSfntVersion(0xDEAD_BEEF))
    ));
}

#[test]
fn checksum_mismatch_strict_vs_lenient() {
    let mut head_table = Table::new(HEAD_TAG, build_head(2048, HEAD_MAGIC, 1, 0));
    head_table.checksum_override = Some(0x1234_5678);
    let data = build_font(
        SFNT_TRUETYPE,
        vec![
            head_table,
            Table::new(HHEA_TAG, build_hhea(1, 0, 1900, -500, 90, 1500, 800)),
            Table::new(OS2_TAG, build_os2(4, 1950, -450, 100, 1900, 500)),
        ],
    );

    assert!(matches!(
        parse(&data, &ParseOptions::strict()),
        Err(Error::ChecksumMismatch { tag: HEAD_TAG, .. })
    ));

    let metrics = parse(&data, &ParseOptions::lenient()).expect("lenient parse should recover");
    assert_eq!(metrics.units_per_em, 2048);
}

#[test]
fn bad_magic_number_strict_vs_lenient() {
    let data = build_font(
        SFNT_TRUETYPE,
        vec![
            Table::new(HEAD_TAG, build_head(2048, 0xBAD_C0DE, 1, 0)),
            Table::new(HHEA_TAG, build_hhea(1, 0, 1900, -500, 90, 1500, 800)),
            Table::new(OS2_TAG, build_os2(4, 1950, -450, 100, 1900, 500)),
        ],
    );

    assert!(matches!(
        parse(&data, &ParseOptions::strict()),
        Err(Error::BadMagicNumber(0xBAD_C0DE))
    ));

    let metrics = parse(&data, &ParseOptions::lenient()).expect("lenient parse should recover");
    assert_eq!(metrics.units_per_em, 2048);
}

#[test]
fn unsupported_table_version_strict_vs_lenient() {
    let data = build_font(
        SFNT_TRUETYPE,
        vec![
            Table::new(HEAD_TAG, build_head(2048, HEAD_MAGIC, 2, 0)),
            Table::new(HHEA_TAG, build_hhea(1, 0, 1900, -500, 90, 1500, 800)),
            Table::new(OS2_TAG, build_os2(4, 1950, -450, 100, 1900, 500)),
        ],
    );

    assert!(matches!(
        parse(&data, &ParseOptions::strict()),
        Err(Error::UnsupportedTableVersion {
            table: "head",
            major: 2,
            minor: 0
        })
    ));

    let metrics = parse(&data, &ParseOptions::lenient()).expect("lenient parse should recover");
    assert_eq!(metrics.units_per_em, 2048);
}

#[test]
fn unsupported_os2_version_strict_vs_lenient() {
    let data = build_font(
        SFNT_TRUETYPE,
        vec![
            Table::new(HEAD_TAG, build_head(2048, HEAD_MAGIC, 1, 0)),
            Table::new(HHEA_TAG, build_hhea(1, 0, 1900, -500, 90, 1500, 800)),
            Table::new(OS2_TAG, build_os2(6, 1950, -450, 100, 1900, 500)),
        ],
    );

    assert!(matches!(
        parse(&data, &ParseOptions::strict()),
        Err(Error::UnsupportedTableVersion {
            table: "OS/2",
            major: 6,
            minor: 0
        })
    ));

    let metrics = parse(&data, &ParseOptions::lenient()).expect("lenient parse should recover");
    assert_eq!(metrics.win_ascent, 1900);
}

#[test]
fn table_out_of_bounds_is_an_error() {
    let mut head_table = Table::new(HEAD_TAG, build_head(2048, HEAD_MAGIC, 1, 0));
    head_table.offset_override = Some(10_000);
    let data = build_font(
        SFNT_TRUETYPE,
        vec![
            head_table,
            Table::new(HHEA_TAG, build_hhea(1, 0, 1900, -500, 90, 1500, 800)),
            Table::new(OS2_TAG, build_os2(4, 1950, -450, 100, 1900, 500)),
        ],
    );

    assert!(matches!(
        parse(&data, &ParseOptions::strict()),
        Err(Error::TableOutOfBounds { tag: HEAD_TAG, .. })
    ));
}

#[test]
fn truncated_input_yields_too_short() {
    let data = valid_font();
    for cut in [0usize, 1, 4, 11] {
        let truncated = &data[..cut];
        assert!(matches!(
            parse(truncated, &ParseOptions::strict()),
            Err(Error::TooShort { .. })
        ));
    }
}

fn font_with_hmtx(num_glyphs: u16, number_of_h_metrics: u16, hmtx: Vec<u8>) -> Vec<u8> {
    let head = build_head(2048, HEAD_MAGIC, 1, 0);
    let hhea = build_hhea(1, 0, 1900, -500, 90, 1500, number_of_h_metrics);
    let os2 = build_os2(4, 1950, -450, 100, 1900, 500);
    let maxp = build_maxp(num_glyphs);
    build_font(
        SFNT_TRUETYPE,
        vec![
            Table::new(HEAD_TAG, head),
            Table::new(HHEA_TAG, hhea),
            Table::new(OS2_TAG, os2),
            Table::new(MAXP_TAG, maxp),
            Table::new(HMTX_TAG, hmtx),
        ],
    )
}

#[test]
fn parses_advance_widths_with_trailing_lsb_only_entries() {
    let hmtx = build_hmtx(&[(600, 10), (650, 5), (700, 0)], &[3, -2]);
    let data = font_with_hmtx(5, 3, hmtx);

    let widths = advance_widths(&data, &ParseOptions::strict()).expect("should parse");
    assert_eq!(widths, vec![600, 650, 700, 700, 700]);
}

#[test]
fn advance_widths_with_no_trailing_entries() {
    let hmtx = build_hmtx(&[(500, 0), (500, 0)], &[]);
    let data = font_with_hmtx(2, 2, hmtx);

    let widths = advance_widths(&data, &ParseOptions::strict()).expect("should parse");
    assert_eq!(widths, vec![500, 500]);
}

#[test]
fn advance_widths_missing_maxp_is_an_error() {
    let head = build_head(2048, HEAD_MAGIC, 1, 0);
    let hhea = build_hhea(1, 0, 1900, -500, 90, 1500, 1);
    let hmtx = build_hmtx(&[(600, 0)], &[]);
    let data = build_font(
        SFNT_TRUETYPE,
        vec![
            Table::new(HEAD_TAG, head),
            Table::new(HHEA_TAG, hhea),
            Table::new(HMTX_TAG, hmtx),
        ],
    );

    assert!(matches!(
        advance_widths(&data, &ParseOptions::strict()),
        Err(Error::MissingTable("maxp"))
    ));
}

#[test]
fn advance_widths_missing_hmtx_is_an_error() {
    let head = build_head(2048, HEAD_MAGIC, 1, 0);
    let hhea = build_hhea(1, 0, 1900, -500, 90, 1500, 1);
    let maxp = build_maxp(1);
    let data = build_font(
        SFNT_TRUETYPE,
        vec![
            Table::new(HEAD_TAG, head),
            Table::new(HHEA_TAG, hhea),
            Table::new(MAXP_TAG, maxp),
        ],
    );

    assert!(matches!(
        advance_widths(&data, &ParseOptions::strict()),
        Err(Error::MissingTable("hmtx"))
    ));
}

#[test]
fn number_of_h_metrics_greater_than_num_glyphs_strict_vs_lenient() {
    let hmtx = build_hmtx(&[(600, 0), (650, 0), (700, 0)], &[]);
    let data = font_with_hmtx(2, 3, hmtx);

    assert!(matches!(
        advance_widths(&data, &ParseOptions::strict()),
        Err(Error::InvalidHMetricsCount {
            number_of_h_metrics: 3,
            num_glyphs: 2,
        })
    ));

    // Lenient mode skips the sanity check and just reads numberOfHMetrics
    // records straight out of hmtx, ignoring maxp's glyph count.
    let widths = advance_widths(&data, &ParseOptions::lenient()).expect("should parse");
    assert_eq!(widths, vec![600, 650, 700]);
}

#[test]
fn advance_widths_truncated_hmtx_yields_too_short() {
    // hhea claims 2 long metrics, but the hmtx table itself is only long
    // enough for one - the kind of mismatch a lazy subsetting tool leaves
    // behind.
    let hmtx = build_hmtx(&[(600, 10)], &[]);
    let data = font_with_hmtx(3, 2, hmtx);

    assert!(matches!(
        advance_widths(&data, &ParseOptions::strict()),
        Err(Error::TooShort { .. })
    ));
}
