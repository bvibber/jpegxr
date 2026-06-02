use std::convert::TryFrom;
use std::io::{self, Read, Seek};

use crate::{ImageDecode, JXRError, PixelFormat};
use image::error::{
    DecodingError, ImageFormatHint, LimitError, LimitErrorKind, ParameterError, ParameterErrorKind,
    UnsupportedError, UnsupportedErrorKind,
};
use image::ImageError::Decoding;
use image::{
    ColorType, DynamicImage, GrayImage, ImageDecoder, ImageError, ImageResult, RgbImage, RgbaImage,
};

pub fn register_decoding_hook() -> bool {
    let decode_registered = image::hooks::register_decoding_hook(
        "jxr".into(),
        Box::new(|r| Ok(Box::new(JxrDecoder::new(r)?))),
    );

    // little-endian: II BC 01
    image::hooks::register_format_detection_hook(
        "jxr".into(),
        &[0x49, 0x49, 0xBC, 0x01],
        Some(&[0xFF, 0xFF, 0xFF, 0xFF]),
    );

    // big-endian: MM 01 BC
    image::hooks::register_format_detection_hook(
        "jxr".into(),
        &[0x4D, 0x4D, 0x01, 0xBC],
        Some(&[0xFF, 0xFF, 0xFF, 0xFF]),
    );

    decode_registered
}

fn fmt_hint() -> ImageFormatHint {
    ImageFormatHint::Name("JPEG XR".into())
}

fn map_jxr_error(err: JXRError) -> ImageError {
    match err {
        JXRError::IoError(e) => ImageError::IoError(e),

        JXRError::UnsupportedFormat => {
            ImageError::Unsupported(UnsupportedError::from_format_and_kind(
                fmt_hint(),
                UnsupportedErrorKind::GenericFeature("unsupported JPEG XR feature".into()),
            ))
        }

        JXRError::OutOfMemory | JXRError::BufferOverflow => {
            ImageError::Limits(LimitError::from_kind(LimitErrorKind::InsufficientMemory))
        }

        JXRError::InvalidData
        | JXRError::InvalidArgument
        | JXRError::InvalidParameter
        | JXRError::UnrecognizedPixelFormat
        | JXRError::UnrecognizedColorFormat
        | JXRError::UnrecognizedInterpretation
        | JXRError::UnrecognizedBitDepth => ImageError::Parameter(ParameterError::from_kind(
            ParameterErrorKind::Generic(err.to_string()),
        )),

        other => Decoding(DecodingError::new(fmt_hint(), other)),
    }
}

fn warn_hook(msg: &str) {
    eprintln!("jpegxr/image_hook warning: {msg}");
}

fn choose_output_layout(fmt: PixelFormat) -> ImageResult<(ColorType, usize, bool)> {
    use PixelFormat::*;

    let out = match fmt {
        PixelFormat8bppGray => (ColorType::L8, 1usize, false),
        PixelFormat16bppGray => (ColorType::L16, 2usize, false),
        PixelFormat24bppRGB => (ColorType::Rgb8, 3usize, false),
        PixelFormat24bppBGR => (ColorType::Rgb8, 3usize, true),
        PixelFormat32bppRGBA => (ColorType::Rgba8, 4usize, false),
        PixelFormat32bppBGRA | PixelFormat32bppPBGRA => (ColorType::Rgba8, 4usize, true),
        PixelFormat48bppRGB => (ColorType::Rgb16, 6usize, false),
        PixelFormat64bppRGBA => (ColorType::Rgba16, 8usize, false),
        PixelFormat96bppRGBFloat => (ColorType::Rgb32F, 12usize, false),
        PixelFormat128bppRGBAFloat => (ColorType::Rgba32F, 16usize, false),

        PixelFormat64bppPRGBA => {
            warn_hook("premultiplied alpha decoded into RGBA16 container (values preserved)");
            (ColorType::Rgba16, 8usize, false)
        }
        PixelFormat128bppPRGBAFloat => {
            warn_hook("premultiplied alpha decoded into RGBA32F container (values preserved)");
            (ColorType::Rgba32F, 16usize, false)
        }

        _ => {
            return Err(ImageError::Unsupported(
                UnsupportedError::from_format_and_kind(
                    fmt_hint(),
                    UnsupportedErrorKind::GenericFeature(format!(
                        "pixel format {fmt:?} (lossless mapping unavailable)"
                    )),
                ),
            ));
        }
    };

    Ok(out)
}

pub struct JxrDecoder<R: Read + Seek> {
    dec: ImageDecode<R>,
    width: u32,
    height: u32,
    color: ColorType,
    stride: usize,
    swap_rb: bool,
}

impl<R> JxrDecoder<R>
where
    R: Read + Seek,
{
    pub fn new(reader: R) -> ImageResult<Self> {
        let dec = ImageDecode::with_reader(reader).map_err(map_jxr_error)?;
        let (w_i32, h_i32) = dec.get_size().map_err(map_jxr_error)?;

        let width = u32::try_from(w_i32).map_err(|e| {
            ImageError::Parameter(ParameterError::from_kind(ParameterErrorKind::Generic(
                format!("invalid JPEG XR width {w_i32}: {e}"),
            )))
        })?;

        let height = u32::try_from(h_i32).map_err(|e| {
            ImageError::Parameter(ParameterError::from_kind(ParameterErrorKind::Generic(
                format!("invalid JPEG XR height {h_i32}: {e}"),
            )))
        })?;
        if width == 0 || height == 0 {
            return Err(Decoding(DecodingError::new(
                fmt_hint(),
                JXRError::InvalidData,
            )));
        }

        let src_fmt = dec.get_pixel_format().map_err(map_jxr_error)?;

        let (color, bpp, swap_rb) = choose_output_layout(src_fmt)?;

        let stride = (width as usize).checked_mul(bpp).ok_or_else(|| {
            ImageError::Limits(LimitError::from_kind(LimitErrorKind::InsufficientMemory))
        })?;

        Ok(Self {
            dec,
            width,
            height,
            color,
            stride,
            swap_rb,
        })
    }
}

impl<R: Read + Seek> ImageDecoder for JxrDecoder<R> {
    fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    fn color_type(&self) -> ColorType {
        self.color
    }

    fn read_image(mut self, buf: &mut [u8]) -> ImageResult<()> {
        let expected = self.total_bytes() as usize;
        if buf.len() != expected {
            return Err(ImageError::Parameter(ParameterError::from_kind(
                ParameterErrorKind::Generic(format!(
                    "output buffer length mismatch: got {}, expected {}",
                    buf.len(),
                    expected
                )),
            )));
        }

        self.dec.copy_all(buf, self.stride).map_err(map_jxr_error)?;

        if self.swap_rb {
            let px_size = self.color.bytes_per_pixel() as usize;
            for px in buf.chunks_exact_mut(px_size) {
                px.swap(0, 2);
            }
        }

        Ok(())
    }

    fn read_image_boxed(self: Box<Self>, buf: &mut [u8]) -> ImageResult<()> {
        (*self).read_image(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::register_decoding_hook;
    use image::{ImageError, ImageReader};
    use std::fs;
    use std::io::Cursor;
    use std::path::Path;
    use std::sync::Once;

    static REGISTER_HOOK_ONCE: Once = Once::new();

    fn ensure_hook_registered() {
        REGISTER_HOOK_ONCE.call_once(|| {
            let _ = register_decoding_hook();
        });
    }

    #[test]
    fn decoding_hook_can_save_panel_hdr_lossless_exr() {
        ensure_hook_registered();

        let bytes = fs::read("samples/panel-hdr.jxr").expect("sample JXR file should be readable");

        let img = ImageReader::new(Cursor::new(bytes))
            .with_guessed_format()
            .expect("format detection should succeed")
            .decode()
            .expect("JXR decode via hook should succeed");

        let out = Path::new("samples/panel-hdr.exr");

        img.save_with_format(&out, image::ImageFormat::OpenExr)
            .expect("saving EXR should succeed");

        let roundtrip = ImageReader::open(&out)
            .expect("open written EXR")
            .decode()
            .expect("decode written EXR");

        assert_eq!(img.width(), roundtrip.width());
        assert_eq!(img.height(), roundtrip.height());

        match (img, roundtrip) {
            (image::DynamicImage::ImageRgba32F(a), image::DynamicImage::ImageRgba32F(b)) => {
                assert_eq!(a.as_raw(), b.as_raw(), "EXR roundtrip should be lossless");
            }
            (image::DynamicImage::ImageRgb32F(a), image::DynamicImage::ImageRgb32F(b)) => {
                assert_eq!(a.as_raw(), b.as_raw(), "EXR roundtrip should be lossless");
            }
            (a, b) => panic!(
                "unexpected image variants: {:?} vs {:?}",
                a.color(),
                b.color()
            ),
        }
    }
}
