using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Diagnostics;
using MemeDock.Bridge;
using MemeDock.Windows.Localization;
using MemeDock.Windows.Models;
using MemeDock.Windows.Services;
using MemeDock.Windows.ViewModels;
using Microsoft.UI.Input;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;
using Windows.System;
using Windows.ApplicationModel.DataTransfer;

namespace MemeDock.Windows.Views;

public sealed partial class StickerDetailView : UserControl
{
    private static readonly TimeSpan HeroLayoutWait = TimeSpan.FromMilliseconds(150);
    private StickerDetailViewModel? _model;
    private StickerOutputViewModel? _output;
    private LocalizationService? _strings;
    private ObservableCollection<CollectionMetadata>? _collections;
    private ObservableCollection<TagMetadata>? _tags;
    private bool _updating;
    private bool _play = true;
    private bool _active = true;
    private bool _fallback;
    private string? _previewId;
    private string? _previewPath;
    // The sticker whose thumbnail stands in until its detail and original arrive.
    private string? _placeholderId;
    private string _chipSignature = "";
    private BitmapImage? _image;
    private CancellationTokenSource? _previewCancellation;
    private ContentDialog? _previewDialog;
    public event EventHandler? CloseRequested;
    private sealed record CollectionChoice(string Name, CollectionMetadata? Collection);

    public StickerDetailView() => InitializeComponent();

    /// <summary>The square the tile's picture flies into, and back out of.</summary>
    public UIElement HeroTarget => PreviewFrame;

    public void Configure(StickerDetailViewModel model, LocalizationService strings,
        ObservableCollection<CollectionMetadata> collections, ObservableCollection<TagMetadata> tags, StickerOutputViewModel output)
    {
        if (_model is not null) _model.PropertyChanged -= OnModelChanged;
        if (_output is not null) _output.PropertyChanged -= OnModelChanged;
        _output = output;
        output.PropertyChanged += OnModelChanged;
        (_model, _strings, _collections, _tags) = (model, strings, collections, tags);
        model.PropertyChanged += OnModelChanged;
        _chipSignature = "";
        Relocalize();
    }

    public void Relocalize()
    {
        if (_strings is null) return;
        Heading.Text = _strings["desktop_details"];
        SetLabel(CloseButton, _strings["message_dismiss"]);
        SetLabel(MoreButton, _strings["more_actions"]);
        CopyLabel.Text = _strings["copy_image"];
        FormatButton.Content = _strings["export_choose"];
        AutomationProperties.SetName(FormatButton, _strings["export_choose"]);
        CopyOriginal.Text = _strings["export_original"];
        CopyPng.Text = _strings["export_png"];
        CopyFrame.Text = _strings.Format("share_first_frame_as", _strings["export_png"]);
        NameBox.PlaceholderText = _strings["sticker_title"];
        AutomationProperties.SetName(NameBox, _strings["sticker_title"]);
        NoteBox.Header = _strings["sticker_note"];
        NoteBox.PlaceholderText = _strings["add_note"];
        OrganizeLabel.Text = _strings["organize"];
        CollectionPicker.Header = _strings["tab_collections"];
        TagsLabel.Text = _strings["tags_title"];
        AddTagLabel.Text = _strings["add_tags"];
        AutomationProperties.SetName(TagChoices, _strings["select_tags"]);
        ApplyTags.Content = _strings["done"];
        NoTags.Text = _strings["batch_no_tags"];
        FileSection.Header = _strings["detail_file_title"];
        DeleteButton.Text = _strings["delete"];
        DimensionsLabel.Text = _strings["detail_size"];
        BytesLabel.Text = _strings["detail_bytes"];
        FormatLabel.Text = _strings["detail_format"];
        OriginalLabel.Text = _strings["detail_original_name"];
        PreviewRetry.Content = _strings["preview_retry"];
        _chipSignature = "";
        RefreshCatalogs();
    }

    private static void SetLabel(AppBarButton button, string label)
    {
        button.Label = label;
        AutomationProperties.SetName(button, label);
        ToolTipService.SetToolTip(button, label);
    }

    public void RefreshCatalogs()
    {
        if (_strings is null || _collections is null || _tags is null) return;
        _updating = true;
        try
        {
            CollectionPicker.ItemsSource = new[] { new CollectionChoice(_strings["no_collection"], null) }
                .Concat(_collections.Select(item => new CollectionChoice(item.Name, item))).ToArray();
            TagChoices.ItemsSource = _tags;
            NoTags.Visibility = MainWindow.ToVisibility(_tags.Count == 0);
            ApplyTags.Visibility = MainWindow.ToVisibility(_tags.Count > 0);
        }
        finally { _updating = false; }
        UpdateState();
    }

    /// <summary>
    /// Shows a tile's thumbnail in the preview square right away, so the pane has a
    /// picture (and a landing place) before the detail has been read.
    /// </summary>
    public void ShowPlaceholder(string id, ImageSource? thumbnail)
    {
        if (id == _previewId || id == _placeholderId) return;
        StopPreview();
        _placeholderId = id;
        Placeholder.Source = thumbnail;
        PreviewArea.Visibility = Visibility.Visible;
    }

    /// <summary>Waits for the preview square to be laid out in the opening pane.</summary>
    public async Task<bool> WaitForHeroTargetAsync()
    {
        var laidOut = new TaskCompletionSource<bool>(TaskCreationOptions.RunContinuationsAsynchronously);
        void OnLayout(object? sender, object args)
        {
            if (PreviewFrame.ActualWidth > 0 && PreviewArea.ActualHeight > 0) laidOut.TrySetResult(true);
        }
        PreviewFrame.LayoutUpdated += OnLayout;
        try
        {
            await Task.WhenAny(laidOut.Task, Task.Delay(HeroLayoutWait));
            return laidOut.Task.IsCompleted;
        }
        finally { PreviewFrame.LayoutUpdated -= OnLayout; }
    }

    private void OnModelChanged(object? sender, PropertyChangedEventArgs args) => UpdateState();

    private void UpdateState()
    {
        if (_model is null || _strings is null) return;
        _updating = true;
        try
        {
            var busy = _model.IsSaving || _model.IsLoading;
            DetailLoading.IsActive = _model.IsLoading && Placeholder.Source is null && Preview.Source is null;
            DetailLoading.Visibility = MainWindow.ToVisibility(DetailLoading.IsActive);
            Saving.IsActive = _model.IsSaving;
            Saving.Visibility = MainWindow.ToVisibility(_model.IsSaving);
            ErrorBar.IsOpen = _model.ErrorKey is not null;
            ErrorBar.Message = _model.ErrorText;
            Form.Visibility = MainWindow.ToVisibility(_model.Detail is not null);
            // A failed switch keeps the previous sticker; its picture comes back.
            if (!_model.IsLoading && _placeholderId is not null && _model.CurrentId != _placeholderId) _placeholderId = null;
            PreviewArea.Visibility = MainWindow.ToVisibility(_model.Detail is not null || _model.IsLoading || _placeholderId is not null);

            if (NameBox.Text != _model.Title) NameBox.Text = _model.Title;
            if (NoteBox.Text != _model.Note) NoteBox.Text = _model.Note;
            NameBox.IsEnabled = NoteBox.IsEnabled = !_model.IsLoading;
            StarButton.IsEnabled = CollectionPicker.IsEnabled = AddTagButton.IsEnabled = DeleteButton.IsEnabled = !busy;
            ApplyTags.IsEnabled = !busy;
            foreach (var chip in TagChips.Children.OfType<Button>()) chip.IsEnabled = !busy;

            if (_model.Detail is { } detail)
            {
                var copying = _output is { IsBusy: true, IsDragging: false };
                var canOutput = _output?.CanStart == true && !busy;
                CopyButton.IsEnabled = FormatButton.IsEnabled = canOutput;
                CopyBusy.IsActive = copying;
                CopyBusy.Visibility = MainWindow.ToVisibility(copying);
                CopyShortcut.Visibility = MainWindow.ToVisibility(!copying);
                DragSurface.CanDrag = _output is not null && !busy;
                CopyFrame.Visibility = MainWindow.ToVisibility(detail.Asset.Animated);
                CopyPng.Visibility = MainWindow.ToVisibility(!detail.Asset.Animated);
                ToolTipService.SetToolTip(CopyButton, _strings["copy_image"] + " (Ctrl+C)");
                AutomationProperties.SetName(CopyButton, _strings["copy_image"]);
                StarButton.IsChecked = detail.Sticker.Starred;
                StarGlyph.Glyph = detail.Sticker.Starred ? "\uE735" : "\uE734";
                SetLabel(StarButton, _strings[detail.Sticker.Starred ? "unfavorite" : "favorite"]);
                AutomationProperties.SetName(Preview, detail.Sticker.Title);
                OriginalValue.Text = detail.Sticker.OriginalName;
                DimensionsValue.Text = _strings.Format("detail_dimensions", detail.Asset.Width, detail.Asset.Height);
                BytesValue.Text = ByteSize.Format(detail.Asset.ByteSize);
                FormatValue.Text = detail.Asset.Mime.Split('/').Last().ToUpperInvariant();
                SyncCollection(detail);
                SyncTagChips(detail, busy);
                // While another sticker's thumbnail is standing in, the old detail is not restarted.
                var pending = _placeholderId is not null && _placeholderId != detail.Sticker.Id;
                if (!pending && (_previewId != detail.Sticker.Id || _previewPath != detail.OriginalPath)) StartPreview(detail);
            }
            else
            {
                if (!_model.IsLoading && _placeholderId is null) StopPreview();
                CopyButton.IsEnabled = FormatButton.IsEnabled = DragSurface.CanDrag = false;
            }
            UpdatePlayback();
        }
        finally { _updating = false; }
    }

    private static void SetLabel(AppBarToggleButton button, string label)
    {
        button.Label = label;
        AutomationProperties.SetName(button, label);
        ToolTipService.SetToolTip(button, label);
    }

    private void SyncCollection(StickerDetail detail)
    {
        var current = CollectionPicker.Items.OfType<CollectionChoice>()
            .FirstOrDefault(item => item.Collection?.Id == detail.Collection?.Id);
        if (!ReferenceEquals(CollectionPicker.SelectedItem, current)) CollectionPicker.SelectedItem = current;
    }

    /// <summary>
    /// Each tag is a chip that removes it; the chips are rebuilt only when the set or the
    /// language changes, so focus is not lost on every model notification.
    /// </summary>
    private void SyncTagChips(StickerDetail detail, bool busy)
    {
        var signature = detail.Sticker.Id + "|" + string.Join("|", detail.Tags.Select(tag => tag.Id + ":" + tag.Name));
        if (signature == _chipSignature) return;
        _chipSignature = signature;
        for (var index = TagChips.Children.Count - 1; index >= 0; index--)
            if (!ReferenceEquals(TagChips.Children[index], AddTagButton)) TagChips.Children.RemoveAt(index);
        var position = 0;
        foreach (var tag in detail.Tags)
        {
            var content = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6 };
            content.Children.Add(new TextBlock { Text = tag.Name, MaxWidth = 200, TextTrimming = TextTrimming.CharacterEllipsis });
            content.Children.Add(new FontIcon { Glyph = "\uE711", FontSize = 10 });
            var chip = new Button { Content = content, Tag = tag, IsEnabled = !busy, Style = (Style)Application.Current.Resources["TagChipButtonStyle"] };
            var label = _strings!["remove_tags"] + " · " + tag.Name;
            AutomationProperties.SetName(chip, label);
            ToolTipService.SetToolTip(chip, label);
            chip.Click += OnRemoveTag;
            TagChips.Children.Insert(position++, chip);
        }
    }

    private async void OnRemoveTag(object sender, RoutedEventArgs args)
    {
        if (_model?.Detail is not { } detail || sender is not Button { Tag: TagMetadata tag }) return;
        await _model.AssignTagsAsync(detail.Tags.Where(item => item.Id != tag.Id).ToArray());
    }

    private void OnTitleChanged(object sender, TextChangedEventArgs args) { if (!_updating && _model is not null) _model.Title = NameBox.Text; }
    private void OnNoteChanged(object sender, TextChangedEventArgs args) { if (!_updating && _model is not null) _model.Note = NoteBox.Text; }

    /// <summary>Leaving a field commits it, so a typed title is not lost to a pane that was never submitted.</summary>
    private async void OnEditorLostFocus(object sender, RoutedEventArgs args)
    {
        if (_updating || _model is null) return;
        await _model.CommitAsync();
    }

    private async void OnEditorKeyDown(object sender, KeyRoutedEventArgs args)
    {
        if (_model is null) return;
        if (args.Key == VirtualKey.Escape && _model.IsDirty)
        {
            _model.Discard();
            args.Handled = true;
        }
        else if (args.Key == VirtualKey.Enter && (ReferenceEquals(sender, NameBox)
            || (InputKeyboardSource.GetKeyStateForCurrentThread(VirtualKey.Control) & global::Windows.UI.Core.CoreVirtualKeyStates.Down) != 0))
        {
            args.Handled = true;
            await _model.CommitAsync();
        }
    }

    private async void OnStar(object sender, RoutedEventArgs args) { if (_model is not null) await _model.ToggleStarAsync(); }
    private async void OnCopy(object sender, RoutedEventArgs args) => await CopyAsync(StickerOutputKind.Image);
    private async void OnCopyOriginal(object sender, RoutedEventArgs args) => await CopyAsync(StickerOutputKind.OriginalFile);
    private async void OnCopyFrame(object sender, RoutedEventArgs args) => await CopyAsync(StickerOutputKind.FirstFrame);

    private async Task CopyAsync(StickerOutputKind kind)
    {
        if (_model?.Detail is not { } detail || _output?.CanStart != true) return;
        var target = new StickerOutputTarget(detail.Sticker.Id, detail.Sticker.Lifecycle.Generation);
        if (await _model.CommitAsync()) await _output.CopyAsync(target, kind);
    }

    private async void OnPreviewDragStarting(UIElement sender, DragStartingEventArgs args)
    {
        args.Cancel = true;
        if (_model?.Detail is not { } detail || _output?.CanStart != true) return;
        var target = new StickerOutputTarget(detail.Sticker.Id, detail.Sticker.Lifecycle.Generation);
        var deferral = args.GetDeferral();
        try
        {
            if (await _model.CommitAsync()) args.Cancel = !await _output.PrepareDragAsync(target, args.Data);
        }
        finally { deferral.Complete(); }
    }

    private async void OnPreviewDropCompleted(UIElement sender, DropCompletedEventArgs args)
    {
        if (_output is not null) await _output.CompleteDragAsync(args.DropResult == DataPackageOperation.Copy);
    }

    private async void OnDelete(object sender, RoutedEventArgs args) { if (_model is not null) await _model.DeleteAsync(); }
    private void OnClose(object sender, RoutedEventArgs args) => CloseRequested?.Invoke(this, EventArgs.Empty);

    private async void OnCollectionChosen(object sender, SelectionChangedEventArgs args)
    {
        if (_updating || _model?.Detail is not { } detail || CollectionPicker.SelectedItem is not CollectionChoice choice
            || choice.Collection?.Id == detail.Collection?.Id) return;
        // A rejected change is shown by the error bar; the picker returns to the stored value.
        if (!await _model.AssignCollectionAsync(choice.Collection) && _model.Detail is { } current)
        {
            _updating = true;
            try { SyncCollection(current); }
            finally { _updating = false; }
        }
    }

    private void OnTagOpening(object sender, object args)
    {
        TagChoices.SelectedItems.Clear();
        if (_model?.Detail is not { } detail || _tags is null) return;
        var ids = detail.Tags.Select(tag => tag.Id).ToHashSet();
        foreach (var tag in _tags.Where(tag => ids.Contains(tag.Id))) TagChoices.SelectedItems.Add(tag);
    }

    private async void OnApplyTags(object sender, RoutedEventArgs args)
    {
        if (_model is not null && await _model.AssignTagsAsync(TagChoices.SelectedItems.Cast<TagMetadata>().ToArray())) TagFlyout.Hide();
    }

    // ---- Preview ----

    private void StartPreview(StickerDetail detail)
    {
        StopPreview(keepPlaceholder: true);
        _placeholderId = null;
        _previewId = detail.Sticker.Id;
        _previewPath = detail.OriginalPath;
        _play = true;
        _fallback = detail.OriginalPath is null;
        _previewCancellation = new CancellationTokenSource();
        _ = LoadPreviewAsync(detail, _previewCancellation.Token, _fallback);
    }

    private async Task LoadPreviewAsync(StickerDetail detail, CancellationToken cancellation, bool thumbnail)
    {
        try
        {
            var path = thumbnail ? await _model!.ThumbnailAsync(detail.Sticker.Id, cancellation) : detail.OriginalPath!;
            if (cancellation.IsCancellationRequested || detail.Sticker.Id != _previewId) return;
            var normalized = path.StartsWith(@"\\?\UNC\", StringComparison.OrdinalIgnoreCase) ? @"\\" + path[8..]
                : path.StartsWith(@"\\?\", StringComparison.Ordinal) ? path[4..] : path;
            _image = new BitmapImage { DecodePixelWidth = 1024, DecodePixelType = DecodePixelType.Physical, AutoPlay = false };
            var image = _image;
            image.ImageOpened += (_, _) =>
            {
                if (cancellation.IsCancellationRequested || _image != image) return;
                // The original now covers the square, so the stand-in can go.
                Placeholder.Source = null;
                PreviewRetry.Visibility = Visibility.Collapsed;
                PreviewHint.Text = detail.Asset.Animated && (!image.IsAnimatedBitmap || thumbnail) ? _strings!["apng_first_frame"] : "";
                if (_active && _play && image.IsAnimatedBitmap) image.Play();
                UpdatePlayback();
                UpdateState();
            };
            image.ImageFailed += async (_, _) =>
            {
                if (cancellation.IsCancellationRequested || _image != image) return;
                if (!thumbnail)
                {
                    _fallback = true;
                    await LoadPreviewAsync(detail, cancellation, thumbnail: true);
                }
                else
                {
                    Preview.Source = null;
                    PreviewRetry.Visibility = Visibility.Visible;
                    PreviewHint.Text = _strings!["preview_unavailable"];
                }
            };
            // The image sits above the stand-in and stays transparent until it has decoded.
            Preview.Source = image;
            image.UriSource = new Uri(normalized);
        }
        catch (Exception error)
        {
            if (cancellation.IsCancellationRequested) return;
            Trace.WriteLine(error);
            PreviewRetry.Visibility = Visibility.Visible;
            PreviewHint.Text = _strings!["preview_unavailable"];
        }
    }

    private void OnPlayback(object sender, RoutedEventArgs args)
    {
        _play = !_play;
        if (_play && _active) _image?.Play(); else _image?.Stop();
        UpdatePlayback();
    }

    private void UpdatePlayback()
    {
        Playback.Visibility = MainWindow.ToVisibility(_image?.IsAnimatedBitmap == true && !_fallback);
        PlaybackGlyph.Glyph = _play ? "\uE769" : "\uE768";
        if (_strings is not null)
        {
            var label = _strings[_play ? "pause_animation" : "play_animation"];
            PlaybackLabel.Text = label;
            AutomationProperties.SetName(Playback, label);
        }
        if (_model?.Detail?.Asset.Animated == true && (_fallback || _image?.IsAnimatedBitmap == false) && _strings is not null)
            PreviewHint.Text = _strings["apng_first_frame"];
    }

    public void SetActive(bool active)
    {
        _active = active;
        if (active && _play && _image?.IsAnimatedBitmap == true) _image.Play(); else _image?.Stop();
    }

    private void StopPreview(bool keepPlaceholder = false)
    {
        _previewCancellation?.Cancel();
        _previewCancellation?.Dispose();
        _previewCancellation = null;
        _image?.Stop();
        _image = null;
        Preview.Source = null;
        if (!keepPlaceholder)
        {
            Placeholder.Source = null;
            _placeholderId = null;
        }
        _previewId = _previewPath = null;
        PreviewHint.Text = "";
        PreviewRetry.Visibility = Visibility.Collapsed;
    }

    private void OnPreviewRetry(object sender, RoutedEventArgs args) { if (_model?.Detail is { } detail) StartPreview(detail); }
    private async void OnPreviewDoubleTapped(object sender, DoubleTappedRoutedEventArgs args) => await ShowPreviewAsync();

    public async Task ShowPreviewAsync()
    {
        if (_image is null || _previewDialog is not null || _strings is null || _model?.Detail is null) return;
        var image = new Image { Source = _image, Stretch = Stretch.Uniform };
        _previewDialog = new ContentDialog
        {
            XamlRoot = XamlRoot, Title = _model.Detail.Sticker.Title, CloseButtonText = _strings["message_dismiss"],
            Content = new ScrollViewer { Content = image, ZoomMode = ZoomMode.Enabled, MinZoomFactor = 0.25f, MaxZoomFactor = 4,
                Height = Math.Max(160, Math.Min(600, XamlRoot.Size.Height - 240)), Width = Math.Max(200, Math.Min(840, XamlRoot.Size.Width - 120)) }
        };
        _previewDialog.Resources["ContentDialogMaxWidth"] = 960d;
        try { await _previewDialog.ShowAsync(); }
        finally { _previewDialog = null; }
    }

    private void OnUnloaded(object sender, RoutedEventArgs args) => SetActive(false);

    public void Shutdown()
    {
        _previewDialog?.Hide();
        StopPreview();
        if (_model is not null) _model.PropertyChanged -= OnModelChanged;
        if (_output is not null) _output.PropertyChanged -= OnModelChanged;
    }
}
