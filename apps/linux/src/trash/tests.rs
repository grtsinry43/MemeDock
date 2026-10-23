use super::*;
use crate::sync::tests::{import, named, open, wait_until};
use std::error::Error;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

fn button(parent: &impl IsA<gtk4::Widget>, label: &str) -> Option<gtk4::Button> {
    let widget = parent.upcast_ref::<gtk4::Widget>();
    if let Some(button) = widget.downcast_ref::<gtk4::Button>()
        && button.label().as_deref() == Some(label)
    {
        return Some(button.clone());
    }
    let mut child = widget.first_child();
    while let Some(current) = child {
        if let Some(button) = button(&current, label) {
            return Some(button);
        }
        child = current.next_sibling();
    }
    None
}
pub(crate) fn accept(coordinator: &Coordinator) -> TestResult {
    let dialog = coordinator
        .confirmation
        .borrow()
        .upgrade()
        .ok_or("confirmation missing")?;
    button(&dialog, i18n::text(Key::Purge))
        .ok_or("purge button missing")?
        .emit_clicked();
    Ok(())
}

pub(crate) async fn verify(parent: &impl IsA<gtk4::Widget>) -> TestResult {
    let root = tempfile::tempdir()?;
    let library = open(root.path()).await?;
    let coordinator = Coordinator::new(library.clone());
    let messages = Rc::new(RefCell::new(Vec::<String>::new()));
    let output = Rc::clone(&messages);
    let notify: Notify = Rc::new(move |message| output.borrow_mut().push(message.into()));
    let result = async {
        let image = import(&library, "trash original", 43).await?;
        library
            .delete_sticker(image.id(), image.lifecycle().generation())?
            .wait()
            .await?;
        let collection = library
            .create_collection("trash collection".parse()?, None)?
            .wait()
            .await?;
        let collection = library
            .delete_collection(collection.id(), collection.lifecycle().generation())?
            .wait()
            .await?;
        let tag = library.create_tag("trash tag".parse()?)?.wait().await?;
        library
            .delete_tag(tag.id(), tag.lifecycle().generation())?
            .wait()
            .await?;
        coordinator.empty(parent, Rc::clone(&notify));
        wait_until(|| coordinator.confirmation.borrow().upgrade().is_some()).await?;
        // These changes happen after the confirmation has fixed the three targets.
        let late = import(&library, "trash later", 67).await?;
        library
            .delete_sticker(late.id(), late.lifecycle().generation())?
            .wait()
            .await?;
        let restored = library
            .restore_collection(
                collection.id(),
                collection.lifecycle().generation(),
                collection.lifecycle().revision(),
            )?
            .wait()
            .await?;
        accept(&coordinator)?;
        wait_until(|| !coordinator.busy()).await?;
        assert!(
            library
                .sticker_detail(image.id())?
                .wait()
                .await?
                .sticker
                .lifecycle()
                .is_purged()
        );
        let snapshot = library.trash_snapshot()?.wait().await?;
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].entity, EntityId::Sticker(late.id()));
        assert!(
            library
                .collections(false)?
                .wait()
                .await?
                .iter()
                .any(|v| v.id() == restored.id())
        );
        assert!(
            messages
                .borrow()
                .iter()
                .any(|message| message.contains(&i18n::purge_result(2, 1)))
        );

        library
            .delete_collection(restored.id(), restored.lifecycle().generation())?
            .wait()
            .await?;
        let new_tag = library.create_tag("group tag".parse()?)?.wait().await?;
        library
            .delete_tag(new_tag.id(), new_tag.lifecycle().generation())?
            .wait()
            .await?;
        coordinator.groups(parent, Rc::clone(&notify));
        wait_until(|| {
            coordinator
                .groups
                .borrow()
                .upgrade()
                .is_some_and(|dialog| named(&dialog, "trash-group-restore").is_some())
        })
        .await?;
        let dialog = coordinator
            .groups
            .borrow()
            .upgrade()
            .ok_or("groups dialog missing")?;
        named(&dialog, "trash-group-restore")
            .ok_or("restore button missing")?
            .downcast::<gtk4::Button>()
            .map_err(|_| "restore widget")?
            .emit_clicked();
        wait_until(|| !coordinator.busy()).await?;
        assert!(
            library
                .collections(false)?
                .wait()
                .await?
                .iter()
                .any(|v| v.id() == restored.id())
        );
        wait_until(|| named(&dialog, "trash-group-purge").is_some()).await?;
        // Wait for the row refresh that removed the restored collection.
        glib::timeout_future(std::time::Duration::from_millis(30)).await;
        named(&dialog, "trash-group-purge")
            .ok_or("group purge button missing")?
            .downcast::<gtk4::Button>()
            .map_err(|_| "purge widget")?
            .emit_clicked();
        accept(&coordinator)?;
        wait_until(|| !coordinator.busy()).await?;
        assert!(
            !library
                .tags(true)?
                .wait()
                .await?
                .iter()
                .any(|v| v.id() == new_tag.id())
        );

        let captured = library.trash_snapshot()?.wait().await?;
        coordinator.request(parent, captured, Rc::clone(&notify), Rc::new(|_| {}));
        coordinator.suspend().await;
        assert!(
            coordinator
                .confirmation
                .borrow()
                .upgrade()
                .is_none_or(|dialog| !dialog.is_visible())
        );
        assert!(coordinator.current.borrow().is_none());
        assert!(
            library
                .trash_snapshot()?
                .wait()
                .await?
                .iter()
                .any(|v| v.entity == EntityId::Sticker(late.id()))
        );
        coordinator.resume();
        coordinator.cleanup(Rc::clone(&notify));
        wait_until(|| !coordinator.busy()).await?;
        assert!(
            messages
                .borrow()
                .iter()
                .any(|message| message == i18n::text(Key::PurgeCleanupDone))
        );
        Ok::<_, Box<dyn Error>>(())
    }
    .await;
    coordinator.shutdown().await;
    library.close().await?;
    result
}
