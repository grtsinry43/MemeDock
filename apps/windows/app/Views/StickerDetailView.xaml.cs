using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Diagnostics;
using MemeDock.Bridge;
using MemeDock.Windows.Localization;
using MemeDock.Windows.Models;
using MemeDock.Windows.ViewModels;
using Microsoft.UI.Input;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media.Imaging;
using Windows.System;
using Windows.ApplicationModel.DataTransfer;

namespace MemeDock.Windows.Views;

public sealed partial class StickerDetailView : UserControl
{
    private StickerDetailViewModel? _model;
    private StickerOutputViewModel? _output;
    private LocalizationService? _strings;
    private ObservableCollection<CollectionMetadata>? _collections;
    private ObservableCollection<TagMetadata>? _tags;
    private bool _updating;
    private bool _editingTitle;
    private bool _editingNote;
    private string? _displayedId;
    private bool _play = true;
    private bool _active = true;
    private bool _fallback;
    private string? _previewId;
    private string? _previewPath;
    private BitmapImage? _image;
    private CancellationTokenSource? _previewCancellation;
    private ContentDialog? _previewDialog;
    public event EventHandler? CloseRequested;
    private sealed record CollectionChoice(string Name, CollectionMetadata? Collection);

    public StickerDetailView() => InitializeComponent();

    public void Configure(StickerDetailViewModel model, LocalizationService strings,
        ObservableCollection<CollectionMetadata> collections, ObservableCollection<TagMetadata> tags, StickerOutputViewModel output)
    {
        if (_model is not null) _model.PropertyChanged -= OnModelChanged;
        if (_output is not null) _output.PropertyChanged -= OnModelChanged;
        _output = output;
        output.PropertyChanged += OnModelChanged;
        (_model, _strings, _collections, _tags) = (model, strings, collections, tags);
        model.PropertyChanged += OnModelChanged;
        Relocalize();
        RefreshCatalogs();
        UpdateState();
    }

    public void Relocalize()
    {
        if (_strings is null) return;
        Heading.Text = _strings["desktop_details"];
        AutomationProperties.SetName(CloseButton, _strings["message_dismiss"]);
        ToolTipService.SetToolTip(CloseButton, _strings["message_dismiss"]);
        CloseButton.Label = _strings["message_dismiss"];
        MoreButton.Label = _strings["more_actions"];
        CopyButton.Label = _strings["copy_image"];
        AutomationProperties.SetName(CopyButton, CopyButton.Label);
        CopyFormats.Text = _strings["export_choose"];
        CopyOriginal.Text = _strings["export_original"];
        CopyPng.Text = _strings["export_png"];
        CopyFrame.Text = _strings.Format("share_first_frame_as", _strings["export_png"]);
        ToolTipService.SetToolTip(MoreButton, _strings["more_actions"]);
        AutomationProperties.SetName(MoreButton, _strings["more_actions"]);
        NameBox.Header = _strings["sticker_title"];
        NoteBox.Header = _strings["sticker_note"];
        NoteBox.PlaceholderText = _strings["add_note"];
        SaveButton.Content = _strings["done"];
        DiscardButton.Content = _strings["cancel"];
        CollectionLabel.Text = _strings["tab_collections"];
        TagsLabel.Text = _strings["tags_title"];
        AutomationProperties.SetName(CollectionPicker, _strings["select_collections"]);
        AutomationProperties.SetName(TagChoices, _strings["select_tags"]);
        ToolTipService.SetToolTip(CollectionButton, _strings["select_collections"]);
        ToolTipService.SetToolTip(TagButton, _strings["select_tags"]);
        ApplyTags.Content = _strings["done"];
        NoTags.Text = _strings["batch_no_tags"];
        FileSection.Header = _strings["detail_file_title"];
        DeleteButton.Text = _strings["delete"];
        DimensionsLabel.Text = _strings["detail_size"];
        BytesLabel.Text = _strings["detail_bytes"];
        FormatLabel.Text = _strings["detail_format"];
        OriginalLabel.Text = _strings["detail_original_name"];
        PreviewRetry.Content = _strings["preview_retry"];
        RefreshCatalogs();
        UpdateState();
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
            SyncCollection();
        }
        finally { _updating = false; }
    }

    private void OnModelChanged(object? sender, PropertyChangedEventArgs args) => UpdateState();
    private void UpdateState()
    {
        if (_model is null || _strings is null) return;
        _updating = true;
        try
        {
            DetailLoading.IsActive = _model.IsLoading;
            DetailLoading.Visibility = MainWindow.ToVisibility(_model.IsLoading);
            Form.Visibility = MainWindow.ToVisibility(_model.Detail is not null);
            ErrorBar.IsOpen = _model.ErrorKey is not null;
            ErrorBar.Message = _model.ErrorText;
            if (_displayedId != _model.CurrentId)
            {
                _displayedId = _model.CurrentId;
                _editingTitle = _editingNote = false;
            }
            if (NameBox.Text != _model.Title) NameBox.Text = _model.Title;
            if (NoteBox.Text != _model.Note) NoteBox.Text = _model.Note;
            TitleText.Text = _model.Title;
            NoteText.Text = _model.Note.Length == 0 ? _strings["add_note"] : _model.Note;
            TitleDisplay.Visibility = MainWindow.ToVisibility(!_editingTitle);
            NameBox.Visibility = MainWindow.ToVisibility(_editingTitle);
            NoteDisplay.Visibility = MainWindow.ToVisibility(!_editingNote);
            NoteBox.Visibility = MainWindow.ToVisibility(_editingNote);
            TitleDisplay.IsEnabled = NoteDisplay.IsEnabled = !_model.IsLoading && !_model.IsSaving;
            AutomationProperties.SetName(TitleDisplay, _strings["sticker_title"] + ": " + _model.Title);
            AutomationProperties.SetName(NoteDisplay, _strings["sticker_note"] + ": " + NoteText.Text);
            NameBox.IsEnabled = NoteBox.IsEnabled = !_model.IsLoading;
            DraftActions.Visibility = MainWindow.ToVisibility(_model.IsDirty || _model.IsSaving);
            Saving.IsActive = _model.IsSaving;
            SaveButton.IsEnabled = DiscardButton.IsEnabled = !_model.IsSaving;
            StarButton.IsEnabled = CollectionButton.IsEnabled = TagButton.IsEnabled = DeleteButton.IsEnabled = !_model.IsSaving && !_model.IsLoading;
            CollectionPicker.IsEnabled = ApplyTags.IsEnabled = !_model.IsSaving && !_model.IsLoading;
            if (_model.Detail is { } detail)
            {
                var canOutput = _output?.CanStart == true && !_model.IsSaving && !_model.IsLoading;
                CopyButton.IsEnabled = CopyFormats.IsEnabled = canOutput;
                DragSurface.CanDrag = _output is not null && !_model.IsSaving && !_model.IsLoading;
                CopyFrame.Visibility = MainWindow.ToVisibility(detail.Asset.Animated);
                CopyPng.Visibility = MainWindow.ToVisibility(!detail.Asset.Animated);
                ToolTipService.SetToolTip(CopyButton, _strings["copy_image"] + " · " + _strings[detail.Asset.Animated ? "export_original" : "export_png"]);
                StarButton.IsChecked = detail.Sticker.Starred;
                StarGlyph.Glyph = detail.Sticker.Starred ? "\uE735" : "\uE734";
                StarButton.Label = _strings[detail.Sticker.Starred ? "unfavorite" : "favorite"];
                ToolTipService.SetToolTip(StarButton, StarButton.Label);
                AutomationProperties.SetName(StarButton, StarButton.Label);
                AutomationProperties.SetName(Preview, detail.Sticker.Title);
                CollectionName.Text = detail.Collection?.Name ?? _strings["no_collection"];
                TagSummary.Text = detail.Tags.Length == 0 ? _strings["batch_no_tags"] : string.Join(" · ", detail.Tags.Select(tag => tag.Name));
                AutomationProperties.SetName(CollectionButton, CollectionLabel.Text + ": " + CollectionName.Text);
                AutomationProperties.SetName(TagButton, TagsLabel.Text + ": " + TagSummary.Text);
                OriginalValue.Text = detail.Sticker.OriginalName;
                DimensionsValue.Text = _strings.Format("detail_dimensions", detail.Asset.Width, detail.Asset.Height);
                BytesValue.Text = detail.Asset.ByteSize.ToString("N0", _strings.Culture) + " B";
                FormatValue.Text = detail.Asset.Mime.Split('/').Last().ToUpperInvariant();
                SyncCollection();
                if (_previewId != detail.Sticker.Id || _previewPath != detail.OriginalPath) StartPreview(detail);
            }
            else { StopPreview(); CopyButton.IsEnabled = CopyFormats.IsEnabled = DragSurface.CanDrag = false; }
            UpdatePlayback();
        }
        finally { _updating = false; }
    }

    private void SyncCollection()
    {
        if (_model is null) return;
        CollectionPicker.SelectedItem = CollectionPicker.Items.OfType<CollectionChoice>()
            .FirstOrDefault(item => item.Collection?.Id == _model.Detail?.Collection?.Id);
    }
    private void OnTitleChanged(object sender, TextChangedEventArgs args) { if (!_updating && _model is not null) _model.Title = NameBox.Text; }
    private void OnNoteChanged(object sender, TextChangedEventArgs args) { if (!_updating && _model is not null) _model.Note = NoteBox.Text; }
    private async void OnEditorLostFocus(object sender, RoutedEventArgs args)
    {
        await Task.Yield();
        if (_updating || _model is null || ReferenceEquals(FocusManager.GetFocusedElement(XamlRoot), DiscardButton)) return;
        if (await _model.CommitAsync() && !ReferenceEquals(FocusManager.GetFocusedElement(XamlRoot), NameBox)
            && !ReferenceEquals(FocusManager.GetFocusedElement(XamlRoot), NoteBox)) EndEditing(restoreFocus: false);
    }
    private async void OnEditorKeyDown(object sender, KeyRoutedEventArgs args)
    {
        if (_model is null) return;
        if (args.Key == VirtualKey.Escape) { _model.Discard(); EndEditing(); args.Handled = true; }
        else if (args.Key == VirtualKey.Enter && (ReferenceEquals(sender, NameBox)
            || (InputKeyboardSource.GetKeyStateForCurrentThread(VirtualKey.Control) & global::Windows.UI.Core.CoreVirtualKeyStates.Down) != 0))
        {
            args.Handled = true;
            if (await _model.CommitAsync()) EndEditing();
        }
    }
    private async void OnSave(object sender, RoutedEventArgs args) { if (_model is not null && await _model.CommitAsync()) EndEditing(); }
    private void OnDiscard(object sender, RoutedEventArgs args) { _model?.Discard(); EndEditing(); }
    private async void OnEditTitle(object sender, RoutedEventArgs args) => await BeginEditingAsync(title: true);
    private async void OnEditNote(object sender, RoutedEventArgs args) => await BeginEditingAsync(title: false);
    private async Task BeginEditingAsync(bool title)
    {
        if (_model?.Detail is null || _model.IsLoading || _model.IsSaving) return;
        var target = _model.CurrentId;
        if (!await _model.CommitAsync() || _model.CurrentId != target || _model.IsLoading) return;
        _editingTitle = title;
        _editingNote = !title;
        UpdateState();
        if (title) { NameBox.Focus(FocusState.Programmatic); NameBox.SelectAll(); }
        else { NoteBox.Focus(FocusState.Programmatic); NoteBox.SelectionStart = NoteBox.Text.Length; }
    }
    private void EndEditing(bool restoreFocus = true)
    {
        var title = _editingTitle;
        var hadEditor = _editingTitle || _editingNote;
        _editingTitle = _editingNote = false;
        UpdateState();
        // Keep keyboard focus visible after the editor itself is collapsed.
        if (restoreFocus && hadEditor) (title ? TitleDisplay : NoteDisplay).Focus(FocusState.Programmatic);
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
    private void OnCollectionOpening(object sender, object args) => SyncCollection();
    private async void OnCollectionChosen(object sender, ItemClickEventArgs args)
    {
        if (!_updating && _model is not null && args.ClickedItem is CollectionChoice choice
            && await _model.AssignCollectionAsync(choice.Collection)) CollectionFlyout.Hide();
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

    private void StartPreview(StickerDetail detail)
    {
        StopPreview();
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
                PreviewRetry.Visibility = Visibility.Collapsed;
                PreviewHint.Text = detail.Asset.Animated && (!image.IsAnimatedBitmap || thumbnail) ? _strings!["apng_first_frame"] : "";
                if (_active && _play && image.IsAnimatedBitmap) image.Play();
                UpdatePlayback();
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
                    PreviewRetry.Visibility = Visibility.Visible;
                    PreviewHint.Text = _strings!["preview_unavailable"];
                }
            };
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
            AutomationProperties.SetName(Playback, _strings[_play ? "pause_animation" : "play_animation"]);
            ToolTipService.SetToolTip(Playback, _strings[_play ? "pause_animation" : "play_animation"]);
        }
        if (_model?.Detail?.Asset.Animated == true && (_fallback || _image?.IsAnimatedBitmap == false) && _strings is not null)
            PreviewHint.Text = _strings["apng_first_frame"];
    }
    public void SetActive(bool active)
    {
        _active = active;
        if (active && _play && _image?.IsAnimatedBitmap == true) _image.Play(); else _image?.Stop();
    }
    private void StopPreview()
    {
        _previewCancellation?.Cancel();
        _previewCancellation?.Dispose();
        _previewCancellation = null;
        _image?.Stop();
        _image = null;
        Preview.Source = null;
        _previewId = _previewPath = null;
        PreviewHint.Text = "";
        PreviewRetry.Visibility = Visibility.Collapsed;
    }
    private void OnPreviewRetry(object sender, RoutedEventArgs args) { if (_model?.Detail is { } detail) StartPreview(detail); }
    private async void OnPreviewDoubleTapped(object sender, DoubleTappedRoutedEventArgs args) => await ShowPreviewAsync();
    public async Task ShowPreviewAsync()
    {
        if (_image is null || _previewDialog is not null || _strings is null || _model?.Detail is null) return;
        var image = new Image { Source = _image, Stretch = Microsoft.UI.Xaml.Media.Stretch.Uniform };
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
