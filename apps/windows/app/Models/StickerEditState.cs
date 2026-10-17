using MemeDock.Bridge;

namespace MemeDock.Windows.Models;

public sealed record PendingStickerEdit(EntityReference Target, StickerEdit Edit);

// Draft ownership is independent of which item the view currently displays.
public sealed class StickerEditState(StickerMetadata metadata)
{
    public StickerMetadata Metadata { get; private set; } = metadata;
    public string Title { get; set; } = metadata.Title;
    public string Note { get; set; } = metadata.Note;
    public bool IsDirty => Title != Metadata.Title || Note != Metadata.Note;
    public EntityReference Target => new(Metadata.Id, Metadata.Lifecycle.Generation);

    public PendingStickerEdit Capture() => new(Target,
        new StickerEdit(Title == Metadata.Title ? null : Title, Note == Metadata.Note ? null : Note, null));

    public bool Accept(StickerMetadata saved, PendingStickerEdit sent)
    {
        if (saved.Id != Metadata.Id || sent.Target.Id != Metadata.Id
            || saved.Lifecycle.Generation != Metadata.Lifecycle.Generation
            || sent.Target.Generation != Metadata.Lifecycle.Generation
            || saved.Lifecycle.Revision < Metadata.Lifecycle.Revision) return false;
        var keepTitle = sent.Edit.Title is null ? Title != Metadata.Title : Title != sent.Edit.Title;
        var keepNote = sent.Edit.Note is null ? Note != Metadata.Note : Note != sent.Edit.Note;
        Metadata = saved;
        if (!keepTitle) Title = saved.Title;
        if (!keepNote) Note = saved.Note;
        return true;
    }

    public void Discard()
    {
        Title = Metadata.Title;
        Note = Metadata.Note;
    }
}
