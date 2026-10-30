using System.ComponentModel;
using System.Diagnostics;
using MemeDock.Bridge;
using MemeDock.Windows.Localization;
using MemeDock.Windows.Models;
using MemeDock.Windows.Services;
using MemeDock.Windows.ViewModels;
using Microsoft.UI;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Animation;
using Microsoft.Windows.Storage.Pickers;
using Windows.ApplicationModel.DataTransfer;

namespace MemeDock.Windows;

public sealed partial class MainWindow : Window
{
    private const string HeroOpenKey = "MemeDock.HeroOpen";
    private const string HeroCloseKey = "MemeDock.HeroClose";
    private const string TileImageName = "TileImage";
    // Target slot width for a tile, and the caption row added to it so the picture stays square
    // (8 epx padding on each side, a 16 epx caption and 6 epx between them).
    private const double TileTarget = 136;
    private const double TileCaption = 22;
    private readonly Brush _transparentBrush = new SolidColorBrush(Colors.Transparent);

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
    private TextBox? _searchBox;
    private ContentDialog? _dialog;
    private bool _initialized;
    private bool _updatingNavigation;
    private bool _composing;
    private bool _settingsVisible;
    private bool _childrenConfigured;
    private bool _restoringSelection;
    private long _selectionVersion;
    private bool _selecting;
    private bool _stopping;
    private bool _canClose;
    private bool _windowActive = true;
    private bool _reordering;
    private bool _outputWasBusy;
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
        try
        {
            // The title bar hosts the search box, so it uses the tall caption buttons
            // the title bar guidance asks for with interactive content.
            AppWindow.TitleBar.PreferredHeightOption = TitleBarHeightOption.Tall;
            if (AppWindow.Presenter is OverlappedPresenter presenter)
            {
                presenter.PreferredMinimumWidth = 720;
                presenter.PreferredMinimumHeight = 520;
            }
        }
        catch (Exception error) { Trace.WriteLine($"The title bar height could not be applied: {error.GetType().Name}"); }
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
        // Page changes and the inspector's two jobs animate with the Fluent entrance curve.
        Motion.UseEntrance(Inspector);
        Motion.UseEntrance(Settings);
        Motion.UseCrossFade(Details);
        Motion.UseCrossFade(Batch);
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
        Motion.ConfigureConnectedAnimations(Root);
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

    private void OnNavigationLoaded(object sender, RoutedEventArgs args) => LocalizeSettingsItem();

    /// <summary>
    /// The built-in settings entry is labelled in the system language; the app has its
    /// own language choice, so the label is replaced whenever that changes.
    /// </summary>
    private void LocalizeSettingsItem()
    {
        if (Navigation.SettingsItem is NavigationViewItem settings)
        {
            settings.Content = Strings["desktop_settings"];
            AutomationProperties.SetName(settings, Strings["desktop_settings"]);
        }
    }

    private void ApplyLabels()
    {
        AllNavigation.Content = Strings["filter_all"];
        StarredNavigation.Content = Strings["favorites_only"];
        RecentNavigation.Content = Strings["filter_recent"];
        CollectionsHeader.Content = Strings["tab_collections"];
        TagsHeader.Content = Strings["tags_title"];
        NewCollectionNavigation.Content = Strings["new_collection"];
        NewTagNavigation.Content = Strings["new_tag"];
        TrashNavigation.Content = Strings["trash"];
        LocalizeSettingsItem();
        ImportButton.Label = Strings["library_add"];
        ToolTipService.SetToolTip(ImportButton, Strings["library_add"]);
        Search.PlaceholderText = Strings["library_search"];
        AutomationProperties.SetName(Search, Strings["library_search"]);
        AutomationProperties.SetName(Gallery, Strings["tab_stickers"]);
        ToolTipService.SetToolTip(SortButton, Strings["desktop_sort"]);
        AutomationProperties.SetName(SortButton, Strings["desktop_sort"]);
        SortRecent.Text = Strings["desktop_sort_recent"];
        SortUsed.Text = Strings["filter_recent"];
        SortCollection.Text = Strings["desktop_sort_collection"];
        ReorderMenu.Text = Strings["reorder"];
        ReorderDone.Label = Strings["done"];
        SelectToggle.Label = Strings["select_items"];
        RetryButton.Content = Strings["retry"];
        FailureRetry.Content = Strings["retry"];
        FailureTitle.Text = Strings["library_load_title"];
        RestoreButton.Label = Strings["restore"];
        PurgeButton.Label = Strings["purge_title"];
        Settings.Relocalize();
        Details.Relocalize();
        Batch.Relocalize();
        TrashGroupsPanel.Relocalize();
        ViewModel.Relocalize();
        RebuildCatalogItems();
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
            TrashGroupsPanel.Configure(trashGroups, Strings);
            Settings.ConfigureSync(ViewModel.Sync, Strings);
            Settings.ConfigureStorage(storage);
            Settings.ConfigureBackup(backup);
            details.Deleted += OnStickerDeleted;
            batch.PropertyChanged += OnModelChanged;
            batch.Completed += OnBatchCompleted;
            catalog.PropertyChanged += OnModelChanged;
            trashGroups.PropertyChanged += OnModelChanged;
            output.PropertyChanged += OnModelChanged;
            output.SetForeground(_windowActive);
            _outputWasBusy = output.IsBusy;
        }
        // The coordinator outlives this window only until shutdown, so the
        // handler is attached once per instance and dropped in the finally
        // block of OnClosing.
        AttachSync(ViewModel.Sync);
        ReportOutput();
        var batchRunning = ViewModel.Batch?.IsRunning == true;
        // A restore either holds the session open for a merge or is about to close it
        // for a replace; either way nothing else may start work on it meanwhile.
        var restoring = ViewModel.IsRestoring;
        var blocked = batchRunning || restoring;
        var trash = ViewModel.IsTrash;
        var cards = ViewModel.Cards.Count;
        var searching = ViewModel.SearchText.Length > 0;
        var trashGroupsVisible = trash && ViewModel.TrashGroups?.HasVisibleContent == true;
        var catalogBusy = ViewModel.Catalog?.IsBusy == true;

        Gallery.IsEnabled = !blocked && !ViewModel.IsTrashing;
        foreach (var item in Navigation.MenuItems.OfType<NavigationViewItem>()) item.IsEnabled = !blocked;
        TrashNavigation.IsEnabled = !blocked;
        NewCollectionNavigation.IsEnabled = NewTagNavigation.IsEnabled = !blocked && !catalogBusy && ViewModel.IsReady;
        if (Navigation.SettingsItem is NavigationViewItem settingsItem) settingsItem.IsEnabled = !blocked;

        ImportButton.Visibility = ToVisibility(!trash);
        ImportButton.IsEnabled = ViewModel.IsReady && !ViewModel.IsImporting && !_stopping && !blocked;
        Search.IsEnabled = ViewModel.IsReady && !_stopping && !blocked;
        SortButton.IsEnabled = ViewModel.IsReady && ViewModel.Filter.Scope != LibraryScope.Recent && !blocked;
        SortButton.Label = ViewModel.Filter.Scope == LibraryScope.Recent ? Strings["filter_recent"] : ViewModel.Sort switch
        {
            StickerSort.LastUsed => Strings["filter_recent"],
            StickerSort.CollectionOrder => Strings["desktop_sort_collection"],
            _ => Strings["desktop_sort_recent"]
        };
        SelectToggle.IsEnabled = ViewModel.IsReady && !blocked && !_reordering;
        ToolTipService.SetToolTip(SelectToggle, Strings[_selecting ? "finish_selection" : "select_items"]);

        var selected = Gallery.SelectedItems.Count;
        CountLabel.Text = selected > 0 ? ViewModel.SelectionText : cards > 0 ? ViewModel.CountText : "";
        CountLabel.Visibility = ToVisibility(CountLabel.Text.Length > 0);

        // A failure owns the whole area when there is nothing to show; with stickers on
        // screen it stays an inline bar, so the loaded rows remain usable.
        var failed = ViewModel.ErrorKey is not null;
        FailureState.Visibility = ToVisibility(failed && cards == 0 && !ViewModel.IsLoading);
        FailureMessage.Text = ViewModel.ErrorText;
        ErrorBar.IsOpen = failed && cards > 0;
        ErrorBar.Message = ViewModel.ErrorText;
        CatalogErrorBar.IsOpen = ViewModel.CatalogErrorKey is not null;
        CatalogErrorBar.Message = ViewModel.CatalogErrorText;
        Busy.IsActive = !ViewModel.IsReady && !failed || ViewModel.IsLoading && cards == 0;
        Busy.Visibility = ToVisibility(Busy.IsActive);

        ImportStrip.Visibility = ToVisibility(ViewModel.IsImporting);
        ImportProgress.Value = ViewModel.ImportTotal > 0 ? (double)ViewModel.ImportCompleted / ViewModel.ImportTotal : 0;
        ImportStatus.Text = ViewModel.IsImporting ? ViewModel.ImportText : "";

        TrashGroupsPanel.Visibility = ToVisibility(trashGroupsVisible);
        var empty = ViewModel.IsReady && !ViewModel.IsLoading && cards == 0 && !failed && (!trashGroupsVisible || searching);
        EmptyState.Visibility = ToVisibility(empty);
        UpdateEmptyState(searching);

        // Restore and purge replace adding in the trash, where the only meaningful
        // actions are on already deleted stickers.
        RestoreButton.Visibility = PurgeButton.Visibility = ToVisibility(trash);
        RestoreButton.IsEnabled = PurgeButton.IsEnabled = selected > 0 && !ViewModel.IsTrashing && !blocked;

        SortCollection.Visibility = ToVisibility(ViewModel.Filter.AllowsCollectionOrder);
        SortRecent.IsChecked = ViewModel.Sort == StickerSort.Recent;
        SortUsed.IsChecked = ViewModel.Sort == StickerSort.LastUsed;
        SortCollection.IsChecked = ViewModel.Sort == StickerSort.CollectionOrder;
        if (!ViewModel.Filter.AllowsCollectionOrder || ViewModel.Filter.Id != _reorderCollection
            || searching || ViewModel.Sort != StickerSort.CollectionOrder) _reordering = false;
        var reorder = ViewModel.Filter.AllowsCollectionOrder && !ViewModel.HasMore && !ViewModel.IsLoading
            && !searching && !blocked && ViewModel.Output?.IsBusy != true;
        ReorderMenu.IsEnabled = reorder;
        ReorderMenu.Visibility = ReorderSeparator.Visibility = ToVisibility(ViewModel.Filter.AllowsCollectionOrder);
        ReorderMenu.IsChecked = _reordering;
        ReorderDone.Visibility = ToVisibility(_reordering);
        Gallery.CanDragItems = _reordering && reorder || CanDragOut;
        Gallery.CanReorderItems = Gallery.AllowDrop = _reordering && reorder;
        Batch.SetLoadedOnly(ViewModel.HasMore);
        UpdateSyncStatus();
        Details.SetActive(_windowActive && Inspector.IsPaneOpen && !_settingsVisible && Details.Visibility == Visibility.Visible);
    }

    /// <summary>
    /// An empty result is the search's answer when a term was typed, and the
    /// section's own wording otherwise; adding is offered where a new sticker would appear.
    /// </summary>
    private void UpdateEmptyState(bool searching)
    {
        var scope = ViewModel.Filter.Scope;
        EmptyIcon.Glyph = searching ? "\uE721" : scope switch
        {
            LibraryScope.Starred => "\uE734",
            LibraryScope.Recent => "\uE81C",
            LibraryScope.Trash => "\uE74D",
            LibraryScope.Collection => "\uE8B7",
            LibraryScope.Tag => "\uE8EC",
            _ => "\uE80A"
        };
        EmptyHint.Text = Strings[searching ? "library_no_matches_hint" : scope switch
        {
            LibraryScope.Starred => "desktop_starred_empty_hint",
            LibraryScope.Trash => "trash_empty_hint",
            LibraryScope.Collection => "desktop_collection_empty_hint",
            LibraryScope.Tag => "desktop_tag_empty_hint",
            _ => "desktop_library_empty_hint"
        }];
        var canImport = !searching && scope is LibraryScope.All or LibraryScope.Collection;
        EmptyAction.Content = Strings[searching ? "clear_search" : "library_add"];
        EmptyAction.Visibility = ToVisibility(searching || canImport);
        EmptyAction.IsEnabled = searching || ImportButton.IsEnabled;
    }

    /// <summary>
    /// Copy and drag report through the output model; a finished run is announced once,
    /// when the model leaves its busy state.
    /// </summary>
    private void ReportOutput()
    {
        if (ViewModel.Output is not { } output) return;
        var busy = output.IsBusy;
        if (_outputWasBusy && !busy && !_stopping)
        {
            if (output.ErrorKey is { } error) Notice.Show(Strings[error], InfoBarSeverity.Error);
            else if (output.MessageKey == "image_copied") Notice.Show(Strings["image_copied"], InfoBarSeverity.Success);
        }
        _outputWasBusy = busy;
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
        _reordering = false;
        _reorderCollection = null;
        _selectionVersion++;
        Notice.Hide();
        SetSelecting(false, clear: false);
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
        if (ViewModel.Catalog is { } catalog) catalog.PropertyChanged -= OnModelChanged;
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
        Navigation.SelectedItem = Navigation.SettingsItem;
    }

    private void OnPaneToggle(TitleBar sender, object args) => Navigation.IsPaneOpen = !Navigation.IsPaneOpen;

    // ---- Navigation ----

    private async void OnNavigationChanged(NavigationView sender, NavigationViewSelectionChangedEventArgs args)
    {
        if (_updatingNavigation || _stopping || args.SelectedItemContainer is not NavigationViewItem item) return;
        if (ViewModel.Batch?.IsRunning == true || ViewModel.IsRestoring)
        {
            SelectCurrentSection();
            return;
        }
        if (ViewModel.Details is { } detail)
        {
            if (!await detail.CommitAsync())
            {
                SelectCurrentSection();
                return;
            }
            // Committing is asynchronous: the window may have started closing
            // while the editor saved, and a closing window must not navigate.
            if (_stopping) return;
        }
        _settingsVisible = args.IsSettingsSelected || ReferenceEquals(item, Navigation.SettingsItem);
        Inspector.Visibility = ToVisibility(!_settingsVisible);
        Settings.Visibility = ToVisibility(_settingsVisible);
        Details.SetActive(!_settingsVisible && Inspector.IsPaneOpen);
        // Disk figures age while the user works elsewhere; re-read on every visit.
        if (_settingsVisible)
        {
            _ = Settings.RefreshStorageAsync();
            return;
        }
        if (item == TrashNavigation && ViewModel.TrashGroups is { } trashGroups)
        {
            await trashGroups.RefreshAsync();
            // The read is asynchronous too; after it the panel may be gone.
            if (_stopping) return;
        }
        LibraryFilter? filter = item.Tag switch
        {
            "all" => LibraryFilter.All,
            "starred" => new(LibraryScope.Starred),
            "recent" => new(LibraryScope.Recent),
            "trash" => new(LibraryScope.Trash),
            CollectionMetadata collection => new(LibraryScope.Collection, collection.Id, collection.Name),
            TagMetadata tag => new(LibraryScope.Tag, tag.Id, tag.Name),
            _ => null
        };
        if (filter is null) return;
        if (filter.IsTrash)
        {
            // Deleted stickers are read-only here, so the inspector stays closed
            // instead of opening editors that the core would reject.
            SetSelecting(false, clear: false);
            Inspector.IsPaneOpen = false;
            Details.SetActive(false);
        }
        Gallery.SelectedItems.Clear();
        await ViewModel.NavigateAsync(filter);
        // Navigating is asynchronous: touching the gallery or its scroll view
        // after the window closed would reach released controls.
        if (_stopping) return;
        _scroll?.ChangeView(null, 0, null, disableAnimation: true);
    }

    private async void OnNavigationInvoked(NavigationView sender, NavigationViewItemInvokedEventArgs args)
    {
        if (_stopping) return;
        if (ReferenceEquals(args.InvokedItemContainer, NewCollectionNavigation)) await NameCatalogAsync(CatalogKind.Collection, null);
        else if (ReferenceEquals(args.InvokedItemContainer, NewTagNavigation)) await NameCatalogAsync(CatalogKind.Tag, null);
    }

    private void OnCatalogChanged(object? sender, EventArgs args)
    {
        // A catalog event is not a reason to rebuild navigation on a closing
        // window; the controls are being detached.
        if (_stopping) return;
        RebuildCatalogItems();
        Details.RefreshCatalogs();
        Batch.RefreshCatalogs();
    }

    /// <summary>
    /// Collections and tags are sections of the sidebar, each between its header and
    /// its "new" entry. The rows are replaced as a whole because the catalog is small
    /// and every row's menu reads the latest order when it opens.
    /// </summary>
    private void RebuildCatalogItems()
    {
        _updatingNavigation = true;
        try
        {
            ReplaceSection(CollectionsHeader, NewCollectionNavigation,
                ViewModel.Collections.Select(collection => CatalogItem(collection.Name, collection, "\uE8B7",
                    CatalogTarget.From(collection), collection)));
            ReplaceSection(TagsHeader, NewTagNavigation,
                ViewModel.Tags.Select(tag => CatalogItem(tag.Name, tag, "\uE8EC", CatalogTarget.From(tag), null)));
            SelectCurrentSection();
        }
        finally { _updatingNavigation = false; }
    }

    private void ReplaceSection(NavigationViewItemBase header, NavigationViewItem end, IEnumerable<NavigationViewItem> items)
    {
        var menu = Navigation.MenuItems;
        var start = menu.IndexOf(header) + 1;
        for (var index = menu.IndexOf(end) - 1; index >= start; index--) menu.RemoveAt(index);
        foreach (var item in items) menu.Insert(start++, item);
    }

    private NavigationViewItem CatalogItem(string name, object tag, string glyph, CatalogTarget target, CollectionMetadata? collection)
    {
        var item = new NavigationViewItem { Content = name, Tag = tag, Icon = new FontIcon { Glyph = glyph } };
        ToolTipService.SetToolTip(item, name);
        var menu = new MenuFlyout();
        // Filled when it opens, so the move entries see the current order and the
        // labels follow a language change made after the sidebar was built.
        menu.Opening += (_, _) => FillCatalogMenu(menu, target, collection);
        item.ContextFlyout = menu;
        return item;
    }

    private void FillCatalogMenu(MenuFlyout menu, CatalogTarget target, CollectionMetadata? collection)
    {
        menu.Items.Clear();
        var available = ViewModel.Catalog is { IsBusy: false } && ViewModel.Batch?.IsRunning != true && !ViewModel.IsRestoring;
        var rename = new MenuFlyoutItem { Text = Strings["rename"], Icon = new FontIcon { Glyph = "\uE8AC" }, IsEnabled = available };
        rename.Click += async (_, _) => await NameCatalogAsync(target.Kind, target);
        menu.Items.Add(rename);
        if (collection is not null)
        {
            var index = IndexOfCollection(collection.Id);
            var up = new MenuFlyoutItem { Text = Strings["move_up"], Icon = new FontIcon { Glyph = "\uE70E" }, IsEnabled = available && index > 0 };
            var down = new MenuFlyoutItem
            {
                Text = Strings["move_down"], Icon = new FontIcon { Glyph = "\uE70D" },
                IsEnabled = available && index >= 0 && index < ViewModel.Collections.Count - 1
            };
            up.Click += async (_, _) => await MoveCollectionAsync(collection.Id, up: true);
            down.Click += async (_, _) => await MoveCollectionAsync(collection.Id, up: false);
            menu.Items.Add(up);
            menu.Items.Add(down);
        }
        menu.Items.Add(new MenuFlyoutSeparator());
        var delete = new MenuFlyoutItem { Text = Strings["delete"], Icon = new FontIcon { Glyph = "\uE74D" }, IsEnabled = available };
        delete.Click += async (_, _) => await DeleteCatalogAsync(target);
        menu.Items.Add(delete);
    }

    private int IndexOfCollection(string id)
    {
        for (var index = 0; index < ViewModel.Collections.Count; index++)
            if (ViewModel.Collections[index].Id == id) return index;
        return -1;
    }

    private async Task MoveCollectionAsync(string id, bool up)
    {
        if (ViewModel.Catalog is not { } catalog) return;
        // The catalog may have been reloaded since the menu opened; act on the live entry.
        var index = IndexOfCollection(id);
        if (index < 0) return;
        var collections = ViewModel.Collections;
        string? before;
        if (up)
        {
            if (index == 0) return;
            before = collections[index - 1].Id;
        }
        else
        {
            if (index >= collections.Count - 1) return;
            before = index + 2 < collections.Count ? collections[index + 2].Id : null;
        }
        if (!await catalog.MoveAsync(collections[index], before)) ReportCatalogFailure(catalog);
    }

    private async Task NameCatalogAsync(CatalogKind kind, CatalogTarget? target)
    {
        if (_dialog is not null || ViewModel.Catalog is not { IsBusy: false } catalog || Root.XamlRoot is null) return;
        var input = new TextBox { Header = Strings["name"], Text = target?.Name ?? "" };
        var error = new InfoBar { IsClosable = false, Severity = InfoBarSeverity.Error };
        var panel = new StackPanel { Spacing = 12 };
        panel.Children.Add(input);
        panel.Children.Add(error);
        var dialog = new ContentDialog
        {
            XamlRoot = Root.XamlRoot,
            Title = Strings[target is null ? kind == CatalogKind.Collection ? "new_collection" : "new_tag" : "rename"],
            Content = panel, PrimaryButtonText = Strings["done"], CloseButtonText = Strings["cancel"],
            DefaultButton = ContentDialogButton.Primary, IsPrimaryButtonEnabled = !string.IsNullOrWhiteSpace(input.Text)
        };
        input.TextChanged += (_, _) => dialog.IsPrimaryButtonEnabled = !string.IsNullOrWhiteSpace(input.Text) && !catalog.IsBusy;
        dialog.Opened += (_, _) => { input.Focus(FocusState.Programmatic); input.SelectAll(); };
        dialog.Closing += (_, args) => { if (catalog.IsBusy) args.Cancel = true; };
        dialog.PrimaryButtonClick += async (_, args) =>
        {
            var deferral = args.GetDeferral();
            dialog.IsPrimaryButtonEnabled = input.IsEnabled = false;
            try
            {
                var success = target is null ? await catalog.CreateAsync(kind, input.Text) : await catalog.RenameAsync(target, input.Text);
                args.Cancel = !success;
                if (!success) { error.Message = catalog.ErrorText; error.IsOpen = true; }
            }
            finally
            {
                input.IsEnabled = true;
                dialog.IsPrimaryButtonEnabled = !string.IsNullOrWhiteSpace(input.Text);
                deferral.Complete();
            }
        };
        _dialog = dialog;
        try { await dialog.ShowAsync(); }
        finally { _dialog = null; }
    }

    /// <summary>
    /// Deleting moves the entry to the trash, so the notice offers the same undo the
    /// organize page used to; a collection's stickers are never deleted with it.
    /// </summary>
    private async Task DeleteCatalogAsync(CatalogTarget target)
    {
        if (_dialog is not null || ViewModel.Catalog is not { IsBusy: false } catalog || Root.XamlRoot is null) return;
        var dialog = new ContentDialog
        {
            XamlRoot = Root.XamlRoot, Title = Strings["delete"] + " · " + target.Name,
            Content = Strings[target.Kind == CatalogKind.Collection ? "desktop_collection_delete_hint" : "delete_hint"],
            PrimaryButtonText = Strings["delete"], CloseButtonText = Strings["cancel"], DefaultButton = ContentDialogButton.Close
        };
        _dialog = dialog;
        ContentDialogResult result;
        try { result = await dialog.ShowAsync(); }
        finally { _dialog = null; }
        if (result != ContentDialogResult.Primary || _stopping) return;
        if (!await catalog.DeleteAsync(target))
        {
            ReportCatalogFailure(catalog);
            return;
        }
        Notice.Show(catalog.MessageText, InfoBarSeverity.Success, Strings["restore"], async () =>
        {
            if (!await catalog.UndoAsync()) ReportCatalogFailure(catalog);
            else Notice.Show(catalog.MessageText, InfoBarSeverity.Success);
        });
    }

    private void ReportCatalogFailure(CatalogViewModel catalog)
    {
        if (!_stopping && catalog.ErrorKey is not null) Notice.Show(catalog.ErrorText, InfoBarSeverity.Error);
    }

    /// <summary>Points the sidebar at the section the library is showing.</summary>
    private void SelectCurrentSection()
    {
        if (_settingsVisible) return;
        var updating = _updatingNavigation;
        _updatingNavigation = true;
        try
        {
            var filter = ViewModel.Filter;
            Navigation.SelectedItem = filter.Scope switch
            {
                LibraryScope.Starred => StarredNavigation,
                LibraryScope.Recent => RecentNavigation,
                LibraryScope.Trash => TrashNavigation,
                LibraryScope.Collection => Navigation.MenuItems.OfType<NavigationViewItem>()
                    .FirstOrDefault(item => item.Tag is CollectionMetadata value && value.Id == filter.Id) ?? AllNavigation,
                LibraryScope.Tag => Navigation.MenuItems.OfType<NavigationViewItem>()
                    .FirstOrDefault(item => item.Tag is TagMetadata value && value.Id == filter.Id) ?? AllNavigation,
                _ => AllNavigation
            };
        }
        finally { _updatingNavigation = updating; }
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

    // ---- Search ----

    private void OnSearchLoaded(object sender, RoutedEventArgs args)
    {
        if (_searchBox is not null) return;
        // The suggest box raises no composition events of its own; its inner text box does,
        // and a search must not run on a half-composed IME string.
        _searchBox = FindDescendant<TextBox>(Search);
        if (_searchBox is null) return;
        _searchBox.TextCompositionStarted += OnCompositionStarted;
        _searchBox.TextCompositionEnded += OnCompositionEnded;
    }

    private void OnSearchChanged(AutoSuggestBox sender, AutoSuggestBoxTextChangedEventArgs args)
    {
        // Programmatic changes come from this window, which decides itself whether to search.
        if (args.Reason == AutoSuggestionBoxTextChangeReason.UserInput && !_composing) QueueSearch();
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

    private void ClearSearch()
    {
        if (Search.Text.Length == 0 && ViewModel.SearchText.Length == 0) return;
        Search.Text = "";
        QueueSearch();
    }

    private void QueueSearch()
    {
        _searchCancellation?.Cancel();
        _searchCancellation?.Dispose();
        _searchCancellation = null;
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

    private bool SearchFocused => _searchBox is not null && Root.XamlRoot is not null
        && ReferenceEquals(FocusManager.GetFocusedElement(Root.XamlRoot), _searchBox);

    // ---- Gallery ----

    private void OnGalleryLoaded(object sender, RoutedEventArgs args)
    {
        if (_scroll is not null) _scroll.ViewChanged -= OnViewChanged;
        _scroll = FindDescendant<ScrollViewer>(Gallery);
        if (_scroll is not null) _scroll.ViewChanged += OnViewChanged;
        LayoutTiles(Gallery.ActualWidth);
    }

    private void OnGallerySizeChanged(object sender, SizeChangedEventArgs args) => LayoutTiles(args.NewSize.Width);

    /// <summary>
    /// Columns come from the measured width and then share it evenly, so every row
    /// fills exactly instead of leaving a ragged gap on the right.
    /// </summary>
    private void LayoutTiles(double width)
    {
        if (Gallery.ItemsPanelRoot is not ItemsWrapGrid panel) return;
        var available = width - Gallery.Padding.Left - Gallery.Padding.Right - 1;
        if (available < TileTarget / 2) return;
        var columns = Math.Max(1, (int)(available / TileTarget));
        var slot = Math.Floor(available / columns);
        if (Math.Abs(panel.ItemWidth - slot) < 0.5) return;
        panel.ItemWidth = slot;
        panel.ItemHeight = slot + TileCaption;
    }

    private async void OnViewChanged(object? sender, ScrollViewerViewChangedEventArgs args)
    {
        if (_stopping || _scroll is null || ViewModel.IsLoading || !ViewModel.HasMore) return;
        if (_scroll.ScrollableHeight - _scroll.VerticalOffset < 700) await ViewModel.LoadNextAsync();
    }

    private static T? FindDescendant<T>(DependencyObject parent) where T : DependencyObject
    {
        if (parent is T match) return match;
        for (var index = 0; index < VisualTreeHelper.GetChildrenCount(parent); index++)
            if (FindDescendant<T>(VisualTreeHelper.GetChild(parent, index)) is { } found) return found;
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

    private async void OnEmptyAction(object sender, RoutedEventArgs args)
    {
        if (ViewModel.SearchText.Length > 0 || Search.Text.Length > 0) ClearSearch();
        else await StartImportAsync();
    }

    // ---- Selection and the inspector ----

    private void OnSelectionChanged(object sender, SelectionChangedEventArgs args) => _ = ApplySelectionAsync();

    /// <summary>
    /// One column, one job: a single sticker opens its detail, picking several (or
    /// selection mode) opens the batch organizer. Opening the detail from a closed
    /// column carries the tile's picture over to the preview.
    /// </summary>
    private async Task ApplySelectionAsync()
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
        var single = !_selecting && selected.Length == 1 ? selected[0] : null;
        if (single is not null)
        {
            Details.ShowPlaceholder(single.Id, single.Thumbnail);
            if (!Inspector.IsPaneOpen) await OpenDetailWithHeroAsync(single);
            if (_stopping || version != _selectionVersion) return;
        }
        if (ViewModel.Details is { } detail)
        {
            var accepted = await detail.SelectAsync(single?.Id);
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
        var batch = _selecting || selected.Length > 1;
        Details.Visibility = ToVisibility(!batch && single is not null);
        Batch.Visibility = ToVisibility(batch);
        Inspector.IsPaneOpen = batch || single is not null;
        ViewModel.SetSelectedCount(Gallery.SelectedItems.Count);
        UpdateState();
    }

    /// <summary>
    /// The tile's picture flies into the preview slot while the pane opens. The slot is
    /// already showing the thumbnail, so nothing has to be waited on in between.
    /// </summary>
    private async Task OpenDetailWithHeroAsync(StickerCard card)
    {
        ConnectedAnimation? hero = null;
        if (Motion.Enabled && Gallery.ContainerFromItem(card) is GridViewItem)
        {
            try { hero = Gallery.PrepareConnectedAnimation(HeroOpenKey, card, TileImageName); }
            catch (Exception error) { Trace.WriteLine($"Hero preparation failed: {error.GetType().Name}"); }
        }
        Batch.Visibility = Visibility.Collapsed;
        Details.Visibility = Visibility.Visible;
        Inspector.IsPaneOpen = true;
        if (hero is null) return;
        if (_stopping || !await Details.WaitForHeroTargetAsync() || !hero.TryStart(Details.HeroTarget)) hero.Cancel();
    }

    private void OnSelectAll(object? sender, EventArgs args) => Gallery.SelectAll();
    private void OnClearSelection(object? sender, EventArgs args) => Gallery.SelectedItems.Clear();

    private void OnToggleSelecting(object sender, RoutedEventArgs args) => SetSelecting(SelectToggle.IsChecked == true);

    /// <summary>
    /// Selection mode turns clicks into picks (with check boxes) and keeps the batch
    /// organizer open even before anything is picked; leaving it drops the picks.
    /// </summary>
    private void SetSelecting(bool selecting, bool clear = true)
    {
        _selecting = selecting;
        SelectToggle.IsChecked = selecting;
        Gallery.IsItemClickEnabled = !selecting;
        Gallery.SelectionMode = selecting ? ListViewSelectionMode.Multiple : ListViewSelectionMode.Extended;
        if (!clear) return;
        if (!selecting && Gallery.SelectedItems.Count > 0) Gallery.SelectedItems.Clear();
        else _ = ApplySelectionAsync();
    }

    private async void OnInspectorClose(object? sender, EventArgs args) => await CloseInspectorAsync();

    /// <summary>
    /// Closing commits the open editor first; the preview then flies back to its tile
    /// when that tile is on screen.
    /// </summary>
    private async Task CloseInspectorAsync()
    {
        if (ViewModel.Batch?.IsRunning == true) return;
        if (ViewModel.Details is { } detail && !await detail.CommitAsync()) return;
        if (_stopping) return;
        if (_selecting)
        {
            SetSelecting(false);
            Inspector.IsPaneOpen = false;
            return;
        }
        ConnectedAnimation? hero = null;
        var card = Gallery.SelectedItems.Count == 1 ? Gallery.SelectedItem as StickerCard : null;
        if (Motion.Enabled && card is not null && Details.Visibility == Visibility.Visible && Inspector.IsPaneOpen
            && Gallery.ContainerFromItem(card) is GridViewItem)
        {
            try
            {
                hero = ConnectedAnimationService.GetForCurrentView().PrepareToAnimate(HeroCloseKey, Details.HeroTarget);
                hero.Configuration = new DirectConnectedAnimationConfiguration();
            }
            catch (Exception error) { Trace.WriteLine($"Hero preparation failed: {error.GetType().Name}"); hero = null; }
        }
        Gallery.SelectedItems.Clear();
        Inspector.IsPaneOpen = false;
        Details.SetActive(false);
        if (hero is null || card is null) return;
        var started = false;
        try { started = await Gallery.TryStartConnectedAnimationAsync(hero, card, TileImageName); }
        catch (Exception error) { Trace.WriteLine($"Hero landing failed: {error.GetType().Name}"); }
        if (!started) hero.Cancel();
    }

    private void OnInspectorSizeChanged(object sender, SizeChangedEventArgs args)
    {
        var inline = args.NewSize.Width >= 760;
        Inspector.DisplayMode = inline ? SplitViewDisplayMode.Inline : SplitViewDisplayMode.Overlay;
        Inspector.OpenPaneLength = Math.Min(320, Math.Max(260, args.NewSize.Width - 32));
        // Inline the pane sits on the content layer; as an overlay it needs an opaque surface.
        Inspector.PaneBackground = inline ? _transparentBrush : Root.Resources["InspectorOverlayBrush"] as Brush ?? _transparentBrush;
    }

    private void OnInspectorPaneClosed(SplitView sender, object args) => Details.SetActive(false);

    // ---- Keyboard ----

    private void OnFocusSearch(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        if (_settingsVisible || !Search.IsEnabled) return;
        Search.Focus(FocusState.Keyboard);
        args.Handled = true;
    }

    private bool EditingText => Root.XamlRoot is not null && FocusManager.GetFocusedElement(Root.XamlRoot) is TextBox or RichEditBox or PasswordBox
        or TextBlock { IsTextSelectionEnabled: true } or RichTextBlock { IsTextSelectionEnabled: true };

    private bool CanDragOut => !_stopping && !_reordering && ViewModel.Output is not null && !ViewModel.IsTrash
        && ViewModel.Batch?.IsRunning != true && !ViewModel.IsLoading && Gallery.SelectedItems.Count <= 1;

    private async void OnCopyShortcut(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        if (_settingsVisible || EditingText || ViewModel.IsTrash || Gallery.SelectedItems.Count != 1
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
        if (_settingsVisible || EditingText) return;
        Gallery.SelectAll();
        args.Handled = true;
    }

    /// <summary>Delete moves the open sticker to the trash, with undo in the notice.</summary>
    private async void OnDeleteShortcut(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        if (_settingsVisible || EditingText || ViewModel.IsTrash || _selecting || Gallery.SelectedItems.Count != 1
            || ViewModel.Details is not { CurrentId: not null, IsSaving: false } detail) return;
        args.Handled = true;
        await detail.DeleteAsync();
    }

    private async void OnEscape(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        if (_composing) return;
        if (SearchFocused)
        {
            // Esc clears the query first, and leaves the box once it is already empty.
            args.Handled = true;
            if (Search.Text.Length > 0) ClearSearch();
            else Gallery.Focus(FocusState.Keyboard);
            return;
        }
        if (_settingsVisible || EditingText) return;
        args.Handled = true;
        if (_reordering) { _reordering = false; UpdateState(); return; }
        if (_selecting) { SetSelecting(false); return; }
        if (Inspector.IsPaneOpen) await CloseInspectorAsync();
        else Gallery.SelectedItems.Clear();
    }

    // ---- Import ----

    private async void OnImportShortcut(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        if (!ImportButton.IsEnabled || ImportButton.Visibility != Visibility.Visible || _settingsVisible) return;
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
            if (_stopping || files.Count == 0) return;
            await ViewModel.ImportAsync(files.Select(file => file.Path).ToArray());
            if (_stopping) return;
            // The import keeps its outcome as a message key; it is announced once here.
            var severity = ViewModel.ImportMessageKey switch
            {
                "import_done" or "import_done_reused" or "import_done_existing" => InfoBarSeverity.Success,
                "import_restore_required" => InfoBarSeverity.Warning,
                null => InfoBarSeverity.Informational,
                _ => InfoBarSeverity.Error
            };
            Notice.Show(ViewModel.ImportText, severity);
        }
        catch (Exception error)
        {
            Trace.WriteLine(error);
            if (!_stopping) Notice.Show(Strings[Strings.ErrorKey(error)], InfoBarSeverity.Error);
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

    // ---- Lifecycle ----

    private async void OnClosing(AppWindow sender, AppWindowClosingEventArgs args)
    {
        if (_canClose) return;
        args.Cancel = true;
        if (_stopping) return;
        if (ViewModel.Details is { } detail && !await detail.CommitAsync()) return;
        if (_stopping) return;
        _stopping = true;
        _dialog?.Hide();
        Notice.Hide();
        Details.Shutdown();
        Batch.Shutdown();
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
            if (_searchBox is not null)
            {
                _searchBox.TextCompositionStarted -= OnCompositionStarted;
                _searchBox.TextCompositionEnded -= OnCompositionEnded;
            }
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

    private void OnActivated(object sender, WindowActivatedEventArgs args)
    {
        _windowActive = args.WindowActivationState != WindowActivationState.Deactivated;
        ViewModel.Output?.SetForeground(_windowActive);
        Details.SetActive(_windowActive && Inspector.IsPaneOpen && !_settingsVisible && Details.Visibility == Visibility.Visible);
    }

    private async void OnContentChanged(object? sender, string? id)
    {
        if (!_stopping && ViewModel.Details is { } detail && (id is null || id == detail.CurrentId)) await detail.RefreshAsync();
    }

    // ---- Trash and deletion ----

    private void OnStickerDeleted(object? sender, EventArgs args)
    {
        Gallery.SelectedItems.Clear();
        Notice.Show(Strings["sticker_deleted"], InfoBarSeverity.Success, Strings["restore"], UndoDeleteAsync);
    }

    private async Task UndoDeleteAsync()
    {
        if (ViewModel.Details is not { } detail || _stopping) return;
        if (await detail.UndoDeleteAsync()) Notice.Show(Strings["restored"], InfoBarSeverity.Success);
        else if (detail.ErrorText.Length > 0) Notice.Show(detail.ErrorText, InfoBarSeverity.Error);
        UpdateState();
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
        if (_dialog is not null || Root.XamlRoot is null) return false;
        var dialog = new ContentDialog
        {
            XamlRoot = Root.XamlRoot,
            Title = Strings["purge_title"],
            Content = Strings.Format("purge_hint", count),
            PrimaryButtonText = Strings["purge_title"],
            CloseButtonText = Strings["cancel"],
            DefaultButton = ContentDialogButton.Close
        };
        _dialog = dialog;
        try { return await dialog.ShowAsync() == ContentDialogResult.Primary; }
        finally { _dialog = null; }
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
        if (outcome.AnySucceeded)
        {
            Notice.Show(restored ? Strings["restored"] : Strings.Format("purge_result", outcome.Succeeded, outcome.Failed),
                outcome.AnyFailed ? InfoBarSeverity.Warning : InfoBarSeverity.Success);
        }
        UpdateState();
    }

    // ---- Gallery pointer and drag ----

    private async void OnGalleryDoubleTapped(object sender, DoubleTappedRoutedEventArgs args)
    {
        if (ViewModel.IsTrash || _selecting) return;
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
        if (ViewModel.IsTrash || _selecting || args.Key != global::Windows.System.VirtualKey.Enter || Gallery.SelectedItems.Count != 1
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
        if (!_reordering || args.DropResult == DataPackageOperation.None
            || args.Items.Count != 1 || args.Items[0] is not StickerCard card || ViewModel.Catalog is not { } catalog) return;
        var collection = ViewModel.Collections.FirstOrDefault(item => item.Id == ViewModel.Filter.Id);
        if (collection is null) return;
        var index = ViewModel.Cards.IndexOf(card);
        if (index < 0) return;
        await catalog.MoveStickerAsync(collection, card, index + 1 < ViewModel.Cards.Count ? ViewModel.Cards[index + 1].Id : null);
        ReportCatalogFailure(catalog);
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
        SetSelecting(false);
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
        UpdateState();
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
            var restore = new MenuFlyoutItem { Text = Strings["restore"], Icon = new FontIcon { Glyph = "\uE777" } };
            restore.Click += async (_, _) => await RunRestoreAsync([card]);
            var purge = new MenuFlyoutItem { Text = Strings["purge_title"], Icon = new FontIcon { Glyph = "\uE74D" } };
            purge.Click += async (_, _) => await RunPurgeAsync([card]);
            trashMenu.Items.Add(restore);
            trashMenu.Items.Add(new MenuFlyoutSeparator());
            trashMenu.Items.Add(purge);
            args.Handled = true;
            trashMenu.ShowAt(container);
            return;
        }
        if (ViewModel.Output is not { } output) return;
        var menu = new MenuFlyout();
        var copy = new MenuFlyoutItem
        {
            Text = Strings["copy_image"], Icon = new FontIcon { Glyph = "\uE8C8" }, IsEnabled = output.CanStart,
            KeyboardAcceleratorTextOverride = "Ctrl+C"
        };
        copy.Click += async (_, _) => await CopyCardAsync(card, StickerOutputKind.Image);
        menu.Items.Add(copy);
        var formats = new MenuFlyoutSubItem { Text = Strings["export_choose"], Icon = new FontIcon { Glyph = "\uE7C3" }, IsEnabled = output.CanStart };
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
            ReportCatalogFailure(catalog);
            await ViewModel.RetryAsync();
        };
        down.Click += async (_, _) =>
        {
            var current = ViewModel.Cards.IndexOf(card);
            if (current < 0 || current >= ViewModel.Cards.Count - 1) return;
            await catalog.MoveStickerAsync(collection, card, current + 2 < ViewModel.Cards.Count ? ViewModel.Cards[current + 2].Id : null);
            ReportCatalogFailure(catalog);
            await ViewModel.RetryAsync();
        };
        menu.Items.Add(up);
        menu.Items.Add(down);
        menu.ShowAt(container);
    }
}
