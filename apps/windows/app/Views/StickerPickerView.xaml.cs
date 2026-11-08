using System.ComponentModel;
using System.Diagnostics;
using MemeDock.Windows.Models;
using MemeDock.Windows.Services;
using MemeDock.Windows.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Windows.ApplicationModel.DataTransfer;
using Windows.System;

namespace MemeDock.Windows.Views;

public sealed partial class StickerPickerView : UserControl
{
    private MainWindow? _main;
    private QuickPickerViewModel? _model;
    private bool _updating, _composing, _menu, _working, _dragging, _compact, _modalOperation;
    private Task<bool>? _dragPreparation;
    private long _presentation;
    private TextBox? _searchEditor;
    internal Func<StickerCard, bool, StickerOutputKind, Task>? Output { get; set; }
    internal Func<StickerCard, Task>? Save { get; set; }
    internal Action<string?>? Open { get; set; }
    internal Action<bool>? Dismiss { get; set; }
    internal Action? InteractionEnded { get; set; }
    internal event Action? ContentSizeChanged;
    internal bool Interacting => _composing || _menu || _working || _dragging;
    internal bool MenuOpen => _menu;
    internal bool Dragging => _dragging;
    internal bool ModalOperation => _modalOperation;
    public StickerPickerView() => InitializeComponent();
    internal void Configure(MainWindow main, QuickPickerViewModel? model, bool compact)
    {
        if (_model is not null) _model.PropertyChanged -= OnChanged;
        (_main, _model, _compact) = (main, model, compact);
        Gallery.ItemsSource = model?.Cards;
        if (model is not null) model.PropertyChanged += OnChanged;
        Relocalize();
    }
    internal void Relocalize()
    {
        if (_main is null) return;
        _updating = true;
        RecentFilter.Text = _main.Strings["picker_recent"];
        StarredFilter.Text = _main.Strings["picker_starred"];
        AllFilter.Text = _main.Strings["picker_all"];
        CollectionMenu.Items.Clear();
        foreach (var collection in _main.ViewModel.Collections)
        {
            var item = new MenuFlyoutItem { Text = collection.Name };
            item.Click += async (_, _) =>
            {
                if (_model is null) return;
                _updating = true; Filters.SelectedItem = null; _updating = false;
                Collections.Content = collection.Name;
                await _model.SelectAsync(new(LibraryScope.Collection, collection.Id, collection.Name));
            };
            CollectionMenu.Items.Add(item);
        }
        Collections.Content = _model?.Filter.Scope == LibraryScope.Collection ? _model.Filter.Name : _main.Strings["picker_collections"];
        Collections.IsEnabled = CollectionMenu.Items.Count > 0;
        Search.PlaceholderText = _main.Strings["picker_search"];
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(Search, Search.PlaceholderText);
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(Filters, _main.Strings["picker_filters"]);
        ToolTipService.SetToolTip(OpenLibrary, _main.Strings["picker_open_library"]);
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(OpenLibrary, _main.Strings["picker_open_library"]);
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(Collections, _main.Strings["picker_collections"]);
        PasteLabel.Text = _main.Strings["picker_paste"];
        Retry.Content = _main.Strings["retry"];
        SearchHeader.Visibility = FilterBar.Visibility = Footer.Visibility = _compact ? Visibility.Collapsed : Visibility.Visible;
        LayoutRoot.Padding = _compact ? new Thickness(0) : new Thickness(12);
        LayoutRoot.RowSpacing = _compact ? 0 : 8;
        ScrollViewer.SetVerticalScrollMode(Gallery, _compact ? ScrollMode.Disabled : ScrollMode.Enabled);
        ScrollViewer.SetVerticalScrollBarVisibility(Gallery, _compact ? ScrollBarVisibility.Disabled : ScrollBarVisibility.Auto);
        _updating = false;
        Update();
    }
    internal async Task PresentAsync()
    {
        var presentation = ++_presentation;
        Feedback.IsOpen = false;
        _updating = true; Search.Text = ""; Filters.SelectedItem = RecentFilter;
        if (_main is not null) Collections.Content = _main.Strings["picker_collections"];
        _updating = false;
        if (_model is not null) await _model.PresentAsync();
        if (presentation == _presentation && _model?.Visible == true)
        {
            if (Gallery.Items.Count > 0) Gallery.SelectedIndex = 0;
            FocusInput();
        }
    }
    internal void FocusInput()
    {
        if (_compact) Gallery.Focus(FocusState.Programmatic);
        else (_searchEditor as Control ?? Search).Focus(FocusState.Programmatic);
    }
    internal void Suspend()
    {
        ++_presentation; _model?.Suspend(); Feedback.IsOpen = false;
        _composing = false; _menu = false; CollectionMenu.Hide();
    }
    internal void ShowMessage(string key) { if (_main is not null) ShowFeedback(_main.Strings[key], key != "picker_saved"); }
    internal void ShowFeedback(string text, bool failure)
    { Feedback.Message = text; Feedback.Severity = failure ? InfoBarSeverity.Warning : InfoBarSeverity.Success; Feedback.IsOpen = true; }
    private void OnChanged(object? sender, PropertyChangedEventArgs args) => Update();
    internal void RefreshState() => Update();
    private void Update()
    {
        if (_main is null) return;
        Gallery.IsEnabled = Search.IsEnabled = Filters.IsEnabled = Paste.IsEnabled = _main.ViewModel.IsReady && !_main.ViewModel.IsRestoring;
        if (_model is null)
        {
            var failed = _main.ViewModel.ErrorKey is not null;
            Busy.IsActive = !failed;
            Busy.Visibility = failed ? Visibility.Collapsed : Visibility.Visible;
            Empty.Text = failed ? _main.ViewModel.ErrorText : _main.Strings["picker_loading_library"];
            EmptyState.Visibility = Visibility.Visible;
            Retry.Visibility = failed ? Visibility.Visible : Visibility.Collapsed;
            UpdateCompactHeight();
            return;
        }
        Busy.IsActive = _model.Loading;
        Busy.Visibility = _model.Loading ? Visibility.Visible : Visibility.Collapsed;
        Empty.Text = _model.Error ?? _main.Strings[_model.SearchText.Length > 0 ? "picker_no_results" : "picker_empty"];
        EmptyState.Visibility = !_model.Loading && (_model.Cards.Count == 0 || _model.Error is not null) ? Visibility.Visible : Visibility.Collapsed;
        Retry.Visibility = _model.Error is not null ? Visibility.Visible : Visibility.Collapsed;
        if (_model.Error is not null) ShowFeedback(_model.Error, true);
        Retry.IsEnabled = !_model.Loading;
        FooterProgress.IsActive = _model.Loading || _working || _dragging;
        FooterProgress.Visibility = FooterProgress.IsActive ? Visibility.Visible : Visibility.Collapsed;
        Activity.Text = FooterProgress.IsActive ? _main.Strings[_working || _dragging ? "share_preparing" : "picker_loading"] : "";
        if (Gallery.SelectedItem is null && Gallery.Items.Count > 0) Gallery.SelectedIndex = 0;
        Paste.IsEnabled = !_working && !_dragging && Gallery.SelectedItem is StickerCard && _model.Visible;
        Paste.Visibility = Gallery.SelectedItem is StickerCard ? Visibility.Visible : Visibility.Collapsed;
        UpdateCompactHeight();
    }
    private void UpdateCompactHeight()
    {
        if (_compact)
        {
            GalleryRegion.Height = Math.Max(100, Math.Ceiling(Math.Min(8, _model?.Cards.Count ?? 0) / 4d) * 84);
            ContentSizeChanged?.Invoke();
        }
        else GalleryRegion.Height = double.NaN;
    }
    private void OnGallerySelectionChanged(object sender, SelectionChangedEventArgs args)
    {
        Paste.IsEnabled = !_working && !_dragging && Gallery.SelectedItem is StickerCard;
        Paste.Visibility = Gallery.SelectedItem is StickerCard ? Visibility.Visible : Visibility.Collapsed;
    }
    private void OnGallerySizeChanged(object sender, SizeChangedEventArgs args)
    {
        if (Gallery.ItemsPanelRoot is ItemsWrapGrid panel && args.NewSize.Width > 0)
        { panel.ItemWidth = Math.Floor(args.NewSize.Width / (_compact ? 4 : 5)); panel.ItemHeight = 84; }
    }
    private async void OnSearch(AutoSuggestBox sender, AutoSuggestBoxTextChangedEventArgs args)
    { if (!_updating && !_composing && _model is not null) await _model.SearchAsync(Search.Text); }
    private async void OnFilter(SelectorBar sender, SelectorBarSelectionChangedEventArgs args)
    {
        if (_updating || _model is null || sender.SelectedItem is not { Tag: string tag }) return;
        var filter = tag switch { "recent" => new LibraryFilter(LibraryScope.Recent), "starred" => new(LibraryScope.Starred), _ => LibraryFilter.All };
        if (_main is not null) Collections.Content = _main.Strings["picker_collections"];
        await _model.SelectAsync(filter);
    }
    private void OnCollectionMenuOpened(object sender, object args) => _menu = true;
    private void OnCollectionMenuClosed(object sender, object args) { _menu = false; InteractionEnded?.Invoke(); }
    private void OnSearchLoaded(object sender, RoutedEventArgs args)
    {
        static TextBox? Editor(DependencyObject parent)
        {
            for (var index = 0; index < VisualTreeHelper.GetChildrenCount(parent); index++)
            {
                var child = VisualTreeHelper.GetChild(parent, index);
                if (child is TextBox box) return box;
                if (Editor(child) is { } found) return found;
            }
            return null;
        }
        if (_searchEditor is not null) return;
        _searchEditor = Editor(Search);
        if (_searchEditor is null) return;
        _searchEditor.TextCompositionStarted += OnCompositionStarted;
        _searchEditor.TextCompositionEnded += OnCompositionEnded;
        if (!_compact && _model?.Visible == true) _searchEditor.Focus(FocusState.Programmatic);
    }
    private void OnCompositionStarted(TextBox sender, TextCompositionStartedEventArgs args) => _composing = true;
    private async void OnCompositionEnded(TextBox sender, TextCompositionEndedEventArgs args)
    { _composing = false; InteractionEnded?.Invoke(); if (_model?.Visible == true) await _model.SearchAsync(Search.Text); }
    private async void OnMore(object sender, RoutedEventArgs args)
    {
        if (_model is not null) { if (_model.Error is not null) await _model.ReloadAsync(); else await _model.NextAsync(); }
        else if (_main is not null) await _main.RetryResidentAsync();
    }
    private void OnOpenLibrary(object sender, RoutedEventArgs args) => Open?.Invoke(null);
    private async void OnPaste(object sender, RoutedEventArgs args)
    { if (Gallery.SelectedItem is StickerCard card) await ActivateAsync(card, true); }
    private async void OnItemClick(object sender, ItemClickEventArgs args)
    { if (args.ClickedItem is StickerCard card) await ActivateAsync(card, true); }
    private async Task ActivateAsync(StickerCard card, bool paste, StickerOutputKind kind = StickerOutputKind.Image)
    {
        if (Interacting || Output is null || _main?.ViewModel.IsRestoring != false || _model?.Visible != true) return;
        _working = true;
        Update();
        try { await Output(card, paste, kind); }
        catch (Exception error) { if (_main is not null) ShowMessage(_main.Strings.ErrorKey(error)); }
        finally { _working = false; Update(); InteractionEnded?.Invoke(); }
    }
    private async void OnKeyDown(object sender, KeyRoutedEventArgs args)
    {
        if (_composing) return;
        var ctrl = (Microsoft.UI.Input.InputKeyboardSource.GetKeyStateForCurrentThread(VirtualKey.Control) & global::Windows.UI.Core.CoreVirtualKeyStates.Down) != 0;
        if (args.Key == VirtualKey.Escape) { args.Handled = true; Dismiss?.Invoke(true); }
        else if (args.Key == VirtualKey.Enter && Gallery.SelectedItem is StickerCard enter)
        { args.Handled = true; await ActivateAsync(enter, true); }
        else if (ctrl && args.Key == VirtualKey.C && (_searchEditor?.FocusState == FocusState.Unfocused || _searchEditor?.SelectionLength == 0)
            && Gallery.SelectedItem is StickerCard copy)
        { args.Handled = true; await ActivateAsync(copy, false); }
        else if (_searchEditor is { FocusState: not FocusState.Unfocused } && args.Key is VirtualKey.Down or VirtualKey.Up or VirtualKey.Left or VirtualKey.Right)
        {
            if (Search.Text.Length > 0 && args.Key is VirtualKey.Left or VirtualKey.Right) return;
            var columns = Math.Max(1, (int)(Gallery.ActualWidth / 84));
            var delta = args.Key switch { VirtualKey.Down => columns, VirtualKey.Up => -columns, VirtualKey.Left => -1, _ => 1 };
            if (Gallery.Items.Count > 0) Gallery.SelectedIndex = Math.Clamp(Gallery.SelectedIndex + delta, 0, Gallery.Items.Count - 1);
            if (Gallery.SelectedItem is not null) Gallery.ScrollIntoView(Gallery.SelectedItem);
            args.Handled = true;
        }
    }
    private void OnContainer(ListViewBase sender, ContainerContentChangingEventArgs args)
    {
        if (args.Item is not StickerCard card || _model is null) return;
        if (args.InRecycleQueue) _model.Thumbnails.Recycle(card);
        else
        {
            _model.Thumbnails.Realize(card);
            if (!_compact && ReferenceEquals(card, _model.Cards.LastOrDefault())) _ = _model.NextAsync();
        }
    }
    private void OnContextRequested(UIElement sender, ContextRequestedEventArgs args)
    {
        if (sender is not FrameworkElement { DataContext: StickerCard card } || _main is null) return;
        var flyout = new MenuFlyout();
        void Add(string key, Action action)
        {
            var item = new MenuFlyoutItem { Text = _main.Strings[key] };
            if (key == "picker_copy") item.KeyboardAcceleratorTextOverride = "Ctrl+C";
            item.Click += (_, _) => { _menu = false; action(); }; flyout.Items.Add(item);
        }
        Add("picker_paste", async () => await ActivateAsync(card, true));
        Add("picker_copy", async () => await ActivateAsync(card, false));
        Add("picker_copy_file", async () => await ActivateAsync(card, false, StickerOutputKind.OriginalFile));
        Add("picker_save", async () =>
        {
            if (Save is null || Interacting) return;
            _working = true; _modalOperation = true;
            try { await Save(card); }
            finally { _working = false; _modalOperation = false; InteractionEnded?.Invoke(); }
        });
        Add("picker_open_library", () => Open?.Invoke(card.Id));
        flyout.Opened += (_, _) => _menu = true;
        flyout.Closed += (_, _) => { _menu = false; InteractionEnded?.Invoke(); };
        args.Handled = true;
        flyout.ShowAt((FrameworkElement)sender);
    }
    private void OnDragStarting(object sender, DragItemsStartingEventArgs args)
    {
        args.Cancel = true;
        if (Interacting || args.Items.Count != 1 || args.Items[0] is not StickerCard card
            || _main?.ViewModel.Output is not { CanStart: true } output || _main.ViewModel.IsRestoring) return;
        _dragging = true;
        var package = new DataPackage();
        _dragPreparation = output.PrepareDragAsync(new(card.Id, card.Generation), package);
        var preparation = _dragPreparation;
        args.Data.RequestedOperation = DataPackageOperation.Copy;
        args.Data.SetData(StickerImportService.InternalDragFormat, true);
        args.Data.SetDataProvider(StandardDataFormats.StorageItems, request =>
        {
            var deferral = request.GetDeferral();
            if (!DispatcherQueue.TryEnqueue(async () =>
            {
                try { if (await preparation) request.SetData(await package.GetView().GetStorageItemsAsync()); }
                catch (Exception error) { Trace.WriteLine($"Picker drag: {error.GetType().Name}"); }
                finally { deferral.Complete(); }
            })) deferral.Complete();
        });
        args.Cancel = false;
    }
    private async void OnDragCompleted(ListViewBase sender, DragItemsCompletedEventArgs args)
    {
        try
        {
            if (_dragPreparation is not null) await _dragPreparation;
            if (_main?.ViewModel.Output is { } output) await output.CompleteDragAsync(args.DropResult != DataPackageOperation.None);
        }
        finally { _dragging = false; _dragPreparation = null; if (args.DropResult != DataPackageOperation.None) Dismiss?.Invoke(false); InteractionEnded?.Invoke(); }
    }
}
