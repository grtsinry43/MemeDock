using System.ComponentModel;
using System.Diagnostics;
using MemeDock.Bridge;
using MemeDock.Windows.Localization;
using MemeDock.Windows.Models;
using MemeDock.Windows.Services;
using MemeDock.Windows.ViewModels;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.Windows.Storage.Pickers;

namespace MemeDock.Windows;

public sealed partial class MainWindow : Window
{
    public LocalizationService Strings { get; } = new();
    public LibraryViewModel ViewModel { get; }
    private readonly CancellationTokenSource _lifetime = new();
    private readonly AppPreferences _preferences;
    private readonly string _root;
    private CancellationTokenSource? _searchCancellation;
    private Task _initialization = Task.CompletedTask;
    private Task _searching = Task.CompletedTask;
    private Task _importing = Task.CompletedTask;
    private ScrollViewer? _scroll;
    private bool _initialized;
    private bool _updatingNavigation;
    private bool _composing;
    private bool _settingsVisible;
    private bool _tagMode;
    private bool _stopping;
    private bool _canClose;

    public MainWindow()
    {
        ViewModel = new LibraryViewModel(Strings);
        _root = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "MemeDock");
        _preferences = new AppPreferences(_root);
        InitializeComponent();
        ExtendsContentIntoTitleBar = true;
        SetTitleBar(WindowTitleBar);
        SystemBackdrop = new MicaBackdrop();
        AppWindow.Resize(new global::Windows.Graphics.SizeInt32(1180, 820));
        AppWindow.Closing += OnClosing;
        Strings.PropertyChanged += OnLanguageChanged;
        ViewModel.PropertyChanged += OnModelChanged;
        ViewModel.CatalogChanged += OnCatalogChanged;
        ViewModel.ThumbnailInvalidated += OnThumbnailInvalidated;
        ApplyLabels();
        _updatingNavigation = true;
        Navigation.SelectedItem = AllNavigation;
        _updatingNavigation = false;
    }

    public static Visibility ToVisibility(bool value) => value ? Visibility.Visible : Visibility.Collapsed;

    private async void OnLoaded(object sender, RoutedEventArgs args)
    {
        if (_initialized) return;
        _initialized = true;
        _initialization = InitializeAsync();
        await _initialization;
    }

    private async Task InitializeAsync()
    {
        await _preferences.LoadAsync();
        if (_stopping) return;
        Strings.SetLanguage(_preferences.Language);
        Settings.Configure(Strings, _preferences);
        await ViewModel.InitializeAsync(_root);
        UpdateState();
    }

    private void ApplyLabels()
    {
        AllNavigation.Content = Strings["filter_all"];
        StarredNavigation.Content = Strings["favorites_only"];
        RecentNavigation.Content = Strings["filter_recent"];
        CollectionsNavigation.Content = Strings["tab_collections"];
        TagsNavigation.Content = Strings["tags_title"];
        SettingsNavigation.Content = Strings["desktop_settings"];
        ImportButton.Content = Strings["library_add"];
        Search.PlaceholderText = Strings["library_search"];
        AutomationProperties.SetName(Search, Strings["library_search"]);
        AutomationProperties.SetName(Gallery, Strings["tab_stickers"]);
        SortButton.Content = Strings["desktop_sort"];
        SortRecent.Text = Strings["desktop_sort_recent"];
        SortUsed.Text = Strings["filter_recent"];
        SortCollection.Text = Strings["desktop_sort_collection"];
        RetryButton.Content = Strings["retry"];
        SelectAllButton.Content = Strings["desktop_select_all"];
        ClearSelectionButton.Content = Strings["desktop_clear_selection"];
        SelectionHint.Text = Strings["desktop_selection_loaded_only"];
        TagSelector.PlaceholderText = Strings["tags_title"];
        AutomationProperties.SetName(TagSelector, Strings["tags_title"]);
        Settings.Relocalize();
        ViewModel.Relocalize();
        UpdateState();
    }

    private void OnLanguageChanged(object? sender, PropertyChangedEventArgs args) => ApplyLabels();
    private void OnModelChanged(object? sender, PropertyChangedEventArgs args) => UpdateState();

    private void UpdateState()
    {
        ImportButton.IsEnabled = ViewModel.IsReady && !ViewModel.IsImporting && !_stopping;
        Search.IsEnabled = ViewModel.IsReady && !_stopping;
        SortButton.IsEnabled = ViewModel.IsReady && ViewModel.Filter.Scope != LibraryScope.Recent;
        Busy.IsActive = !ViewModel.IsReady && ViewModel.ErrorKey is null || ViewModel.IsLoading || ViewModel.IsImporting;
        Busy.Visibility = ToVisibility(Busy.IsActive);
        ErrorBar.IsOpen = ViewModel.ErrorKey is not null;
        ErrorBar.Message = ViewModel.ErrorText;
        CatalogErrorBar.IsOpen = ViewModel.CatalogErrorKey is not null;
        CatalogErrorBar.Message = ViewModel.CatalogErrorText;
        EmptyState.Visibility = ToVisibility(ViewModel.IsReady && !ViewModel.IsLoading && ViewModel.Cards.Count == 0 && ViewModel.ErrorKey is null);
        EmptyHint.Text = ViewModel.SearchText.Length > 0 ? Strings["library_no_matches_hint"] : "";
        Status.Text = !ViewModel.IsReady || ViewModel.IsLoading ? Strings["import_preparing"]
            : ViewModel.ImportText;
        SelectionBar.Visibility = ToVisibility(Gallery.SelectedItems.Count > 0);
        SelectionHint.Visibility = ToVisibility(ViewModel.HasMore);
        SortCollection.Visibility = ToVisibility(ViewModel.Filter.AllowsCollectionOrder);
        SortRecent.IsChecked = ViewModel.Sort == StickerSort.Recent;
        SortUsed.IsChecked = ViewModel.Sort == StickerSort.LastUsed;
        SortCollection.IsChecked = ViewModel.Sort == StickerSort.CollectionOrder;
    }

    private void OnPaneToggle(TitleBar sender, object args) => Navigation.IsPaneOpen = !Navigation.IsPaneOpen;

    private async void OnNavigationChanged(NavigationView sender, NavigationViewSelectionChangedEventArgs args)
    {
        if (_updatingNavigation || _stopping || args.SelectedItemContainer is not NavigationViewItem item) return;
        _settingsVisible = item == SettingsNavigation;
        LibraryContent.Visibility = ToVisibility(!_settingsVisible);
        Settings.Visibility = ToVisibility(_settingsVisible);
        if (_settingsVisible) return;
        _tagMode = item == TagsNavigation;
        TagSelector.Visibility = ToVisibility(_tagMode);
        LibraryFilter? filter = item.Tag switch
        {
            "all" => LibraryFilter.All,
            "starred" => new(LibraryScope.Starred),
            "recent" => new(LibraryScope.Recent),
            CollectionMetadata collection => new(LibraryScope.Collection, collection.Id, collection.Name),
            _ => null
        };
        if (_tagMode && ViewModel.Tags.FirstOrDefault() is { } tag)
        {
            _updatingNavigation = true;
            TagSelector.SelectedValue = tag.Id;
            _updatingNavigation = false;
            filter = new(LibraryScope.Tag, tag.Id, tag.Name);
        }
        if (filter is null) return;
        Gallery.SelectedItems.Clear();
        await ViewModel.NavigateAsync(filter);
        _scroll?.ChangeView(null, 0, null, disableAnimation: true);
    }

    private void OnCatalogChanged(object? sender, EventArgs args)
    {
        _updatingNavigation = true;
        try
        {
            CollectionsNavigation.MenuItems.Clear();
            foreach (var collection in ViewModel.Collections)
                CollectionsNavigation.MenuItems.Add(new NavigationViewItem { Content = collection.Name, Tag = collection });
            CollectionsNavigation.IsEnabled = ViewModel.Collections.Count > 0;
            TagsNavigation.IsEnabled = ViewModel.Tags.Count > 0;
            if (!_settingsVisible)
            {
                Navigation.SelectedItem = ViewModel.Filter.Scope switch
                {
                    LibraryScope.Starred => StarredNavigation,
                    LibraryScope.Recent => RecentNavigation,
                    LibraryScope.Tag => TagsNavigation,
                    LibraryScope.Collection => CollectionsNavigation.MenuItems.OfType<NavigationViewItem>()
                        .FirstOrDefault(item => item.Tag is CollectionMetadata value && value.Id == ViewModel.Filter.Id),
                    _ => AllNavigation
                };
                _tagMode = ViewModel.Filter.Scope == LibraryScope.Tag;
                TagSelector.Visibility = ToVisibility(_tagMode);
                if (_tagMode) TagSelector.SelectedValue = ViewModel.Filter.Id;
            }
        }
        finally { _updatingNavigation = false; }
    }

    private async void OnTagChanged(object sender, SelectionChangedEventArgs args)
    {
        if (_updatingNavigation || !_tagMode || _stopping || TagSelector.SelectedItem is not TagMetadata tag) return;
        Gallery.SelectedItems.Clear();
        await ViewModel.NavigateAsync(new(LibraryScope.Tag, tag.Id, tag.Name));
    }

    private async void OnSort(object sender, RoutedEventArgs args)
    {
        if (_stopping || sender is not RadioMenuFlyoutItem item) return;
        Gallery.SelectedItems.Clear();
        await ViewModel.SortAsync(Enum.Parse<StickerSort>((string)item.Tag));
    }

    private void OnSearchChanged(object sender, TextChangedEventArgs args)
    {
        if (!_composing) QueueSearch();
    }

    private void OnCompositionStarted(TextBox sender, TextCompositionStartedEventArgs args)
    {
        _composing = true;
        _searchCancellation?.Cancel();
    }

    private void OnCompositionEnded(TextBox sender, TextCompositionEndedEventArgs args)
    {
        _composing = false;
        QueueSearch();
    }

    private void QueueSearch()
    {
        _searchCancellation?.Cancel();
        _searchCancellation?.Dispose();
        if (!ViewModel.IsReady || _stopping) return;
        _searchCancellation = CancellationTokenSource.CreateLinkedTokenSource(_lifetime.Token);
        _searching = SearchAfterDelayAsync(_searchCancellation.Token);
    }

    private async Task SearchAfterDelayAsync(CancellationToken cancellation)
    {
        try
        {
            await Task.Delay(250, cancellation);
            if (_composing || _stopping) return;
            Gallery.SelectedItems.Clear();
            await ViewModel.SearchAsync(Search.Text);
        }
        catch (OperationCanceledException) { }
    }

    private void OnGalleryLoaded(object sender, RoutedEventArgs args)
    {
        if (_scroll is not null) _scroll.ViewChanged -= OnViewChanged;
        _scroll = FindScrollViewer(Gallery);
        if (_scroll is not null) _scroll.ViewChanged += OnViewChanged;
    }

    private async void OnViewChanged(object? sender, ScrollViewerViewChangedEventArgs args)
    {
        if (_stopping || _scroll is null || ViewModel.IsLoading || !ViewModel.HasMore) return;
        if (_scroll.ScrollableHeight - _scroll.VerticalOffset < 700) await ViewModel.LoadNextAsync();
    }

    private static ScrollViewer? FindScrollViewer(DependencyObject parent)
    {
        if (parent is ScrollViewer scroll) return scroll;
        for (var index = 0; index < VisualTreeHelper.GetChildrenCount(parent); index++)
            if (FindScrollViewer(VisualTreeHelper.GetChild(parent, index)) is { } found) return found;
        return null;
    }

    private void OnContainerContentChanging(ListViewBase sender, ContainerContentChangingEventArgs args)
    {
        if (args.Item is not StickerCard card) return;
        if (args.InRecycleQueue) ViewModel.Thumbnails?.Recycle(card);
        else if (args.Phase == 0) ViewModel.Thumbnails?.Realize(card);
    }

    private void OnThumbnailInvalidated(object? sender, string id)
    {
        if (ViewModel.Cards.FirstOrDefault(card => card.Id == id) is { } card) ViewModel.Thumbnails?.Invalidate(card);
    }

    private void OnRetryThumbnail(object sender, RoutedEventArgs args)
    {
        if (sender is Button { Tag: StickerCard card })
        {
            card.Thumbnail = null;
            ViewModel.Thumbnails?.Invalidate(card);
        }
    }

    private void OnThumbnailRetryLoaded(object sender, RoutedEventArgs args)
    {
        if (sender is Button button)
        {
            AutomationProperties.SetName(button, Strings["retry_thumbnail"]);
            ToolTipService.SetToolTip(button, Strings["retry_thumbnail"]);
        }
    }

    private void OnSelectionChanged(object sender, SelectionChangedEventArgs args)
    {
        ViewModel.SetSelectedCount(Gallery.SelectedItems.Count);
        UpdateState();
    }
    private void OnSelectAll(object sender, RoutedEventArgs args) => Gallery.SelectAll();
    private void OnClearSelection(object sender, RoutedEventArgs args) => Gallery.SelectedItems.Clear();

    private void OnFocusSearch(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        if (_settingsVisible) return;
        Search.Focus(FocusState.Keyboard);
        args.Handled = true;
    }
    private bool EditingText => FocusManager.GetFocusedElement(Root.XamlRoot) is TextBox or RichEditBox or PasswordBox;
    private void OnSelectAllShortcut(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        if (_settingsVisible || EditingText) return;
        Gallery.SelectAll();
        args.Handled = true;
    }
    private void OnEscape(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        if (_composing || _settingsVisible || EditingText) return;
        Gallery.SelectedItems.Clear();
        args.Handled = true;
    }
    private async void OnImportShortcut(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        if (!ImportButton.IsEnabled || _settingsVisible) return;
        args.Handled = true;
        await StartImportAsync();
    }
    private async void OnImport(object sender, RoutedEventArgs args) => await StartImportAsync();
    private Task StartImportAsync()
    {
        if (_stopping || !_importing.IsCompleted || !ViewModel.IsReady) return Task.CompletedTask;
        return _importing = PickAndImportAsync();
    }
    private async Task PickAndImportAsync()
    {
        try
        {
            var picker = new FileOpenPicker(AppWindow.Id);
            foreach (var extension in new[] { ".png", ".jpg", ".jpeg", ".gif", ".webp" }) picker.FileTypeFilter.Add(extension);
            var files = await picker.PickMultipleFilesAsync();
            if (!_stopping) await ViewModel.ImportAsync(files.Select(file => file.Path).ToArray());
        }
        catch (Exception error)
        {
            Trace.WriteLine(error);
            ErrorBar.Message = Strings[Strings.ErrorKey(error)];
            ErrorBar.IsOpen = true;
        }
    }
    private async void OnRetry(object sender, RoutedEventArgs args)
    {
        if (_stopping) return;
        if (ViewModel.IsReady) await ViewModel.RetryAsync();
        else { _initialization = ViewModel.InitializeAsync(_root); await _initialization; }
    }

    private async void OnClosing(AppWindow sender, AppWindowClosingEventArgs args)
    {
        if (_canClose) return;
        args.Cancel = true;
        if (_stopping) return;
        _stopping = true;
        Root.IsHitTestVisible = false;
        _lifetime.Cancel();
        ViewModel.RequestStop();
        try
        {
            await Task.WhenAll(_initialization, _searching, _importing, Settings.PendingSave);
            await ViewModel.DisposeAsync();
        }
        catch (Exception error) { Trace.WriteLine($"Library shutdown: {error}"); }
        finally
        {
            if (_scroll is not null) _scroll.ViewChanged -= OnViewChanged;
            Strings.PropertyChanged -= OnLanguageChanged;
            ViewModel.PropertyChanged -= OnModelChanged;
            ViewModel.CatalogChanged -= OnCatalogChanged;
            ViewModel.ThumbnailInvalidated -= OnThumbnailInvalidated;
            _searchCancellation?.Dispose();
            _lifetime.Dispose();
            _canClose = true;
            Close();
        }
    }
}
