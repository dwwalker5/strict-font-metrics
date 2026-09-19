# strict-font-metrics

A Rust library that reads vertical metrics out of TrueType and OpenType
files (units per em, ascender/descender/line gap from both `hhea` and
`OS/2`, max advance width) by parsing the sfnt table directory and the
`head`/`hhea`/`OS/2` tables directly. No third-party dependencies.

## The problem

The sfnt format has a table directory that lists a checksum, offset, and
length for every table in the file. Nothing forces those checksums to be
correct, the `head` table's magic number to be `0x5F0F3CF5`, or the table
version fields to be ones an actual spec revision defines. In practice a
meaningful fraction of fonts floating around get one of these wrong: a
subsetting tool truncates a table without recomputing its checksum, a font
editor writes a stale version number, a hand patch shifts an offset by a
byte. A parser that ignores all of this will hand back plausible-looking
numbers for a corrupt file, which is worse than an error.

This library checks everything the format specifies by default and returns
an `Err` the moment something doesn't match, naming exactly which check
failed. When you'd rather have a best-effort answer than an error - reading
metrics from a font you don't control the quality of, say - you ask for
that explicitly instead of it being silently assumed.

## Usage

```rust
use strict_font_metrics::{parse, ParseOptions, Error};

let data = std::fs::read("some-font.ttf").expect("read font file");

match parse(&data, &ParseOptions::strict()) {
    Ok(metrics) => {
        println!("units per em: {}", metrics.units_per_em);
        println!("line height:  {}", metrics.line_height());
    }
    Err(Error::ChecksumMismatch { tag, .. }) => {
        eprintln!("font has a bad checksum, retrying leniently");
        let metrics = parse(&data, &ParseOptions::lenient())
            .expect("even a lenient parse should read head/hhea");
        println!("units per em: {}", metrics.units_per_em);
        let _ = tag;
    }
    Err(e) => panic!("could not read font: {e}"),
}
```

`ParseOptions::strict()` is also what `ParseOptions::default()` gives you,
so `parse(&data, &ParseOptions::default())` and the explicit form behave
identically - the lenient path is always something a caller opts into.

## What strict mode checks

- the sfnt version tag is one this parser recognizes (`0x00010000` or `OTTO`)
- every table directory offset/length stays within the file
- the `head`, `hhea`, and `OS/2` table checksums match what the directory records
- `head`'s magic number is `0x5F0F3CF5`
- `head` and `hhea` report version 1.0, the only version either table has ever had
- `OS/2` reports a version this parser knows about (0 through 5)

`ParseOptions::lenient()` turns all of the above off and reads the same
byte layout anyway, on the theory that a font with a wrong checksum still
has real numbers sitting in the right place in the file.

## Status

Early. `head`, `hhea`, and `OS/2` are parsed, which covers vertical line
metrics (both the `hhea` values and the Windows-specific ones from
`OS/2`) but not per-glyph advance widths. See the repository's issues for
what's planned next.

## License

MIT, see `LICENSE`.
