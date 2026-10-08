use gtk4::{gdk, gio, glib, prelude::*};
use memedock_core::{Library, sources::telegram::TelegramPack};
use memedock_domain::source::SourceItemId;
use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
    rc::Rc,
    sync::Arc,
};

struct Request {
    id: SourceItemId,
    picture: glib::WeakRef<gtk4::Picture>,
}
pub struct Queue {
    library: Library,
    pack: Arc<TelegramPack>,
    pending: VecDeque<Request>,
    active: HashMap<SourceItemId, glib::JoinHandle<()>>,
    cache: HashMap<SourceItemId, gdk::Texture>,
    order: VecDeque<SourceItemId>,
    bytes: usize,
}
impl Queue {
    pub fn new(library: Library, pack: Arc<TelegramPack>) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            library,
            pack,
            pending: VecDeque::new(),
            active: HashMap::new(),
            cache: HashMap::new(),
            order: VecDeque::new(),
            bytes: 0,
        }))
    }
    pub fn request(queue: &Rc<RefCell<Self>>, id: SourceItemId, picture: &gtk4::Picture) {
        let cached = {
            let mut q = queue.borrow_mut();
            if let Some(texture) = q.cache.get(&id).cloned() {
                q.order.retain(|k| k != &id);
                q.order.push_back(id);
                Some(texture)
            } else {
                q.pending.push_back(Request {
                    id,
                    picture: picture.downgrade(),
                });
                None
            }
        };
        if let Some(texture) = cached {
            picture.set_paintable(Some(&texture));
            return;
        }
        Self::pump(queue);
    }
    pub fn cancel(queue: &Rc<RefCell<Self>>, id: &SourceItemId) {
        let task = {
            let mut q = queue.borrow_mut();
            q.pending.retain(|r| &r.id != id);
            q.active.remove(id)
        };
        if let Some(task) = task {
            task.abort();
        }
        Self::pump(queue);
    }
    pub fn stop(queue: &Rc<RefCell<Self>>) {
        let tasks = {
            let mut q = queue.borrow_mut();
            q.pending.clear();
            std::mem::take(&mut q.active)
        };
        for (_, task) in tasks {
            task.abort();
        }
    }
    fn pump(queue: &Rc<RefCell<Self>>) {
        loop {
            let next = {
                let mut q = queue.borrow_mut();
                if q.active.len() >= 2 {
                    return;
                }
                q.pending
                    .pop_front()
                    .map(|r| (r, q.library.clone(), Arc::clone(&q.pack)))
            };
            let Some((request, library, pack)) = next else {
                return;
            };
            if request.picture.upgrade().is_none() {
                continue;
            }
            let id = request.id.clone();
            let weak = Rc::downgrade(queue);
            let task = glib::spawn_future_local(async move {
                let result = async {
                    let preview = library
                        .telegram_preview(pack, request.id.clone())
                        .map_err(|_| ())?
                        .wait()
                        .await
                        .map_err(|_| ())?;
                    let pixels = gio::spawn_blocking(move || -> Result<(u32, u32, Vec<u8>), ()> {
                        let file = std::fs::File::open(preview.path).map_err(|_| ())?;
                        let mut reader = image::ImageReader::new(std::io::BufReader::new(file))
                            .with_guessed_format()
                            .map_err(|_| ())?;
                        let mut limits = image::Limits::default();
                        limits.max_image_width = Some(256);
                        limits.max_image_height = Some(256);
                        limits.max_alloc = Some(1024 * 1024);
                        reader.limits(limits);
                        let image = reader.decode().map_err(|_| ())?.into_rgba8();
                        if image.width() > 256 || image.height() > 256 {
                            return Err(());
                        }
                        Ok((image.width(), image.height(), image.into_raw()))
                    })
                    .await
                    .map_err(|_| ())??;
                    Ok::<_, ()>(pixels)
                }
                .await;
                let Some(queue) = weak.upgrade() else {
                    return;
                };
                if let Ok((width, height, pixels)) = result {
                    let texture = gdk::MemoryTexture::new(
                        i32::try_from(width).unwrap_or(256),
                        i32::try_from(height).unwrap_or(256),
                        gdk::MemoryFormat::R8g8b8a8,
                        &glib::Bytes::from_owned(pixels),
                        width as usize * 4,
                    )
                    .upcast::<gdk::Texture>();
                    if let Some(picture) = request.picture.upgrade() {
                        picture.set_paintable(Some(&texture));
                    }
                    let mut q = queue.borrow_mut();
                    q.bytes += width as usize * height as usize * 4;
                    q.order.push_back(request.id.clone());
                    q.cache.insert(request.id.clone(), texture);
                    while q.bytes > 24 * 1024 * 1024 {
                        let Some(old) = q.order.pop_front() else {
                            break;
                        };
                        if let Some(texture) = q.cache.remove(&old) {
                            q.bytes = q.bytes.saturating_sub(
                                texture.width() as usize * texture.height() as usize * 4,
                            );
                        }
                    }
                } else if let Some(picture) = request.picture.upgrade() {
                    picture.set_tooltip_text(Some(crate::i18n::text(crate::i18n::Key::NoPreview)));
                }
                queue.borrow_mut().active.remove(&request.id);
                Self::pump(&queue);
            });
            queue.borrow_mut().active.insert(id, task);
        }
    }
}
