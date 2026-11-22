//! Keep an artist's ICO frames intact. Generate shell sizes from a PNG and use the same art for Start.
use crate::Result;
use image::{
    DynamicImage, ExtendedColorType, ImageFormat, RgbaImage,
    codecs::ico::{IcoEncoder, IcoFrame},
    imageops::{self, FilterType},
};
use std::{fs, io::BufWriter, path::Path};

pub(crate) struct Artwork {
    image: DynamicImage,
    original_ico: Option<Vec<u8>>,
}
impl Artwork {
    pub fn load(path: Option<&Path>) -> Result<Self> {
        let bytes = match path {
            Some(path) => fs::read(path)
                .map_err(|e| format!("Cannot read app icon {}: {e}", path.display()))?,
            None => include_bytes!("icon/rustybox.png").to_vec(),
        };
        let format = image::guess_format(&bytes)?;
        if !matches!(format, ImageFormat::Png | ImageFormat::Ico) {
            return Err("Choose a PNG or Windows ICO file for icon".into());
        }
        let image = image::load_from_memory_with_format(&bytes, format)?;
        Ok(Self {
            image,
            original_ico: (format == ImageFormat::Ico).then_some(bytes),
        })
    }

    fn square(&self, size: u32) -> RgbaImage {
        let image = self
            .image
            .resize(size, size, FilterType::Lanczos3)
            .to_rgba8();
        let mut canvas = RgbaImage::new(size, size);
        imageops::replace(
            &mut canvas,
            &image,
            ((size - image.width()) / 2).into(),
            ((size - image.height()) / 2).into(),
        );
        canvas
    }

    pub fn write_ico(&self, path: &Path) -> Result<()> {
        if let Some(original) = &self.original_ico {
            fs::write(path, original)?;
            return Ok(());
        }
        let mut frames = Vec::new();
        for size in [16, 20, 24, 32, 40, 48, 64, 128, 256] {
            frames.push(IcoFrame::as_png(
                self.square(size).as_raw(),
                size,
                size,
                ExtendedColorType::Rgba8,
            )?);
        }
        use std::io::Write;
        let mut output = BufWriter::new(fs::File::create(path)?);
        IcoEncoder::new(&mut output).encode_images(&frames)?;
        output.flush()?;
        Ok(())
    }

    pub fn write_tiles(&self, directory: &Path) -> Result<()> {
        fs::create_dir_all(directory)?;
        for size in [44, 50, 150] {
            self.square(size)
                .save(directory.join(format!("{size}.png")))?;
        }
        Ok(())
    }
}
