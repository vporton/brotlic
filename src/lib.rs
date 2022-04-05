//! # Brotlic
//!
//! Brotlic (or BrotlyC) is a thin wrapper around [brotli](https://github.com/google/brotli). It
//! provides Rust bindings to all compression and decompression APIs. On the fly compression and
//! decompression is supported for both `BufRead` and `Write` via [`CompressorReader<R>`,
//! `CompressorWriter<W>`, `DecompressorReader<R>` and `DecompressorWriter<W>`. For low-level
//! instances, see `BrotliEncoder` and `BrotliDecoder`. These can be configured via
//! `BrotliEncoderOptions` and `BrotliDecoderOptions` respectively.
//!
//! ## High level abstractions
//!
//! When dealing with [`BufRead`]:
//!
//! * [`DecompressorReader<R>`] - Reads a brotli compressed input stream and decompresses it.
//! * [`CompressorReader<R>`] - Reads a stream and compresses it while reading.
//!
//! When dealing with [`Write`]:
//!
//! * [`CompressorWriter<W>`] - Writes brotli compressed data to the underlying writer.
//! * [`DecompressorWriter<W>`] - Writes brotli decompressed data to the underlying writer.
//!
//! To simplify this decision, the following table outlines all the differences:
//!
//! |                           | Input        | Output       | Wraps       |
//! |---------------------------|--------------|--------------|-------------|
//! | [`CompressorReader<R>`]   | Uncompressed | Compressed   | [`BufRead`] |
//! | [`DecompressorReader<R>`] | Compressed   | Uncompressed | [`BufRead`] |
//! | [`CompressorWriter<W>`]   | Uncompressed | Compressed   | [`Write`]   |
//! | [`DecompressorWriter<W>`] | Compressed   | Uncompressed | [`Write`]   |
//!
//! [`BufRead`]: https://doc.rust-lang.org/std/io/trait.BufRead.html
//! [`Write`]: https://doc.rust-lang.org/std/io/trait.Write.html
//!
//! To compress a file with brotli:
//!
//! ```no_run
//! use std::fs::File;
//! use std::io::{self, Write};
//! use brotlic::CompressorWriter;
//!
//! let mut input = File::open("test.txt")?; // uncompressed text file
//! let mut output = File::create("test.brotli")?; // compressed text output file
//! let mut output_compressed = CompressorWriter::new(output);
//!
//! output_compressed.write_all(b"test")?;
//!
//! # Ok::<(), io::Error>(())
//! ```
//!
//! To decompress that same file:
//!
//! ```no_run
//! use std::fs::File;
//! use std::io::{self, BufReader, Read};
//! use brotlic::DecompressorReader;
//!
//! let mut input = BufReader::new(File::open("test.brotli")?); // uncompressed text file
//! let mut input_decompressed = DecompressorReader::new(input); // requires BufRead
//!
//! let mut text = String::new();
//! input_decompressed.read_to_string(&mut text)?;
//!
//! assert_eq!(text, "test");
//!
//! # Ok::<(), io::Error>(())
//! ```
//!
//! To compress and decompress in memory:
//!
//! ```
//! use std::io::{self, Cursor, Read, Write};
//! use brotlic::{CompressorWriter, DecompressorReader};
//!
//! let input = vec![0; 1024];
//!
//! // create a wrapper around Write that supports on the fly brotli compression.
//! let mut compressor = CompressorWriter::new(Cursor::new(Vec::new())); // write to memory
//! compressor.write_all(input.as_slice());
//! let encoded_input = compressor.into_inner()?.into_inner(); // read to vec
//!
//! // create a wrapper around BufRead that supports on the fly brotli decompression.
//! let mut decompressed_reader = DecompressorReader::new(Cursor::new(encoded_input));
//! let mut decoded_input = Vec::new();
//!
//! decompressed_reader.read_to_end(&mut decoded_input)?;
//!
//! assert_eq!(input, decoded_input);
//!
//! # Ok::<(), io::Error>(())
//! ```
//!
//! ## Customizing compression quality
//!
//! Sometimes it can be desirable to trade run-time costs for an even better compression ratio:
//!
//! ```
//! use std::io::Cursor;
//! use brotlic::{BlockSize, BrotliEncoderOptions, CompressorWriter, Quality, WindowSize};
//! # use brotlic::ParameterError;
//!
//! let encoder = BrotliEncoderOptions::new()
//!     .quality(Quality::best())
//!     .window_size(WindowSize::best())
//!     .block_size(BlockSize::best())
//!     .build()?;
//!
//! let writer = Cursor::new(Vec::new());
//! let compressed_writer = CompressorWriter::with_encoder(encoder, writer);
//!
//! # Ok::<(), ParameterError>(())
//! ```
//!
//! It is recommended to not use the encoder directly but instead pass it onto the higher level
//! abstractions.

#![warn(missing_docs)]

pub mod decode;
pub mod encode;

pub use encode::BrotliEncoder;
pub use encode::BrotliEncoderOptions;
pub use encode::CompressorWriter;
pub use encode::CompressorReader;

pub use decode::BrotliDecoder;
pub use decode::BrotliDecoderOptions;
pub use decode::DecompressorReader;
pub use decode::DecompressorWriter;

use brotlic_sys::*;
use std::os::raw::c_int;
use std::{error, fmt, io};

/// Quality level of the brotli compression
///
/// [`Quality::best()`] represents the best available quality that maximizes the compression ratio
/// at the cost of run-time speed. [`Quality::worst()`] represents the worst available quality that
/// maximizes speed at the expense of compression ratio.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct Quality(u8);

impl Quality {
    /// Attempts to create a new brotli compression quality.
    ///
    /// The range of valid qualities is from 0 to 11 inclusive, where 0 is the worst possible
    /// quality and 11 is the best possible quality.
    ///
    /// # Errors
    ///
    /// An [`Err`] will be returned if the `value` is out of the range of valid qualities.
    ///
    /// # Examples
    ///
    /// ```
    /// use brotlic::Quality;
    ///
    /// let worst_quality = Quality::new(0).unwrap();
    /// let best_quality = Quality::new(11).unwrap();
    ///
    /// assert_eq!(worst_quality, Quality::worst());
    /// assert_eq!(best_quality, Quality::best());
    /// ```
    pub fn new(value: u8) -> Result<Quality, QualityError> {
        match value {
            BROTLI_MIN_QUALITY..=BROTLI_MAX_QUALITY => Ok(Quality(value)),
            _ => Err(QualityError),
        }
    }

    /// The highest quality for brotli compression.
    ///
    /// This quality yields maximum compression ratio at the expense of run-time speed.
    pub fn best() -> Quality {
        Quality(BROTLI_MAX_QUALITY)
    }

    /// The default quality to use for brotli compression.
    ///
    /// This is current set to the best possible quality.
    pub fn default() -> Quality {
        Quality(BROTLI_DEFAULT_QUALITY)
    }

    /// The worst quality to use for brotli compression.
    ///
    /// This quality yields the worst compression ratio while offering the highest run-time speed.
    pub fn worst() -> Quality {
        Quality(BROTLI_MIN_QUALITY)
    }
}

impl Default for Quality {
    /// Creates a new `Quality` using [`default`].
    /// See its documentation for more.
    ///
    /// [`default`]: Quality::default
    fn default() -> Self {
        Quality::default()
    }
}

/// An error returned by [`Quality::new`].
#[derive(Debug)]
pub struct QualityError;

impl fmt::Display for QualityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "quality out of range (must be between {} and {} inclusive)",
            BROTLI_MIN_QUALITY, BROTLI_MAX_QUALITY
        )
    }
}

impl error::Error for QualityError {}

/// The sliding window size (in bits) to use for compression.
///
/// Its maximum size is currently limited to 16 MiB, as specified in RFC7932 (Brotli proper).
/// Larger window sizes are supported via [`LargeWindowSize`], however note that decompression
/// support for these have to be explicitly enabled. This can be configured via
/// [`non_std_window_size_support`] for the matching [`BrotliDecoder`].
///
/// [`non_std_window_size_support`]: decode::BrotliDecoderOptions::non_std_window_size_support()
/// [`BrotliDecoder`]: decode::BrotliDecoder
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct WindowSize(u8);

impl WindowSize {
    /// Consturcts a new sliding window size to use for brotli compression.
    ///
    /// Valid `bits` range from 10 (1 KiB) to 24 (16 MiB) inclusive.
    ///
    /// # Errors
    ///
    /// An [`Err`] will be returned if the `bits` are out of the range of valid window sizes.
    ///
    /// # Examples
    ///
    /// ```
    /// use brotlic::WindowSize;
    ///
    /// let worst_size = WindowSize::new(10).unwrap();
    /// let best_size = WindowSize::new(24).unwrap();
    ///
    /// assert_eq!(worst_size, WindowSize::worst());
    /// assert_eq!(best_size, WindowSize::best());
    /// ```
    pub fn new(bits: u8) -> Result<WindowSize, WindowSizeError> {
        match bits {
            BROTLI_MIN_WINDOW_BITS..=BROTLI_MAX_WINDOW_BITS => Ok(WindowSize(bits)),
            _ => Err(WindowSizeError),
        }
    }

    /// Constructs the best sliding window size to use for brotli compression.
    ///
    /// This is currently limited to 24 bits (16 MiB) due to RFC7932 (Brotli proper). To use larger
    /// sliding window sizes, please refer to [`LargeWindowSize`]. Note however that explicit
    /// support has to be enabled by the decoder. This is supported by enabling
    /// [`non_std_window_size_support`] when constructing a [`BrotliDecoder`].
    ///
    /// [`non_std_window_size_support`]: decode::BrotliDecoderOptions::non_std_window_size_support()
    /// [`BrotliDecoder`]: decode::BrotliDecoder
    ///
    /// # Examples
    ///
    /// ```
    /// use brotlic::WindowSize;
    ///
    /// let best_size = WindowSize::new(24).unwrap();
    ///
    /// assert_eq!(best_size, WindowSize::best());
    /// ```
    pub fn best() -> WindowSize {
        WindowSize(BROTLI_MAX_WINDOW_BITS)
    }

    /// Constructs the default sliding window size to use for brotli compression.
    ///
    /// This is currently set to 22 bits (4 MiB).
    ///
    /// # Examples
    ///
    /// ```
    /// use brotlic::WindowSize;
    ///
    /// let default_size = WindowSize::new(22).unwrap();
    ///
    /// assert_eq!(default_size, WindowSize::default());
    /// ```
    pub fn default() -> WindowSize {
        WindowSize(BROTLI_DEFAULT_WINDOW)
    }

    /// Constructs the worst sliding window size to use for brotli compression
    ///
    /// This is currently set to 10 bits (1 KiB).
    ///
    /// # Examples
    ///
    /// ```
    /// use brotlic::WindowSize;
    ///
    /// let worst_size = WindowSize::new(10).unwrap();
    ///
    /// assert_eq!(worst_size, WindowSize::worst());
    /// ```
    pub fn worst() -> WindowSize {
        WindowSize(BROTLI_MIN_WINDOW_BITS)
    }
}

impl Default for WindowSize {
    /// Creates a new `WindowSize` using [`default`].
    /// See its documentation for more.
    ///
    /// [`default`]: WindowSize::default()
    fn default() -> Self {
        WindowSize::default()
    }
}

impl TryFrom<LargeWindowSize> for WindowSize {
    type Error = WindowSizeError;

    /// Attempts to construct a [`WindowSize`] from a [`LargeWindowSize`].
    ///
    /// This only works if the large window size is currently set to a value lower or equal to
    /// [`WindowSize::best()`].
    ///
    /// # Errors
    ///
    /// Large window size does not fit into a window size.
    fn try_from(large_window_size: LargeWindowSize) -> Result<Self, Self::Error> {
        WindowSize::new(large_window_size.0)
    }
}

/// An error returned by [`WindowSize::new`].
#[derive(Debug)]
pub struct WindowSizeError;

impl fmt::Display for WindowSizeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "window size out of range (must be between {} and {} inclusive)",
            BROTLI_MIN_WINDOW_BITS, BROTLI_MAX_WINDOW_BITS
        )
    }
}

impl error::Error for WindowSizeError {}

/// The large sliding window size (in bits) to use for compression.
///
/// Note that using a large sliding window size for a particular compressor requires explicit
/// support by the decompressor. This is supported by enabling [`non_std_window_size_support`] when
/// constructing a [`BrotliDecoder`].
///
/// [`non_std_window_size_support`]: decode::BrotliDecoderOptions::non_std_window_size_support()
/// [`BrotliDecoder`]: decode::BrotliDecoder
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct LargeWindowSize(u8);

impl LargeWindowSize {
    /// Consturcts a new large sliding window size (in bits) to use for brotli compression.
    ///
    /// Valid `bits` range from 10 (1 KiB) to 30 (1 GiB) inclusive.
    ///
    /// # Errors
    ///
    /// An [`Err`] will be returned if the `bits` are out of the range of valid large window sizes.
    ///
    /// # Examples
    ///
    /// ```
    /// use brotlic::LargeWindowSize;
    ///
    /// let worst_size = LargeWindowSize::new(10).unwrap();
    /// let best_size = LargeWindowSize::new(30).unwrap();
    ///
    /// assert_eq!(worst_size, LargeWindowSize::worst());
    /// assert_eq!(best_size, LargeWindowSize::best());
    /// ```
    pub fn new(bits: u8) -> Result<LargeWindowSize, LargeWindowSizeError> {
        match bits {
            BROTLI_MIN_WINDOW_BITS..=BROTLI_LARGE_MAX_WINDOW_BITS => Ok(LargeWindowSize(bits)),
            _ => Err(LargeWindowSizeError),
        }
    }

    /// Constructs the best large sliding window size to use for brotli compression.
    ///
    /// This is currently set to 30 bits (1 GiB). Note that this requires explicit support by the
    /// decompressor. For more information see [`LargeWindowSize`].
    ///
    /// # Examples
    ///
    /// ```
    /// use brotlic::LargeWindowSize;
    ///
    /// let best_size = LargeWindowSize::new(30).unwrap();
    ///
    /// assert_eq!(best_size, LargeWindowSize::best());
    /// ```
    pub fn best() -> LargeWindowSize {
        LargeWindowSize(BROTLI_LARGE_MAX_WINDOW_BITS)
    }

    /// Constructs the default large sliding window size to use for brotli compression.
    ///
    /// This is currently set to 22 bits (4 MiB).
    ///
    /// # Examples
    ///
    /// ```
    /// use brotlic::LargeWindowSize;
    ///
    /// let default_size = LargeWindowSize::new(22).unwrap();
    ///
    /// assert_eq!(default_size, LargeWindowSize::default());
    /// ```
    pub fn default() -> LargeWindowSize {
        LargeWindowSize(BROTLI_DEFAULT_WINDOW)
    }

    /// Constructs the worst large sliding window size to use for brotli compression
    ///
    /// This is currently set to 10 bits (1 KiB).
    ///
    /// # Examples
    ///
    /// ```
    /// use brotlic::LargeWindowSize;
    ///
    /// let worst_size = LargeWindowSize::new(10).unwrap();
    ///
    /// assert_eq!(worst_size, LargeWindowSize::worst());
    /// ```
    pub fn worst() -> LargeWindowSize {
        LargeWindowSize(BROTLI_MIN_WINDOW_BITS)
    }
}

impl Default for LargeWindowSize {
    /// Creates a new `LargeWindowSize` using [`default`].
    /// See its documentation for more.
    ///
    /// [`default`]: LargeWindowSize::default()
    fn default() -> Self {
        LargeWindowSize::default()
    }
}

impl From<WindowSize> for LargeWindowSize {
    /// Constructs a [`LargeWindowSize`] from a [`WindowSize`].
    ///
    /// This always works because a `LargeWindowSize` covers a larger range than the regular
    /// `WindowSize`. The inverse is not true, however.
    fn from(window_size: WindowSize) -> Self {
        LargeWindowSize(window_size.0)
    }
}

/// An error returned by [`LargeWindowSize::new`].
#[derive(Debug)]
pub struct LargeWindowSizeError;

impl fmt::Display for LargeWindowSizeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "large window size out of range (must be between {} and {} inclusive)",
            BROTLI_MIN_WINDOW_BITS, BROTLI_LARGE_MAX_WINDOW_BITS
        )
    }
}

impl error::Error for LargeWindowSizeError {}

/// The recommended input block size (in bits) to use for compression.
///
/// The compressor may reduce this value at its leisure, for example when the input size is small.
/// Larger block sizes allow better compression at the expense of using more memory. Rough formula
/// for memory required is `3 << bits` bytes.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct BlockSize(u8);

impl BlockSize {
    /// Constructs a new block size (in bits) to use for brotli compression.
    ///
    /// Valid `bits` range from 16 to 24 inclusive.
    ///
    /// # Errors
    ///
    /// An [`Err`] will be returned if the `bits` are out of the range of valid block sizes.
    ///
    /// # Examples
    ///
    /// ```
    /// use brotlic::BlockSize;
    ///
    /// let worst_size = BlockSize::new(16).unwrap();
    /// let best_size = BlockSize::new(24).unwrap();
    ///
    /// assert_eq!(worst_size, BlockSize::worst());
    /// assert_eq!(best_size, BlockSize::best());
    /// ```
    pub fn new(bits: u8) -> Result<BlockSize, BlockSizeError> {
        match bits {
            BROTLI_MIN_INPUT_BLOCK_BITS..=BROTLI_MAX_INPUT_BLOCK_BITS => Ok(BlockSize(bits)),
            _ => Err(BlockSizeError),
        }
    }

    /// Constructs the best block size (in bits) to use for brotli compression.
    ///
    /// This will allow better compression at the expense of memory usage. Currently it is set to
    /// 24 bits.
    ///
    /// # Examples
    ///
    /// ```
    /// use brotlic::BlockSize;
    ///
    /// let best_size = BlockSize::new(24).unwrap();
    ///
    /// assert_eq!(best_size, BlockSize::best());
    /// ```
    pub fn best() -> BlockSize {
        BlockSize(BROTLI_MAX_INPUT_BLOCK_BITS)
    }

    /// Constructs the worst block size (in bits) to use for brotli compression.
    ///
    /// This will consume the least amount of memory at the expense of compression ratio. Currently
    /// it is set to 16 bits.
    ///
    /// # Examples
    ///
    /// ```
    /// use brotlic::BlockSize;
    ///
    /// let worst_size = BlockSize::new(16).unwrap();
    ///
    /// assert_eq!(worst_size, BlockSize::worst());
    /// ```
    pub fn worst() -> BlockSize {
        BlockSize(BROTLI_MIN_INPUT_BLOCK_BITS)
    }
}

/// An error returned by [`BlockSize::new`].
#[derive(Debug)]
pub struct BlockSizeError;

impl fmt::Display for BlockSizeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "block size out of range (must be between {} and {} inclusive)",
            BROTLI_MIN_INPUT_BLOCK_BITS, BROTLI_MAX_INPUT_BLOCK_BITS
        )
    }
}

impl error::Error for BlockSizeError {}

/// Allows to tune a brotli compressor for a specific type of input.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum CompressionMode {
    /// No known attributes about the input data.
    Generic = BrotliEncoderMode_BROTLI_MODE_GENERIC as isize,

    /// Tune compression for UTF-8 formatted text input.
    Text = BrotliEncoderMode_BROTLI_MODE_TEXT as isize,

    /// Tune compression for WOFF 2.0 fonts
    Font = BrotliEncoderMode_BROTLI_MODE_FONT as isize,
}

impl Default for CompressionMode {
    /// Creates a `CompressionMode` using [`Generic`].
    /// See its documentation for more.
    ///
    /// [`Generic`]: CompressionMode::Generic
    fn default() -> Self {
        CompressionMode::Generic
    }
}

/// An error returned by [`compress`].
#[derive(Debug)]
pub struct CompressionError;

impl fmt::Display for CompressionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("buffer was too small or compression error occurred")
    }
}

impl error::Error for CompressionError {}

/// a specialized [`Result`] type returned by [`compress`].
pub type CompressionResult<T> = Result<T, CompressionError>;

/// An error returned by [`decompress`].
#[derive(Debug)]
pub struct DecompressionError;

impl fmt::Display for DecompressionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("buffer was too small or decompression error occurred")
    }
}

impl error::Error for DecompressionError {}

/// a specialized [`Result`] type returned by [`decompress`].
pub type DecompressionResult<T> = Result<T, DecompressionError>;

/// An error returned by [`BrotliEncoderOptions::build`] and [`BrotliDecoderOptions::build`]
///
/// [`BrotliEncoderOptions::build`]: encode::BrotliEncoderOptions::build
/// [`BrotliDecoderOptions::build`]: decode::BrotliDecoderOptions::build
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ParameterError {
    /// The encoder or decoder returned an error.
    ///
    /// This error originates from `BrotliEncoderSetParameter` or `BrotliDecoderSetParameter` being
    /// unsuccessful.
    Generic,

    /// Postfix bits were out of range.
    InvalidPostfix,

    /// Direct distance codes were out of range or were given in invalid increments.
    InvalidDirectDistanceCodes,

    /// The stream offset was beyond its maximum offset.
    InvalidStreamOffset,
}

impl fmt::Display for ParameterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParameterError::Generic =>
                f.write_str("invalid parameter"),
            ParameterError::InvalidPostfix =>
                f.write_str("invalid number of postfix bits"),
            ParameterError::InvalidDirectDistanceCodes =>
                f.write_str("invalid number of direct distance codes"),
            ParameterError::InvalidStreamOffset =>
                f.write_str("stream offset was out of range"),
        }
    }
}

impl error::Error for ParameterError {}

/// Read all bytes from `input` and compress them into `output`, returning how many bytes were
/// written.
///
/// The compression will use the specified `quality` (see [`Quality`] for more information),
/// `window_size` (see [`WindowSize`] for more information) and `mode` (see [`CompressionMode`] for
/// more information). The compressed `input` using the specified compression settings must fit into
/// `output`, otherwise an error is returned and the compression will be aborted. To get an upper
/// bound when `quality` is 2 or higher, use [`compress_bound`].
///
/// # Errors
///
/// An [`Err`] will be returned if:
///
/// * `output` is not large enough to contain the compressed data
/// * A generic compression error occurs
/// * memory allocation failed
///
/// # Examples
///
/// ```
/// use brotlic::{compress, CompressionMode, Quality, WindowSize};
///
/// let input = vec![0; 1024];
/// let mut output = vec![0; 1024];
///
/// let bytes_written = compress(
///      input.as_slice(),
///      output.as_mut_slice(),
///      Quality::default(),
///      WindowSize::default(),
///      CompressionMode::Generic
/// ).unwrap();
///
/// assert!(bytes_written < input.len());
/// ```
#[doc(alias = "BrotliEncoderCompress")]
pub fn compress(
    input: &[u8],
    output: &mut [u8],
    quality: Quality,
    window_size: WindowSize,
    mode: CompressionMode,
) -> CompressionResult<usize> {
    let mut output_size = output.len();

    let res = unsafe {
        BrotliEncoderCompress(
            quality.0 as c_int,
            window_size.0 as c_int,
            mode as BrotliEncoderMode,
            input.len(),
            input.as_ptr(),
            &mut output_size as *mut usize,
            output.as_mut_ptr(),
        )
    };

    if res != 0 {
        Ok(output_size)
    } else {
        Err(CompressionError)
    }
}

/// Returns an upper bound for compression.
///
/// Given an input of `input_size` bytes in size and a `quality`, determine an upper bound for
/// compression. This may be larger than `input_size`. The result is only valid for a quality of at
/// least `2`, as per documentation of `BrotliEncoderMaxCompressedSize`. For qualities lower than
/// `2`, `None` will be returned.
#[doc(alias = "BrotliEncoderMaxCompressedSize")]
pub fn compress_bound(input_size: usize, quality: Quality) -> Option<usize> {
    if quality.0 >= 2 {
        Some(unsafe { BrotliEncoderMaxCompressedSize(input_size) })
    } else {
        None
    }
}

/// Read all bytes from `input` and decompress them into `output`, returning how many bytes were
/// written.
///
/// The uncompressed `input` must fit into `output`, otherwise an error is returned and the
/// decompression will be aborted.
///
/// # Errors
///
/// An [`Err`] will be returned if:
///
/// * `input` is corrupted
/// * memory allocation failed
/// * `output` is not large enough to hold uncompressed `input`
///
/// # Examples
///
/// ```
/// use brotlic::{compress, CompressionMode, decompress, Quality, WindowSize};
///
/// let input = vec![0; 1024];
/// let mut encoded = vec![1; 1024];
/// let mut decoded = vec![2; 1024];
///
/// let bytes_written = compress(
///      input.as_slice(),
///      encoded.as_mut_slice(),
///      Quality::default(),
///      WindowSize::default(),
///      CompressionMode::Generic
/// ).unwrap();
///
/// let encoded = &encoded[..bytes_written];
/// let bytes_written = decompress(encoded, decoded.as_mut_slice()).unwrap();
/// let decoded = &decoded[..bytes_written];
///
/// assert_eq!(input, decoded);
/// ```
#[doc(alias = "BrotliDecoderDecompress")]
pub fn decompress(input: &[u8], output: &mut [u8]) -> DecompressionResult<usize> {
    let mut output_size = output.len();

    let res = unsafe {
        BrotliDecoderDecompress(
            input.len(),
            input.as_ptr(),
            &mut output_size as *mut usize,
            output.as_mut_ptr(),
        )
    };

    if res == BrotliDecoderResult_BROTLI_DECODER_RESULT_SUCCESS {
        Ok(output_size)
    } else {
        Err(DecompressionError)
    }
}

/// An error returned by `into_inner`.
///
/// This error combines an error that happened while processing data, and the instance
/// object which may be used to recover from the condition.
#[derive(Debug)]
pub struct IntoInnerError<I>(I, io::Error);

impl<I> IntoInnerError<I> {
    fn new(instance: I, error: io::Error) -> Self {
        Self(instance, error)
    }

    /// Returns the error which caused the call to `into_inner()` to fail.
    pub fn error(&self) -> &io::Error {
        &self.1
    }

    /// Returns the instance which generated the error
    pub fn into_inner(self) -> I {
        self.0
    }

    /// Returns the error which caused the `into_inner` call to fail. This is used to obtain
    /// ownership of the error in contrast to [`error`].
    pub fn into_error(self) -> io::Error {
        self.1
    }

    /// Returns both the error and the instance that generated it. This is used to obtain ownership
    /// of both of them.
    pub fn into_parts(self) -> (io::Error, I) {
        (self.1, self.0)
    }
}

impl<I> From<IntoInnerError<I>> for io::Error {
    fn from(iie: IntoInnerError<I>) -> io::Error {
        iie.1
    }
}

impl<I: fmt::Debug> error::Error for IntoInnerError<I> {}

impl<I> fmt::Display for IntoInnerError<I> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error().fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Read, Write};
    use super::*;
    use super::encode::*;
    use super::decode::*;

    #[test]
    fn roundtrip() {
        let input = vec![0; 8192];

        let encoded = {
            let mut input_stream = Cursor::new(Vec::new());
            let mut compressed_stream = CompressorWriter::new(input_stream);
            compressed_stream.write_all(input.as_slice());

            compressed_stream.into_inner().unwrap().into_inner()
        };

        let decoded = {
            let mut input_stream = Cursor::new(encoded);
            let mut decompressed_stream = DecompressorReader::new(input_stream);
            let mut output = Vec::new();

            decompressed_stream.read_to_end(&mut output).unwrap();
            output
        };

        assert_eq!(input, decoded);
    }
}