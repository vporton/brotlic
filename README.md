# Brotlic

[![crates.io](https://img.shields.io/crates/v/brotlic.svg)](https://crates.io/crates/brotlic)
[![Released API docs](https://docs.rs/brotlic/badge.svg)](https://docs.rs/brotlic)
[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)

Brotlic (or BrotliC) is a thin wrapper around [brotli](https://github.com/google/brotli). It
provides Rust bindings to all compression and decompression APIs. On the fly compression and
decompression is supported for both `BufRead` and `Write` via `CompressorReader<R>`,
`CompressorWriter<W>`, `DecompressorReader<R>` and `DecompressorWriter<W>`. For low-level
instances, see `BrotliEncoder` and `BrotliDecoder`. These can be configured via
`BrotliEncoderOptions` and `BrotliDecoderOptions` respectively.

## Requirements

A __C__ compiler is required for building [brotli](https://github.com/google/brotli) with cargo.

## Usage

When dealing with [`BufRead`]:

* `DecompressorReader<R>` - Reads a brotli compressed input stream and decompresses it.
* `CompressorReader<R>` - Reads a stream and compresses it while reading.

When dealing with [`Write`]:

* `CompressorWriter<W>` - Writes brotli compressed data to the underlying writer.
* `DecompressorWriter<W>` - Writes brotli decompressed data to the underlying writer.

To simplify this decision, the following table outlines all the differences:

|                           | Input        | Output       | Wraps       |
|---------------------------|--------------|--------------|-------------|
| `CompressorReader<R>`     | Uncompressed | Compressed   | [`BufRead`] |
| `DecompressorReader<R>`   | Compressed   | Uncompressed | [`BufRead`] |
| `CompressorWriter<W>`     | Uncompressed | Compressed   | [`Write`]   |
| `DecompressorWriter<W>`   | Compressed   | Uncompressed | [`Write`]   |

[`BufRead`]: https://doc.rust-lang.org/std/io/trait.BufRead.html
[`Write`]: https://doc.rust-lang.org/std/io/trait.Write.html

To compress a file with brotli:

```rust
use std::fs::File;
use std::io::{self, Write};
use brotlic::CompressorWriter;

let mut input = File::open("test.txt")?; // uncompressed text file
let mut output = File::create("test.brotli")?; // compressed text output file
let mut output_compressed = CompressorWriter::new(output);

output_compressed.write_all(b"test")?;
```

To decompress that same file:

```rust
use std::fs::File;
use std::io::{self, BufReader, Read};
use brotlic::DecompressorReader;

let mut input = BufReader::new(File::open("test.brotli")?); // uncompressed text file
let mut input_decompressed = DecompressorReader::new(input); // requires BufRead

let mut text = String::new();
input_decompressed.read_to_string(&mut text)?;

assert_eq!(text, "test");
```

To compress and decompress in memory:

```rust
use std::io::{self, Cursor, Read, Write};
use brotlic::{CompressorWriter, DecompressorReader};

let input = vec![0; 1024];

// create a wrapper around Write that supports on the fly brotli compression.
let mut compressor = CompressorWriter::new(Cursor::new(Vec::new())); // write to memory
compressor.write_all(input.as_slice());
let encoded_input = compressor.into_inner()?.into_inner(); // read to vec

// create a wrapper around BufRead that supports on the fly brotli decompression.
let mut decompressed_reader = DecompressorReader::new(Cursor::new(encoded_input));
let mut decoded_input = Vec::new();

decompressed_reader.read_to_end(&mut decoded_input)?;

assert_eq!(input, decoded_input);
```

### Customizing compression quality

Sometimes it can be desirable to trade run-time costs for an even better compression ratio:

```rust
use std::io::Cursor;
use brotlic::{BlockSize, BrotliEncoderOptions, CompressorWriter, Quality, WindowSize};

let encoder = BrotliEncoderOptions::new()
    .quality(Quality::best())
    .window_size(WindowSize::best())
    .block_size(BlockSize::best())
    .build()?;

let writer = Cursor::new(Vec::new());
let compressed_writer = CompressorWriter::with_encoder(encoder, writer);
```

It is recommended to not use the encoder directly but instead pass it onto the higher level
abstractions.

## Credits

* [brotli library](https://github.com/google/brotli) - for the underlying C library
* [JetBrains](https://www.jetbrains.com/) - for their amazing tooling