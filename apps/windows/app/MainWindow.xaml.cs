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
using Windows.ApplicationModel.DataTransfer;

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
    private bool _catalogVisible;
    private bool _childrenConfigured;
    private bool _restoringSelection;
    private bool _undoAvailable;
    private long _selectionVersion;
    private bool _tagMode;
    private bool _stopping;
    private bool _canClose;
    private bool _windowActive = true;
    private bool _reordering;
    private string? _reorderCollection;
    private Task<bool>? _galleryDragPreparation;
    private SyncCoordinator? _sync;
    private readonly BackupDestinationPicker _backupPicker;
    private readonly ArchiveSourcePicker _archivePicker;

    public MainWindow()
    {
        _root = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "MemeDock");
        _preferences = new AppPreferences(_root);
        // Read before the first frame so the saved appearance does not flash the
        // system theme first. The file is small local JSON; the async read in
        // InitializeAsync would be too late for the window's own visual tree.
        _preferences.Load();
        ViewModel = new LibraryViewModel(Strings, _preferences);
        InitializeComponent();
        ExtendsContentIntoTitleBar = true;
        SetTitleBar(WindowTitleBar);
        // Unpackaged WinUI apps cannot change Application.RequestedTheme at runtime,
        // so the saved appearance is applied to the window's content root instead.
        ApplyAppearance(_preferences.Appearance);
        // The app is unpackaged, so the save dialog has to be constructed with the
        // window's native identity. AppWindow.Id is that HWND-backed WindowId, and it
        // stays valid for the life of the window, so one picker serves every backup.
        _backupPicker = new BackupDestinationPicker(AppWindow.Id, Strings);
        // Same identity rule as the save dialog: the open dialog is modal to this
        // window through its native handle.
        _archivePicker = new ArchiveSourcePicker(AppWindow.Id);
        Settings.AppearanceChanged += OnAppearanceChanged;
        SystemBackdrop = new MicaBackdrop();
        AppWindow.Resize(new global::Windows.Graphics.SizeInt32(1180, 820));
        AppWindow.Closing += OnClosing;
        Strings.PropertyChanged += OnLanguageChanged;
        ViewModel.PropertyChanged += OnModelChanged;
        ViewModel.CatalogChanged += OnCatalogChanged;
        ViewModel.ThumbnailInvalidated += OnThumbnailInvalidated;
        ViewModel.ContentChanged += OnContentChanged;
        ViewModel.Rebuilt += OnRebuilt;
        Activated += OnActivated;
        ApplyLabels();
        _updatingNavigation = true;
        Navigation.SelectedItem = AllNavigation;
        _updatingNavigation = false;
    }

    public static Visibility ToVisibility(bool value) => value ? Visibility.Visible : Visibility.Collapsed;

    /// <summary>
    /// The window content root owns the appearance: every control inside it inherits
    /// the requested theme, while <see cref="ElementTheme.Default"/> follows the system
    /// again without restarting the process.
    /// </summary>
    private void ApplyAppearance(AppearanceMode mode)
    {
        Root.RequestedTheme = mode switch
        {
            AppearanceMode.Light => ElementTheme.Light,
            AppearanceMode.Dark => ElementTheme.Dark,
            _ => ElementTheme.Default
        };
        try
        {
            // The caption buttons are drawn by the system outside the XAML tree;
            // PreferredTheme keeps them on the same side as the content.
            AppWindow.TitleBar.PreferredTheme = mode switch
            {
                AppearanceMode.Light => TitleBarTheme.Light,
                AppearanceMode.Dark => TitleBarTheme.Dark,
                _ => TitleBarTheme.UseDefaultAppMode
            };
        }
        catch (Exception error)
        {
            // Older shells may not support the property; the content theme still applies.
            Trace.WriteLine($"The title bar theme could not be applied: {error.GetType().Name}");
        }
    }

    private void OnAppearanceChanged(AppearanceMode mode) => ApplyAppearance(mode);

    private async void OnLoaded(object sender, RoutedEventArgs args)
    {
        if (_initialized) return;
        _initialized = true;
        _initialization = InitializeAsync();
        await _initialization;
    }

    private async Task InitializeAsync()
    {
        // Preferences were read synchronously in the constructor so the appearance
        // applied to the first frame; only the language needs to reach the strings.
        Strings.SetLanguage(_preferences.Language);
        Settings.Configure(Strings, _preferences);
        await ViewModel.InitializeAsync(_root, _backupPicker, _archivePicker);
        if (_stopping) return;
        Settings.ShowIdentity(ViewModel.Identity);
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
        CatalogNavigation.Content = Strings["organize"];
        UndoButton.Content = Strings["restore"];
        ImportLabel.Text = Strings["library_add"];
        AutomationProperties.SetName(ImportButton, Strings["library_add"]);
        Search.PlaceholderText = Strings["library_search"];
        AutomationProperties.SetName(Search, Strings["library_search"]);
        AutomationProperties.SetName(Gallery, Strings["tab_stickers"]);
        AutomationProperties.SetName(SortButton, Strings["desktop_sort"]);
        ToolTipService.SetToolTip(SortButton, Strings["desktop_sort"]);
        SortRecent.Text = Strings["desktop_sort_recent"];
        SortUsed.Text = Strings["filter_recent"];
        SortCollection.Text = Strings["desktop_sort_collection"];
        ReorderMenu.Text = Strings["reorder"];
        ReorderDone.Content = Strings["done"];
        RetryButton.Content = Strings["retry"];
        SelectAllButton.Label = Strings["desktop_select_all"];
        ClearSelectionButton.Label = Strings["desktop_clear_selection"];
        ToolTipService.SetToolTip(SelectAllButton, SelectAllButton.Label);
        ToolTipService.SetToolTip(ClearSelectionButton, ClearSelectionButton.Label);
        SelectionHint.Text = Strings["desktop_selection_loaded_only"];
        TrashNavigation.Content = Strings["trash"];
        RestoreButton.Content = Strings["restore"];
        PurgeButton.Content = Strings["purge_title"];
        AutomationProperties.SetName(TrashNavigation, Strings["trash"]);
        AutomationProperties.SetName(RestoreButton, Strings["restore"]);
        AutomationProperties.SetName(PurgeButton, Strings["purge_title"]);
        TagSelector.PlaceholderText = Strings["tags_title"];
        AutomationProperties.SetName(TagSelector, Strings["tags_title"]);
        Settings.Relocalize();
        Details.Relocalize();
        Batch.Relocalize();
        Catalog.Relocalize();
        TrashGroupsPanel.Relocalize();
        ViewModel.Relocalize();
        UpdateState();
    }

    private void OnLanguageChanged(object? sender, PropertyChangedEventArgs args) => ApplyLabels();
    private void OnModelChanged(object? sender, PropertyChangedEventArgs args) => UpdateState();

    private void UpdateState()
    {
        if (ViewModel.IsReady && !_childrenConfigured && ViewModel.Details is { } details && ViewModel.Batch is { } batch
            && ViewModel.Catalog is { } catalog && ViewModel.TrashGroups is { } trashGroups && ViewModel.Output is { } output
            && ViewModel.Storage is { } storage && ViewModel.Backup is { } backup)
        {
            _childrenConfigured = true;
            Details.Configure(details, Strings, ViewModel.Collections, ViewModel.Tags, output);
            Batch.Configure(batch, Strings, ViewModel.Collections, ViewModel.Tags);
            Catalog.Configure(catalog, Strings, ViewModel.Collections, ViewModel.Tags);
            TrashGroupsPanel.Configure(trashGroups, Strings);
            Settings.ConfigureSync(ViewModel.Sync, Strings);
            Settings.ConfigureStorage(storage);
            Settings.ConfigureBackup(backup);
            details.Deleted += OnStickerDeleted;
            batch.PropertyChanged += OnModelChanged;
            batch.Completed += OnBatchCompleted;
            trashGroups.PropertyChanged += OnModelChanged;
            output.PropertyChanged += OnModelChanged;
            output.SetForeground(_windowActive);
        }
        // The coordinator outlives this window only until shutdown, so the
        // handler is attached once per instance and dropped in the finally
        // block of OnClosing.
        AttachSync(ViewModel.Sync);
        var batchRunning = ViewModel.Batch?.IsRunning == true;
        // A restore either holds the session open for a merge or is about to close it
        // for a replace; either way nothing else may start work on it meanwhile.
        var restoring = ViewModel.IsRestoring;
        var blocked = batchRunning || restoring;
        var trash = ViewModel.IsTrash;
        var trashGroupsVisible = trash && ViewModel.TrashGroups?.HasVisibleContent == true;
        Gallery.IsEnabled = !blocked && !ViewModel.IsLoading && !ViewModel.IsTrashing;
        AllNavigation.IsEnabled = StarredNavigation.IsEnabled = RecentNavigation.IsEnabled = !blocked;
        TrashNavigation.IsEnabled = !blocked;
        CatalogNavigation.IsEnabled = SettingsNavigation.IsEnabled = !blocked;
        ImportButton.IsEnabled = ViewModel.IsReady && !ViewModel.IsImporting && !_stopping && !blocked && !trash;
        Search.IsEnabled = ViewModel.IsReady && !_stopping && !blocked;
        SortButton.IsEnabled = ViewModel.IsReady && ViewModel.Filter.Scope != LibraryScope.Recent && !blocked;
        Busy.IsActive = !ViewModel.IsReady && ViewModel.ErrorKey is null || ViewModel.IsLoading || ViewModel.IsImporting
            || ViewModel.IsTrashing || ViewModel.Output is { IsBusy: true, IsDragging: false };
        Busy.Visibility = ToVisibility(Busy.IsActive);
        ErrorBar.IsOpen = ViewModel.ErrorKey is not null;
        ErrorBar.Message = ViewModel.ErrorText;
        CatalogErrorBar.IsOpen = ViewModel.CatalogErrorKey is not null;
        CatalogErrorBar.Message = ViewModel.CatalogErrorText;
        OutputErrorBar.IsOpen = ViewModel.Output?.ErrorKey is not null;
        OutputErrorBar.Message = ViewModel.Output?.StatusText ?? "";
        TrashGroupsPanel.Visibility = ToVisibility(trashGroupsVisible);
        EmptyState.Visibility = ToVisibility(ViewModel.IsReady && !ViewModel.IsLoading && ViewModel.Cards.Count == 0
            && (!trashGroupsVisible || ViewModel.SearchText.Length > 0) && ViewModel.ErrorKey is null);
        EmptyHint.Text = ViewModel.SearchText.Length > 0 ? Strings["library_no_matches_hint"]
            : trash ? Strings["trash_empty_hint"] : "";
        Status.Text = !ViewModel.IsReady || ViewModel.IsLoading ? Strings["import_preparing"]
            : ViewModel.Output is { StatusText.Length: > 0 } status ? status.StatusText : ViewModel.ImportText;
        SelectionBar.Visibility = ToVisibility(Gallery.SelectedItems.Count > 0);
        SelectionHint.Visibility = ToVisibility(ViewModel.HasMore);
        // Restore and purge replace undo in the trash, where the only meaningful
        // actions are on already deleted stickers.
        RestoreButton.Visibility = ToVisibility(trash);
        PurgeButton.Visibility = ToVisibility(trash);
        RestoreButton.IsEnabled = !ViewModel.IsTrashing;
        PurgeButton.IsEnabled = !ViewModel.IsTrashing;
        // Undo exists only while the last message is the delete one and only
        // outside the trash, so a trash result never inherits a stale action.
        UndoButton.Visibility = ToVisibility(!trash && _undoAvailable);
        SortCollection.Visibility = ToVisibility(ViewModel.Filter.AllowsCollectionOrder);
        SortRecent.IsChecked = ViewModel.Sort == StickerSort.Recent;
        SortUsed.IsChecked = ViewModel.Sort == StickerSort.LastUsed;
        SortCollection.IsChecked = ViewModel.Sort == StickerSort.CollectionOrder;
        if (!ViewModel.Filter.AllowsCollectionOrder || ViewModel.Filter.Id != _reorderCollection
            || ViewModel.SearchText.Length > 0 || ViewModel.Sort != StickerSort.CollectionOrder) _reordering = false;
        var reorder = ViewModel.Filter.AllowsCollectionOrder && !ViewModel.HasMore && !ViewModel.IsLoading
            && ViewModel.SearchText.Length == 0 && !blocked && ViewModel.Output?.IsBusy != true;
        ReorderMenu.IsEnabled = reorder;
        ReorderMenu.Visibility = ToVisibility(ViewModel.Filter.AllowsCollectionOrder);
        ReorderMenu.IsChecked = _reordering;
        ReorderDone.Visibility = ToVisibility(_reordering);
        Gallery.CanDragItems = _reordering && reorder || CanDragOut;
        Gallery.CanReorderItems = Gallery.AllowDrop = _reordering && reorder;
        UpdateSyncStatus();
        Details.SetActive(_windowActive && Inspector.IsPaneOpen && !_settingsVisible && !_catalogVisible && Details.Visibility == Visibility.Visible);
    }

    /// <summary>
    /// A replace closed the library and every view model was rebuilt behind a new
    /// session. The XAML tree itself survives, so it is re-pointed at the new children
    /// instead of being recreated; anything still holding old cards is cleared first.
    /// </summary>
    private void OnRebuilt(object? sender, EventArgs args)
    {
        if (_stopping) return;
        DetachChildren();
        _childrenConfigured = false;
        _undoAvailable = false;
        _reordering = false;
        _reorderCollection = null;
        _selectionVersion++;
        OperationBar.IsOpen = false;
        Gallery.SelectedItems.Clear();
        // The clear above is ignored while IsRestoring is true, so the panes that showed
        // the old library's selection are reset here instead.
        Details.Visibility = Visibility.Collapsed;
        Batch.Visibility = Visibility.Collapsed;
        Inspector.IsPaneOpen = false;
        ViewModel.SetSelectedCount(0);
        // The browse state was reset to unfiltered, so the box must not keep showing a
        // query that is no longer applied to the new library.
        Search.Text = "";
        Settings.ShowIdentity(ViewModel.Identity);
        UpdateState();
    }

    /// <summary>
    /// Drops the handlers attached to the previous generation of child view models so a
    /// rebuild cannot leave two pages listening to the same event.
    /// </summary>
    private void DetachChildren()
    {
        if (ViewModel.Details is { } details) details.Deleted -= OnStickerDeleted;
        if (ViewModel.Batch is { } batch)
        {
            batch.PropertyChanged -= OnModelChanged;
            batch.Completed -= OnBatchCompleted;
        }
        if (ViewModel.TrashGroups is { } trashGroups) trashGroups.PropertyChanged -= OnModelChanged;
        if (ViewModel.Output is { } output) output.PropertyChanged -= OnModelChanged;
    }

    private void AttachSync(SyncCoordinator? sync)
    {
        if (ReferenceEquals(_sync, sync)) return;
        if (_sync is not null) _sync.StateChanged -= OnSyncStateChanged;
        _sync = sync;
        if (_sync is not null) _sync.StateChanged += OnSyncStateChanged;
    }

    private void OnSyncStateChanged(object? sender, EventArgs args)
    {
        if (_stopping) return;
        // The coordinator notifies from whichever thread finished the native
        // call; the status bar may only be touched on the UI thread.
        DispatcherQueue?.TryEnqueue(() =>
        {
            if (!_stopping) UpdateSyncStatus();
        });
    }

    private void UpdateSyncStatus()
    {
        if (ViewModel.Sync?.State is not { } state || !SyncPresentation.IsVisible(state))
        {
            SyncStatusButton.Visibility = Visibility.Collapsed;
            return;
        }

        var phase = SyncPresentation.Phase(state);
        var text = Strings[SyncPresentation.StatusKey(state)];

        SyncStatusLabel.Text = text;
        SyncStatusIcon.Glyph = phase switch
        {
            SyncPhase.Running => "\uE895",
            SyncPhase.Authorization => "\uE72E",
            SyncPhase.Paused => "\uE769",
            SyncPhase.Blocked => "\uE7BA",
            SyncPhase.Failed => "\uE783",
            SyncPhase.Pending or SyncPhase.Unsynced => "\uE917",
            _ => "\uE73E",
        };
        // The three brushes are declared in this window's XAML resources, so a
        // direct indexer read is enough. ResourceDictionary's C# projection
        // exposes the IDictionary members (indexer, ContainsKey, TryGetValue);
        // the WinRT ABI names Lookup/HasKey are not callable from C#.
        var brush = phase switch
        {
            SyncPhase.Failed or SyncPhase.Authorization => Root.Resources["SyncFailureBrush"] as Brush,
            SyncPhase.Blocked or SyncPhase.Pending or SyncPhase.Unsynced => Root.Resources["SyncAttentionBrush"] as Brush,
            _ => Root.Resources["SyncIdleBrush"] as Brush,
        };
        if (brush is not null)
        {
            SyncStatusIcon.Foreground = brush;
            SyncStatusLabel.Foreground = brush;
        }

        // The hint carries the actionable part (blocked changes, expired token,
        // retryable failure) without adding another permanent banner.
        var hint = SyncPresentation.HintKey(state);
        ToolTipService.SetToolTip(SyncStatusButton, hint is null ? text : Strings[hint]);
        AutomationProperties.SetName(SyncStatusButton, text);
        // Settings navigation is disabled while a batch runs; keep the entry
        // from promising a destination it cannot reach.
        SyncStatusButton.IsEnabled = ViewModel.Batch?.IsRunning != true && !ViewModel.IsRestoring;
        SyncStatusButton.Visibility = Visibility.Visible;
    }

    private void OnOpenSyncSettings(object sender, RoutedEventArgs args)
    {
        if (_stopping || ViewModel.Batch?.IsRunning == true || ViewModel.IsRestoring) return;
        Navigation.SelectedItem = SettingsNavigation;
    }

    private void OnPaneToggle(TitleBar sender, object args) => Navigation.IsPaneOpen = !Navigation.IsPaneOpen;

    private async void OnNavigationChanged(NavigationView sender, NavigationViewSelectionChangedEventArgs args)
    {
        if (_updatingNavigation || _stopping || args.SelectedItemContainer is not NavigationViewItem item) return;
        if (ViewModel.Batch?.IsRunning == true || ViewModel.IsRestoring)
        {
            OnCatalogChanged(this, EventArgs.Empty);
            return;
        }
        if (ViewModel.Details is { } detail)
        {
            if (!await detail.CommitAsync())
            {
                OnCatalogChanged(this, EventArgs.Empty);
                return;
            }
            // Committing is asynchronous: the window may have started closing
            // while the editor saved, and a closing window must not navigate.
            if (_stopping) return;
        }
        _settingsVisible = item == SettingsNavigation;
        _catalogVisible = item == CatalogNavigation;
        Inspector.Visibility = ToVisibility(!_settingsVisible && !_catalogVisible);
        Settings.Visibility = ToVisibility(_settingsVisible);
        Catalog.Visibility = ToVisibility(_catalogVisible);
        Details.SetActive(!_settingsVisible && !_catalogVisible && Inspector.IsPaneOpen);
        // Disk figures age while the user works elsewhere; re-read on every visit.
        if (_settingsVisible) _ = Settings.RefreshStorageAsync();
        if (_settingsVisible || _catalogVisible) return;
        _tagMode = item == TagsNavigation;
        TagSelector.Visibility = ToVisibility(_tagMode);
        if (item == TrashNavigation && ViewModel.TrashGroups is { } trashGroups)
        {
            await trashGroups.RefreshAsync();
            // The read is asynchronous too; after it the panel may be gone.
            if (_stopping) return;
        }
        if (item == TrashNavigation)
        {
            // Deleted stickers are read-only here, so the inspector stays closed
            // instead of opening editors that the core would reject.
            Inspector.IsPaneOpen = false;
            Details.SetActive(false);
        }
        LibraryFilter? filter = item.Tag switch
        {
            "all" => LibraryFilter.All,
            "starred" => new(LibraryScope.Starred),
            "recent" => new(LibraryScope.Recent),
            "trash" => new(LibraryScope.Trash),
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
        // Navigating is asynchronous: touching the gallery or its scroll view
        // after the window closed would reach released controls.
        if (_stopping) return;
        _scroll?.ChangeView(null, 0, null, disableAnimation: true);
    }

    private void OnCatalogChanged(object? sender, EventArgs args)
    {
        // A catalog event is not a reason to rebuild navigation on a closing
        // window; the controls are being detached.
        if (_stopping) return;
        _updatingNavigation = true;
        try
        {
            CollectionsNavigation.MenuItems.Clear();
            foreach (var collection in ViewModel.Collections)
                CollectionsNavigation.MenuItems.Add(new NavigationViewItem { Content = collection.Name, Tag = collection });
            CollectionsNavigation.IsEnabled = ViewModel.Collections.Count > 0;
            TagsNavigation.IsEnabled = ViewModel.Tags.Count > 0;
            Details.RefreshCatalogs();
            Batch.RefreshCatalogs();
            Catalog.RefreshCatalogs();
            if (!_settingsVisible && !_catalogVisible)
            {
                Navigation.SelectedItem = ViewModel.Filter.Scope switch
                {
                    LibraryScope.Starred => StarredNavigation,
                    LibraryScope.Recent => RecentNavigation,
                    LibraryScope.Trash => TrashNavigation,
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
        if (ViewModel.Details is { } detail && !await detail.CommitAsync())
        {
            // Do not restore the selector on a window that is closing.
            if (_stopping) return;
            _updatingNavigation = true;
            try { TagSelector.SelectedValue = ViewModel.Filter.Id; }
            finally { _updatingNavigation = false; }
            return;
        }
        if (_stopping) return;
        Gallery.SelectedItems.Clear();
        await ViewModel.NavigateAsync(new(LibraryScope.Tag, tag.Id, tag.Name));
    }

    private async void OnSort(object sender, RoutedEventArgs args)
    {
        if (_stopping || sender is not RadioMenuFlyoutItem item) return;
        if (ViewModel.Details is { } detail && !await detail.CommitAsync())
        {
            if (!_stopping) UpdateState();
            return;
        }
        if (_stopping) return;
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
            if (ViewModel.Details is { } detail && !await detail.CommitAsync()) return;
            cancellation.ThrowIfCancellationRequested();
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

    private async void OnSelectionChanged(object sender, SelectionChangedEventArgs args)
    {
        // During a replace rebuild the new detail view model is not bound yet, so a clear
        // emitted by OnRebuilt must not be turned into a load against a half-built page.
        if (_restoringSelection || _stopping || ViewModel.IsRestoring
            || ViewModel.Batch?.IsRunning == true
            || ViewModel.IsLoading && ViewModel.Batch?.Report is not null) return;
        var version = ++_selectionVersion;
        var selected = Gallery.SelectedItems.OfType<StickerCard>().ToArray();
        if (ViewModel.IsTrash)
        {
            // Selection in the trash only drives restore and purge; there is no
            // editable detail and no batch organizer for deleted stickers.
            ViewModel.SetSelectedCount(Gallery.SelectedItems.Count);
            Details.Visibility = Visibility.Collapsed;
            Batch.Visibility = Visibility.Collapsed;
            Inspector.IsPaneOpen = false;
            UpdateState();
            return;
        }
        if (ViewModel.Details is { } detail)
        {
            var accepted = await detail.SelectAsync(selected.Length == 1 ? selected[0].Id : null);
            if (_stopping || version != _selectionVersion) return;
            if (!accepted)
            {
                _restoringSelection = true;
                Gallery.SelectedItems.Clear();
                if (ViewModel.Cards.FirstOrDefault(card => card.Id == detail.CurrentId) is { } current) Gallery.SelectedItems.Add(current);
                _restoringSelection = false;
                ViewModel.SetSelectedCount(Gallery.SelectedItems.Count);
                Details.Visibility = Visibility.Visible;
                Batch.Visibility = Visibility.Collapsed;
                Inspector.IsPaneOpen = true;
                UpdateState();
                return;
            }
        }
        ViewModel.Batch?.SetSelection(selected);
        Details.Visibility = ToVisibility(selected.Length == 1);
        Batch.Visibility = ToVisibility(selected.Length > 1);
        Inspector.IsPaneOpen = selected.Length > 0;
        ViewModel.SetSelectedCount(Gallery.SelectedItems.Count);
        UpdateState();
    }
    private void OnSelectAll(object sender, RoutedEventArgs args) => Gallery.SelectAll();
    private void OnClearSelection(object sender, RoutedEventArgs args) => Gallery.SelectedItems.Clear();

    private void OnFocusSearch(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        if (_settingsVisible || _catalogVisible) return;
        Search.Focus(FocusState.Keyboard);
        args.Handled = true;
    }
    private bool EditingText => FocusManager.GetFocusedElement(Root.XamlRoot) is TextBox or RichEditBox or PasswordBox
        or TextBlock { IsTextSelectionEnabled: true } or RichTextBlock { IsTextSelectionEnabled: true };
    private bool CanDragOut => !_stopping && !_reordering && ViewModel.Output is not null && !ViewModel.IsTrash
        && ViewModel.Batch?.IsRunning != true && !ViewModel.IsLoading && Gallery.SelectedItems.Count <= 1;
    private async void OnCopyShortcut(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        if (_settingsVisible || _catalogVisible || EditingText || ViewModel.IsTrash || Gallery.SelectedItems.Count != 1
            || Gallery.SelectedItem is not StickerCard card || ViewModel.Output?.CanStart != true) return;
        args.Handled = true;
        await CopyCardAsync(card, StickerOutputKind.Image);
    }
    private async Task CopyCardAsync(StickerCard card, StickerOutputKind kind)
    {
        if (_stopping || ViewModel.Output?.CanStart != true) return;
        var target = new StickerOutputTarget(card.Id, card.Generation);
        if (ViewModel.Details is { } detail && detail.CurrentId == target.Id && !await detail.CommitAsync()) return;
        await ViewModel.Output.CopyAsync(target, kind);
    }
    private void OnSelectAllShortcut(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        if (_settingsVisible || _catalogVisible || EditingText) return;
        Gallery.SelectAll();
        args.Handled = true;
    }
    private void OnEscape(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        if (_composing || _settingsVisible || _catalogVisible || EditingText) return;
        if (_reordering) { _reordering = false; UpdateState(); args.Handled = true; return; }
        Gallery.SelectedItems.Clear();
        args.Handled = true;
    }
    private async void OnImportShortcut(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        if (!ImportButton.IsEnabled || _settingsVisible || _catalogVisible) return;
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
        else
        {
            _initialization = ViewModel.InitializeAsync(_root, _backupPicker, _archivePicker);
            await _initialization;
            if (_stopping) return;
            Settings.ShowIdentity(ViewModel.Identity);
        }
    }

    private async void OnClosing(AppWindow sender, AppWindowClosingEventArgs args)
    {
        if (_canClose) return;
        args.Cancel = true;
        if (_stopping) return;
        if (ViewModel.Details is { } detail && !await detail.CommitAsync()) return;
        if (_stopping) return;
        _stopping = true;
        Details.Shutdown();
        Batch.Shutdown();
        Catalog.Shutdown();
        TrashGroupsPanel.Shutdown();
        Settings.ShutdownSync();
        Settings.ShutdownStorage();
        Settings.ShutdownBackup();
        Root.IsHitTestVisible = false;
        _lifetime.Cancel();
        ViewModel.RequestStop();
        try
        {
            // The window is closing either way, so a faulted background task must
            // not skip the session release that follows it.
            var failure = await ShutdownRun.SequenceAsync(
                () => _initialization,
                () => _searching,
                () => _importing,
                () => Settings.PendingSave,
                () => ViewModel.DisposeAsync().AsTask());
            if (failure is not null) Trace.WriteLine($"Library shutdown: {failure}");
        }
        catch (Exception error) { Trace.WriteLine($"Library shutdown: {error}"); }
        finally
        {
            if (_scroll is not null) _scroll.ViewChanged -= OnViewChanged;
            Settings.AppearanceChanged -= OnAppearanceChanged;
            Strings.PropertyChanged -= OnLanguageChanged;
            ViewModel.PropertyChanged -= OnModelChanged;
            ViewModel.CatalogChanged -= OnCatalogChanged;
            ViewModel.ThumbnailInvalidated -= OnThumbnailInvalidated;
            ViewModel.ContentChanged -= OnContentChanged;
            DetachChildren();
            ViewModel.Rebuilt -= OnRebuilt;
            AttachSync(null);
            Activated -= OnActivated;
            _searchCancellation?.Dispose();
            _lifetime.Dispose();
            _canClose = true;
            Close();
        }
    }

    private void OnInspectorSizeChanged(object sender, SizeChangedEventArgs args)
    {
        Inspector.DisplayMode = args.NewSize.Width >= 880 ? SplitViewDisplayMode.Inline : SplitViewDisplayMode.Overlay;
        Inspector.OpenPaneLength = Math.Min(340, Math.Max(240, args.NewSize.Width - 32));
    }
    private void OnInspectorPaneClosed(SplitView sender, object args) => Details.SetActive(false);
    private async void OnInspectorClose(object? sender, EventArgs args)
    {
        if (ViewModel.Batch?.IsRunning == true) return;
        if (ViewModel.Details is { } detail && !await detail.CommitAsync()) return;
        Gallery.SelectedItems.Clear();
        Inspector.IsPaneOpen = false;
        Details.SetActive(false);
    }
    private void OnActivated(object sender, WindowActivatedEventArgs args)
    {
        _windowActive = args.WindowActivationState != WindowActivationState.Deactivated;
        ViewModel.Output?.SetForeground(_windowActive);
        Details.SetActive(_windowActive && Inspector.IsPaneOpen && !_settingsVisible && !_catalogVisible && Details.Visibility == Visibility.Visible);
    }
    private async void OnContentChanged(object? sender, string? id)
    {
        if (!_stopping && ViewModel.Details is { } detail && (id is null || id == detail.CurrentId)) await detail.RefreshAsync();
    }
    private void OnStickerDeleted(object? sender, EventArgs args)
    {
        Gallery.SelectedItems.Clear();
        _undoAvailable = true;
        UndoButton.Visibility = Visibility.Visible;
        OperationBar.Severity = InfoBarSeverity.Success;
        OperationBar.Message = Strings["sticker_deleted"];
        OperationBar.IsOpen = true;
    }
    private async void OnUndoDelete(object sender, RoutedEventArgs args)
    {
        if (ViewModel.Details is not { } detail) return;
        if (await detail.UndoDeleteAsync())
        {
            _undoAvailable = false;
            OperationBar.IsOpen = false;
            UpdateState();
        }
        else
        {
            OperationBar.Severity = InfoBarSeverity.Error;
            OperationBar.Message = detail.ErrorText;
            OperationBar.IsOpen = true;
        }
    }
    private async void OnRestoreSelected(object sender, RoutedEventArgs args)
    {
        var cards = Gallery.SelectedItems.OfType<StickerCard>().ToArray();
        if (cards.Length > 0) await RunRestoreAsync(cards);
    }
    private async void OnPurgeSelected(object sender, RoutedEventArgs args)
    {
        var cards = Gallery.SelectedItems.OfType<StickerCard>().ToArray();
        if (cards.Length > 0) await RunPurgeAsync(cards);
    }
    private async Task RunRestoreAsync(IReadOnlyList<StickerCard> cards)
    {
        if (_stopping || ViewModel.IsTrashing || cards.Count == 0) return;
        ReportTrashOutcome(await ViewModel.RestoreAsync(cards), cards, restored: true);
    }
    private async Task RunPurgeAsync(IReadOnlyList<StickerCard> cards)
    {
        if (_stopping || ViewModel.IsTrashing || cards.Count == 0 || !await ConfirmPurgeAsync(cards.Count)) return;
        ReportTrashOutcome(await ViewModel.PurgeAsync(cards), cards, restored: false);
    }
    /// <summary>
    /// Erasing is irreversible: the core releases the original and its derived
    /// files, so the confirmation states the count and defaults to cancelling.
    /// </summary>
    private async Task<bool> ConfirmPurgeAsync(int count)
    {
        var dialog = new ContentDialog
        {
            XamlRoot = Root.XamlRoot,
            Title = Strings["purge_title"],
            Content = Strings.Format("purge_hint", count),
            PrimaryButtonText = Strings["purge_title"],
            CloseButtonText = Strings["cancel"],
            DefaultButton = ContentDialogButton.Close
        };
        return await dialog.ShowAsync() == ContentDialogResult.Primary;
    }
    private void ReportTrashOutcome(TrashOutcome outcome, IReadOnlyList<StickerCard> cards, bool restored)
    {
        if (_stopping) return;
        // The refresh dropped the entries that were restored or purged; only the
        // failed ones are still listed. Drop just those acted cards from the
        // selection so a failed entry keeps its place and any unrelated pick
        // survives a single-item context-menu action.
        var acted = cards.Select(card => card.Id).ToHashSet(StringComparer.Ordinal);
        var present = ViewModel.Cards.Select(card => card.Id).ToHashSet(StringComparer.Ordinal);
        var gone = Gallery.SelectedItems.OfType<StickerCard>()
            .Where(card => acted.Contains(card.Id) && !present.Contains(card.Id)).ToArray();
        _restoringSelection = true;
        try
        {
            foreach (var card in gone) Gallery.SelectedItems.Remove(card);
        }
        finally { _restoringSelection = false; }
        ViewModel.SetSelectedCount(Gallery.SelectedItems.Count);
        _undoAvailable = false;
        if (outcome.AnySucceeded)
        {
            OperationBar.Severity = outcome.AnyFailed ? InfoBarSeverity.Warning : InfoBarSeverity.Success;
            OperationBar.Message = restored
                ? Strings["restored"]
                : Strings.Format("purge_result", outcome.Succeeded, outcome.Failed);
            OperationBar.IsOpen = true;
        }
        else OperationBar.IsOpen = false;
        UpdateState();
    }
    private async void OnGalleryDoubleTapped(object sender, DoubleTappedRoutedEventArgs args)
    {
        if (ViewModel.IsTrash) return;
        if (Gallery.SelectedItems.Count == 1 && Gallery.SelectedItem is StickerCard card && ViewModel.Details is { } detail
            && await detail.SelectAsync(card.Id) && detail.CurrentId == card.Id) await Details.ShowPreviewAsync();
    }
    private void OnGalleryItemClick(object sender, ItemClickEventArgs args)
    {
        if (ViewModel.IsTrash) return;
        if (Gallery.SelectedItems.Count == 1 && args.ClickedItem is StickerCard card && card.Id == ViewModel.Details?.CurrentId)
        {
            Inspector.IsPaneOpen = true;
            UpdateState();
        }
    }
    private async void OnGalleryKeyDown(object sender, KeyRoutedEventArgs args)
    {
        if (ViewModel.IsTrash || args.Key != global::Windows.System.VirtualKey.Enter || Gallery.SelectedItems.Count != 1
            || Gallery.SelectedItem is not StickerCard card || ViewModel.Details is not { } detail) return;
        args.Handled = true;
        if (await detail.SelectAsync(card.Id) && detail.CurrentId == card.Id) await Details.ShowPreviewAsync();
    }
    private async void OnGalleryReordered(ListViewBase sender, DragItemsCompletedEventArgs args)
    {
        if (_galleryDragPreparation is { } preparation)
        {
            _galleryDragPreparation = null;
            await preparation;
            if (ViewModel.Output is { } output)
                await output.CompleteDragAsync(args.DropResult == DataPackageOperation.Copy);
            return;
        }
        if (!_reordering || args.DropResult == global::Windows.ApplicationModel.DataTransfer.DataPackageOperation.None
            || args.Items.Count != 1 || args.Items[0] is not StickerCard card || ViewModel.Catalog is not { } catalog) return;
        var collection = ViewModel.Collections.FirstOrDefault(item => item.Id == ViewModel.Filter.Id);
        if (collection is null) return;
        var index = ViewModel.Cards.IndexOf(card);
        if (index < 0) return;
        await catalog.MoveStickerAsync(collection, card, index + 1 < ViewModel.Cards.Count ? ViewModel.Cards[index + 1].Id : null);
        await ViewModel.RetryAsync();
    }
    private void OnGalleryDragStarting(object sender, DragItemsStartingEventArgs args)
    {
        args.Cancel = true;
        if (args.Items.Count != 1 || args.Items[0] is not StickerCard card) return;
        if (_reordering) { args.Cancel = !Gallery.CanReorderItems; return; }
        if (!CanDragOut || ViewModel.Output is not { CanStart: true } output) return;
        var target = new StickerOutputTarget(card.Id, card.Generation);
        var package = new DataPackage();
        var dispatcher = DispatcherQueue;
        args.Data.RequestedOperation = DataPackageOperation.Copy;
        // ListViewBase owns the pointer gesture. Its event has no GetDeferral;
        // the data provider keeps asynchronous file preparation out of selection.
        var preparation = output.PrepareDragAsync(target, package);
        _galleryDragPreparation = preparation;
        args.Data.SetDataProvider(StandardDataFormats.StorageItems, request =>
        {
            var deferral = request.GetDeferral();
            if (!dispatcher.TryEnqueue(async () =>
            {
                try
                {
                    if (await preparation)
                        request.SetData(await package.GetView().GetStorageItemsAsync());
                }
                catch (Exception error) { Trace.WriteLine($"Provide dragged file: {error}"); }
                finally { deferral.Complete(); }
            })) deferral.Complete();
        });
        args.Cancel = false;
    }
    private async void OnReorder(object sender, RoutedEventArgs args)
    {
        if (_reordering) { _reordering = false; UpdateState(); return; }
        if (!ReorderMenu.IsEnabled || ViewModel.Filter.Id is not { } collectionId) return;
        if (ViewModel.Details is { } detail && !await detail.CommitAsync()) { UpdateState(); return; }
        await ViewModel.SortAsync(StickerSort.CollectionOrder);
        if (ViewModel.Filter.Id == collectionId && !ViewModel.HasMore && ViewModel.ErrorKey is null)
        {
            _reorderCollection = collectionId;
            _reordering = true;
        }
        UpdateState();
    }
    private void OnReorderDone(object sender, RoutedEventArgs args) { _reordering = false; UpdateState(); }

    private async void OnBatchCompleted(object? sender, EventArgs args)
    {
        if (_stopping || ViewModel.Batch?.Report is not { } report) return;
        var remaining = report.Items.Where(item => item.Outcome is BatchOutcome.Failed or BatchOutcome.Pending)
            .Select(item => item.Id).ToHashSet();
        _restoringSelection = true;
        try
        {
            await ViewModel.RetryAsync();
            if (_stopping) return;
            Gallery.SelectedItems.Clear();
            foreach (var card in ViewModel.Cards.Where(card => remaining.Contains(card.Id))) Gallery.SelectedItems.Add(card);
            ViewModel.Batch?.RetainRemaining(Gallery.SelectedItems.OfType<StickerCard>());
            ViewModel.SetSelectedCount(Gallery.SelectedItems.Count);
            Details.Visibility = Visibility.Collapsed;
            Batch.Visibility = Visibility.Visible;
            Inspector.IsPaneOpen = true;
        }
        finally { _restoringSelection = false; }
    }

    private void OnGalleryRightTapped(object sender, RightTappedRoutedEventArgs args)
    {
        if (_stopping) return;
        DependencyObject? element = args.OriginalSource as DependencyObject;
        while (element is not null && element is not GridViewItem) element = VisualTreeHelper.GetParent(element);
        if (element is not GridViewItem { Content: StickerCard card } container) return;
        if (ViewModel.IsTrash)
        {
            var trashMenu = new MenuFlyout();
            var restore = new MenuFlyoutItem { Text = Strings["restore"], Icon = new SymbolIcon(Symbol.Undo) };
            restore.Click += async (_, _) => await RunRestoreAsync([card]);
            var purge = new MenuFlyoutItem { Text = Strings["purge_title"], Icon = new SymbolIcon(Symbol.Delete) };
            purge.Click += async (_, _) => await RunPurgeAsync([card]);
            trashMenu.Items.Add(restore);
            trashMenu.Items.Add(purge);
            args.Handled = true;
            trashMenu.ShowAt(container);
            return;
        }
        if (ViewModel.Output is not { } output) return;
        var menu = new MenuFlyout();
        var copy = new MenuFlyoutItem { Text = Strings["copy_image"], Icon = new SymbolIcon(Symbol.Copy), IsEnabled = output.CanStart };
        copy.Click += async (_, _) => await CopyCardAsync(card, StickerOutputKind.Image);
        menu.Items.Add(copy);
        var formats = new MenuFlyoutSubItem { Text = Strings["export_choose"], Icon = new SymbolIcon(Symbol.Copy), IsEnabled = output.CanStart };
        var original = new MenuFlyoutItem { Text = Strings["export_original"], IsEnabled = output.CanStart };
        original.Click += async (_, _) => await CopyCardAsync(card, StickerOutputKind.OriginalFile);
        formats.Items.Add(original);
        if (card.Animated)
        {
            var frame = new MenuFlyoutItem { Text = Strings.Format("share_first_frame_as", Strings["export_png"]), IsEnabled = output.CanStart };
            frame.Click += async (_, _) => await CopyCardAsync(card, StickerOutputKind.FirstFrame);
            formats.Items.Add(frame);
        }
        else
        {
            var png = new MenuFlyoutItem { Text = Strings["export_png"], IsEnabled = output.CanStart };
            png.Click += async (_, _) => await CopyCardAsync(card, StickerOutputKind.Image);
            formats.Items.Add(png);
        }
        menu.Items.Add(formats);
        args.Handled = true;
        var collection = ViewModel.Collections.FirstOrDefault(item => item.Id == ViewModel.Filter.Id);
        if (!Gallery.CanReorderItems || collection is null || ViewModel.Catalog is not { } catalog) { menu.ShowAt(container); return; }
        menu.Items.Add(new MenuFlyoutSeparator());
        var index = ViewModel.Cards.IndexOf(card);
        var up = new MenuFlyoutItem { Text = Strings["move_up"], IsEnabled = index > 0 };
        var down = new MenuFlyoutItem { Text = Strings["move_down"], IsEnabled = index >= 0 && index < ViewModel.Cards.Count - 1 };
        up.Click += async (_, _) =>
        {
            var current = ViewModel.Cards.IndexOf(card);
            if (current <= 0) return;
            await catalog.MoveStickerAsync(collection, card, ViewModel.Cards[current - 1].Id);
            await ViewModel.RetryAsync();
        };
        down.Click += async (_, _) =>
        {
            var current = ViewModel.Cards.IndexOf(card);
            if (current < 0 || current >= ViewModel.Cards.Count - 1) return;
            await catalog.MoveStickerAsync(collection, card, current + 2 < ViewModel.Cards.Count ? ViewModel.Cards[current + 2].Id : null);
            await ViewModel.RetryAsync();
        };
        menu.Items.Add(up);
        menu.Items.Add(down);
        menu.ShowAt(container);
        args.Handled = true;
    }
}
