# jpegxr - a Rust library for decoding JPEG XR images

This is a Rust wrapper around the C libjpegxr / jxrlib codec open-sourced by Microsoft. The code is included in-tree with the Rust code as it's no longer actively maintained, and the Codeplex source downloads may not last.

Currently only decoding is supported, but adding an encoder interface should be straightforward.

It is in a very early state, and probably only compiles on Windows at the moment but will shortly be fixed to be cross-platform.

# Authors

The wrapped C JPEG XR library was written by many fine folks at Microsoft!
Rust code wrapping it, and tweaks to the C code, is by Brion Vibber `<brion @ pobox.com>`.

# License

BSD-style license; see `license.md` or the headers in source files.

# Usage

```rust
use fs;
use jpegxr::{ImageDecoder, PixelFormat::*};

// ...

let input = File::open(filename)?;
let mut decoder = ImageDecode::with_reader(input)?;

let format = decoder.get_pixel_format()?;
if format != PixelFormat128bppRGBAFloat {
    panic!("not the pixel format I expected! abort!");
}
let (width, height) = decoder.get_size()?;
// This format has a 16-byte-per-pixel size
let stride = width * 16;
let size = stride * height;
let buffer = Vec::<u8>::with_capacity(size);
decoder.copy_all(&buffer, stride)?;

// now do stuff with the data
```

# Features

Currently sports the ability to read basic image format (width/height/pixel format) from a JPEG XR image and decode its data to memory. Plan to add encoding, all the pieces are there, just haven't set it up yet.

You can ask for a subset of the image, which should allow decoding only the requested macroblocks.
It may be possible to do parallelized decoding by using multiple decoders pulling from the same input data set, as long as their cursors don't interfere.
This has not been tested yet.

HDR images with 32-bit floating point RGBA elements, as saved from the NVIDIA game screen capture tool, appear to decode correctly.


# Future plans

* test and fix on Linux
* test and fix on Mac
* add encoder interface
* performance / multithreading
* more testing of obscure stuff
