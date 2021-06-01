/// Image support for genpdf-rs.

use std::path;
use image::GenericImageView;

use crate::{Alignment, Context, Element, Mm, Position, RenderResult, Rotation, Scale, Size};
use crate::error::Error;
use crate::{render, style};

/// An Image to embed in the PDF.
///
/// This struct is merely a wrapper around the configurations [`printpdf`][] exposes.
///
/// # Example:
/// ```rust
/// use std::convert::TryFrom;
/// use genpdf::{elements, Scale};
/// let image = elements::Image::from_path("examples/images/test_image.jpg")
///       .expect("Image loaded from test image.")
///       .with_alignment(elements::Alignment::Center) // Center the image on the page.
///       .with_scale(genpdf::Scale::new(0.5, 2)); // Squeeze and then stretch upwards.
/// ```
#[derive(Clone)]
pub struct Image {
    data: image::DynamicImage,

    /// Used for positioning if no absolute position is given.
    alignment: Alignment,

    /// The absolute position within the given area.
    ///
    /// If no position is set, we use the Alignment.
    position: Option<Position>,

    /// Scaling of the image, default is 1:1.
    scale: Scale,

    /// The number of degrees of clockwise rotation.
    rotation: Rotation,

    /// DPI override if you know better. Defaults to let `printpdf` define it.
    dpi: Option<f64>,
}

impl Image {
    /// If somehow you have a dynamic image directly from the image package.
    pub fn from_dynamic_image(data: image::DynamicImage) -> Self {
        Image {
            data,
            alignment: Alignment::default(),
            position: None,
            scale: Scale::default(),
            rotation: Rotation::default(),
            dpi: None,
        }
    }

    /// If you have a reader, we can pass that to the image library to pull it in.
    pub fn from_reader<R>(reader: R) -> Result<Self, Error>
        where
            R: std::io::BufRead,
            R: std::io::Read,
            R: std::io::Seek,
    {
        let data = image::io::Reader::new(reader)
            .with_guessed_format()
            .map_err(|e| Error::new("Unable to determine image format.", e))?
            .decode()
            .map_err(|e| Error::new("Unable to decode image.", e))?;
        Ok(Image::from_dynamic_image(data))
    }

    /// Try to convert a path into an Image. Unable to use TryFrom since Path isn't Sized.
    pub fn from_path(path: impl AsRef<path::Path>) -> Result<Self, Error> {
        // currently, the only reliable file formats are bmp/jpeg/png
        // this is an issue of the image library, not a fault of printpdf
        image::io::Reader::open(path)
            .map_err(|e| Error::new("Unable to open path.", e))?
            .with_guessed_format()
            .map_err(|e| Error::new("Unable to determine image format.", e))?
            .decode()
            .map_err(|e| Error::new("Unable to decode image.", e))
            .map(Image::from_dynamic_image)
    }

    /// Translate the image over to position.
    pub fn set_position(&mut self, position: impl Into<Position>) {
        self.position = Some(position.into());
    }

    /// Translate the image over to position and return the image for chaining.
    pub fn with_position(mut self, position: impl Into<Position>) -> Self {
        self.set_position(position);
        self
    }

    /// Scale the image.
    pub fn set_scale(&mut self, scale: impl Into<Scale>) {
        self.scale = scale.into();
    }

    /// Scale the image and return the image for chaining.
    pub fn with_scale(mut self, scale: impl Into<Scale>) -> Self {
        self.set_scale(scale);
        self
    }

    /// Calculate positioning based on the image and document size.
    pub fn set_alignment(&mut self, alignment: impl Into<Alignment>) {
        self.alignment = alignment.into();
    }

    /// Set the alignment to use and return the image for chaining.
    pub fn with_alignment(mut self, alignment: impl Into<Alignment>) -> Self {
        self.set_alignment(alignment);
        self
    }

    /// Determine the offset from left-side based on provided Alignment.
    fn get_offset(&self, width: Mm, max_width: Mm) -> Position {
        let horizontal_offset: Mm = match self.alignment {
            Alignment::Left => Mm::default(),
            Alignment::Center => (max_width - width) / 2.0,
            Alignment::Right => max_width - width,
        };
        Position::new(horizontal_offset, 0)
    }

    /// Calculates a guess for the size of the image based on the dpi/pixel-count/scale.
    fn get_size(&self) -> Size {
        let mmpi: f64 = 25.4; // millimeters per inch
        // Assume 300 DPI to be consistent with printpdf.
        let dpi: f64 = self.dpi.unwrap_or(300.0);
        let (px_width, px_height) = self.data.dimensions();
        let (scale_width, scale_height): (f64, f64) = (self.scale.x, self.scale.y);
        Size::new(
            mmpi * ((scale_width * px_width as f64) / dpi),
            mmpi * ((scale_height * px_height as f64) / dpi)
        )
    }

    /// Set the clockwise rotation of the image around the bottom left corner.
    pub fn set_clockwise_rotation(&mut self, rotation: impl Into<Rotation>) {
        self.rotation = rotation.into();
    }

    /// Set the clockwise rotation of the image around the bottom left corner;
    /// and then return it for chaining.
    pub fn with_clockwise_rotation(mut self, rotation: impl Into<Rotation>) -> Self {
        self.set_clockwise_rotation(rotation);
        self
    }

    /// Set the expected DPI of the encoded Image.
    pub fn set_dpi(&mut self, dpi: f64) {
        self.dpi = Some(dpi);
    }

    /// Set the expected DPI of the encoded Image and then return for chaining.
    pub fn with_dpi(mut self, dpi: f64) -> Self {
        self.set_dpi(dpi);
        self
    }
}

impl Element for Image {
    fn render(
        &mut self,
        _context: &Context,
        area: render::Area<'_>,
        _style: style::Style
    ) -> Result<RenderResult, Error> {
        let mut result = RenderResult::default();
        let true_size = self.get_size();
        let (bb_origin, bb_size) = bounding_box_offset_and_size(&self.rotation, &true_size);

        let mut position: Position = if let Some(position) = self.position {
            position
        } else {
            // Update the result size to be based on the bounding-box size/offset.
            result.size = bb_size;

            // No position override given; so we calculate the Alignment offset based on
            // the area-size and width of the bounding box.
            self.get_offset(bb_size.width, area.size().width)
        };

        // Fix the position with the bounding-box's origin which was changed from
        // (0,0) when it was rotated in any way.
        position += bb_origin;

        // Insert/Render the image with the overridden/calculated position.
        area.add_image(&self.data, position, self.scale, self.rotation, self.dpi);

        // Always false as we can't safely do this unless we want to try to do "sub-images".
        // This is technically possible with the `image` package, but it is potentially more
        // work than necessary. I'd rather support an "Auto-Scale" method to fit to area.
        result.has_more = false;

        Ok(result)
    }
}

/// Given the Size of a box (width/height), compute the bounding-box size when
/// rotated some degrees and where the "minimum" corner is (which should be the
/// new origin/offset). Note, this is not very optimized.
fn bounding_box_offset_and_size(rotation: &Rotation, size: &Size) -> (Position, Size) {
    let theta = rotation.degrees.to_radians();
    let (ct, st) = (theta.cos(), theta.sin());
    let (w, h): (f64, f64) = (size.width.into(), size.height.into());
    match rotation.degrees {
        d if d >    0.0 && d <=    90.0 => {
            let alpha = 180.0 - (rotation.degrees + 90.0);
            let ca = alpha.to_radians().cos();
            let (hct, wct) = (h * ct, w * ct);
            let (hst, wst) = (h * st, w * st);
            let (bb_w, bb_h) = (hst + wct, wst + hct);
            (Position::new(h * ca, bb_h), Size::new(bb_w, bb_h))
        },
        d if d >  90.0 && d <=  180.0 => {
            let alpha = (rotation.degrees - 90.0).to_radians();
            let (ca, sa) = (alpha.cos(), alpha.sin());
            let (bb_w, bb_h) = (w*sa + h*ca, w*ca + h*sa);
            (Position::new(bb_w, w*ca), Size::new(bb_w, bb_h))
        },
        d if d <   0.0 && d >   -90.0 => {
            let (hct, wct) = (h * ct, w * ct);
            let (hst, wst) = (h * st, w * st);
            let (bb_w, bb_h) = (hst + wct, hct + wst);
            (Position::new(0, hct), Size::new(bb_w, bb_h))
        },
        d if d <= -90.0 && d >= -180.0 => {
            let alpha = (180.0 + rotation.degrees).to_radians();
            let (ca, sa) = (alpha.cos(), alpha.sin());
            let (bb_w, bb_h) = (h*sa + w*ca, h*ca + w*sa);
            (Position::new(w*ca, 0), Size::new(bb_w, bb_h))
        },
        _ =>
        // This section is only for degrees == 0.0, but I use the default match due to:
        //       https://github.com/rust-lang/rust/issues/41620
        // Rotation's degrees should be restricted to [-180,180] so these
        // ranges should be complete.
            (Position::new(0, h), size.clone()),
    }
}
