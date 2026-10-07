use crate::i18n::{self, Key};
use gtk4::prelude::*;
use gtk4::{gdk, gio, glib};
use image::{ImageDecoder, ImageEncoder, ImageReader};
use memedock_core::{ArtifactLease, Library, ResourceLimits};
use std::cell::RefCell;
use std::io::{self, Write};
use std::rc::Rc;
use std::sync::Arc;

#[derive(Clone)]
pub struct Prepared {
    pub provider: gdk::ContentProvider,
    pub lease: Arc<ArtifactLease>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Representation {
    File,
    Image,
    Drag,
}

pub fn file_provider(lease: &ArtifactLease) -> gdk::ContentProvider {
    let files = gdk::FileList::from_array(&[gio::File::for_path(&lease.metadata().path)]);
    gdk::ContentProvider::for_value(&files.to_value())
}

/// Decode/encode off the GTK thread. The final texture only wraps owned RGBA bytes.
pub async fn prepare(
    lease: Arc<ArtifactLease>,
    representation: Representation,
) -> Result<Prepared, &'static str> {
    let file = file_provider(&lease);
    if representation == Representation::File {
        return Ok(Prepared {
            provider: file,
            lease,
        });
    }
    let metadata = lease.metadata().clone();
    let bytes = crate::output::read_file(&metadata.path).await?;
    let mut providers = Vec::new();
    if metadata.animated {
        providers.push(gdk::ContentProvider::for_bytes(
            &metadata.mime,
            &glib::Bytes::from_owned(bytes),
        ));
    } else {
        let (decoded, original) = gio::spawn_blocking(move || (decode_static(&bytes), bytes))
            .await
            .map_err(|_| i18n::text(Key::FailureUnknown))?;
        let has_image = match decoded {
            Ok(decoded) => {
                let png = glib::Bytes::from_owned(decoded.png);
                let texture = gdk::MemoryTexture::new(
                    decoded.width,
                    decoded.height,
                    gdk::MemoryFormat::R8g8b8a8,
                    &glib::Bytes::from_owned(decoded.rgba),
                    decoded.stride,
                );
                providers.push(gdk::ContentProvider::for_bytes("image/png", &png));
                providers.push(gdk::ContentProvider::for_value(
                    &texture.upcast::<gdk::Texture>().to_value(),
                ));
                true
            }
            // Dragging the verified original remains available when an optional
            // image representation exceeds its budget or has an unsupported profile.
            Err(_) if representation == Representation::Drag => false,
            Err(message) => return Err(message),
        };
        if metadata.mime != "image/png" || !has_image {
            providers.push(gdk::ContentProvider::for_bytes(
                &metadata.mime,
                &glib::Bytes::from_owned(original),
            ));
        }
    }
    providers.push(file);
    Ok(Prepared {
        provider: gdk::ContentProvider::new_union(&providers),
        lease,
    })
}

struct StaticImage {
    width: i32,
    height: i32,
    stride: usize,
    rgba: Vec<u8>,
    png: Vec<u8>,
}

struct BoundedPng(Vec<u8>);
impl Write for BoundedPng {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let limit = ResourceLimits::default().max_file_bytes;
        if (self.0.len() as u64).saturating_add(bytes.len() as u64) > limit {
            return Err(io::Error::other("clipboard PNG exceeds output limit"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn decode_static(bytes: &[u8]) -> Result<StaticImage, &'static str> {
    let limits = ResourceLimits::default();
    let mut reader = ImageReader::new(io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| i18n::text(Key::FailureImage))?;
    let mut bounds = image::Limits::default();
    bounds.max_alloc = Some(limits.max_decode_bytes);
    reader.limits(bounds);
    let mut decoder = reader
        .into_decoder()
        .map_err(|_| i18n::text(Key::FailureImage))?;
    let (w, h) = decoder.dimensions();
    if w == 0
        || h == 0
        || u64::from(w) * u64::from(h) > limits.max_frame_pixels
        || u64::from(w) * u64::from(h) * 16 > limits.max_decode_bytes
    {
        return Err(i18n::text(Key::FailureLimit));
    }
    let orientation = decoder
        .orientation()
        .map_err(|_| i18n::text(Key::FailureImage))?;
    if decoder
        .icc_profile()
        .map_err(|_| i18n::text(Key::FailureImage))?
        .is_some()
    {
        return Err(i18n::text(Key::FailureColorProfile));
    }
    let mut image =
        image::DynamicImage::from_decoder(decoder).map_err(|_| i18n::text(Key::FailureImage))?;
    image.apply_orientation(orientation);
    let rgba = image.into_rgba8();
    let mut png = BoundedPng(Vec::new());
    image::codecs::png::PngEncoder::new(&mut png)
        .write_image(
            rgba.as_raw(),
            rgba.width(),
            rgba.height(),
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|_| i18n::text(Key::FailureLimit))?;
    Ok(StaticImage {
        width: i32::try_from(rgba.width()).map_err(|_| i18n::text(Key::FailureLimit))?,
        height: i32::try_from(rgba.height()).map_err(|_| i18n::text(Key::FailureLimit))?,
        stride: rgba.width() as usize * 4,
        rgba: rgba.into_raw(),
        png: png.0,
    })
}

#[derive(Default)]
pub struct State {
    current: Option<Prepared>,
    connection: Option<(gdk::Clipboard, glib::SignalHandlerId)>,
    closing: bool,
}

pub fn watch(state: &Rc<RefCell<State>>, clipboard: &gdk::Clipboard) {
    if state.borrow().connection.is_some() {
        return;
    }
    let weak = Rc::downgrade(state);
    let handler = clipboard.connect_changed(move |clipboard| {
        let Some(state) = weak.upgrade() else {
            return;
        };
        let mut value = state.borrow_mut();
        if value.closing {
            return;
        }
        if value
            .current
            .as_ref()
            .is_some_and(|current| clipboard.content().as_ref() != Some(&current.provider))
        {
            value.current.take();
            // A clipboard manager may now own the same file URI. Ownership loss
            // releases display memory, but is not proof that the durable pin is
            // obsolete. The next successful app copy reconciles that pin.
        }
    });
    state.borrow_mut().connection = Some((clipboard.clone(), handler));
}

pub async fn install(
    state: &Rc<RefCell<State>>,
    clipboard: &gdk::Clipboard,
    library: &Library,
    prepared: Prepared,
) -> Result<(), &'static str> {
    if state.borrow().closing {
        return Err(i18n::text(Key::FailureUnknown));
    }
    let protected = match library.protect_clipboard(Arc::clone(&prepared.lease)) {
        Ok(task) => task.wait().await,
        Err(error) => Err(error),
    };
    let reference = match protected {
        Ok(reference) => reference,
        Err(error) => {
            return Err(i18n::core(error.code()));
        }
    };
    if clipboard.set_content(Some(&prepared.provider)).is_err() {
        if let Ok(task) = library.abort_clipboard(reference) {
            let _ = task.wait().await;
        }
        // The previous provider and its lease have not been replaced.
        return Err(i18n::text(Key::FailureUnknown));
    }
    state.borrow_mut().current = Some(prepared);
    // Our successful set is a positive observation of replacement. Unknown
    // foreign ownership never gets translated into reconcile_clipboard(None).
    match library.reconcile_clipboard(Some(reference)) {
        Ok(task) => task.wait().await.map_err(|error| i18n::core(error.code())),
        Err(error) => Err(i18n::core(error.code())),
    }
}

pub fn begin_shutdown(state: &Rc<RefCell<State>>) {
    let connection = {
        let mut value = state.borrow_mut();
        value.closing = true;
        value.connection.take()
    };
    if let Some((clipboard, handler)) = connection {
        clipboard.disconnect(handler);
    }
}

impl State {
    pub fn release(&mut self) {
        self.current.take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn static_payload_preserves_transparency_and_rejects_broken_images()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut source = io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            2,
            3,
            image::Rgba([7, 9, 11, 0]),
        ))
        .write_to(&mut source, image::ImageFormat::Png)?;
        let payload = decode_static(&source.into_inner()).map_err(io::Error::other)?;
        assert_eq!((payload.width, payload.height), (2, 3));
        assert_eq!(
            image::load_from_memory(&payload.png)?
                .to_rgba8()
                .get_pixel(0, 0)
                .0,
            [7, 9, 11, 0]
        );
        assert_eq!(payload.rgba.len(), payload.stride * 3);
        assert!(decode_static(b"broken").is_err());
        Ok(())
    }
}
