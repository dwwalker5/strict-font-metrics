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
    build_font(
        SFNT_TRUETYPE,
        vec![Table::new(HEAD_TAG, head), Table::new(HHEA_TAG, hhea)],
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
}

#[test]
fn accepts_opentype_cff_and_apple_true_tags() {
    let head = build_head(1000, HEAD_MAGIC, 1, 0);
    let hhea = build_hhea(1, 0, 800, -200, 0, 900, 4);
    let otto = build_font(
        SFNT_OPENTYPE_CFF,
        vec![
            Table::new(HEAD_TAG, head.clone()),
            Table::new(HHEA_TAG, hhea.clone()),
        ],
    );
    assert!(parse(&otto, &ParseOptions::strict()).is_ok());

    let apple_true = build_font(
        SFNT_APPLE_TRUE,
        vec![Table::new(HEAD_TAG, head), Table::new(HHEA_TAG, hhea)],
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
    let data = build_font(SFNT_TRUETYPE, vec![Table::new(HEAD_TAG, head)]);
    assert!(matches!(
        parse(&data, &ParseOptions::strict()),
        Err(Error::MissingTable("hhea"))
    ));
}

#[test]
fn bad_sfnt_version_is_rejected() {
    let data = build_font(
        0xDEAD_BEEF,
        vec![
            Table::new(HEAD_TAG, build_head(2048, HEAD_MAGIC, 1, 0)),
            Table::new(HHEA_TAG, build_hhea(1, 0, 1900, -500, 90, 1500, 800)),
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
fn table_out_of_bounds_is_an_error() {
    let mut head_table = Table::new(HEAD_TAG, build_head(2048, HEAD_MAGIC, 1, 0));
    head_table.offset_override = Some(10_000);
    let data = build_font(
        SFNT_TRUETYPE,
        vec![
            head_table,
            Table::new(HHEA_TAG, build_hhea(1, 0, 1900, -500, 90, 1500, 800)),
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
