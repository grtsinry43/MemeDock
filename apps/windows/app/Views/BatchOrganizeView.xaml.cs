using System.Collections.ObjectModel;
using System.ComponentModel;
using MemeDock.Bridge;
using MemeDock.Windows.Localization;
using MemeDock.Windows.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;

namespace MemeDock.Windows.Views;

/// <summary>
/// The inspector while several stickers are picked. Every action is one batch call for
/// the whole selection, never an edit of the single sticker that happened to be open.
/// </summary>
public sealed partial class BatchOrganizeView : UserControl
{
    private BatchOrganizeViewModel? _model;
    private LocalizationService? _strings;
    private ObservableCollection<CollectionMetadata>? _collections;
    private ObservableCollection<TagMetadata>? _tags;
    private bool _loadedOnly;
    public event EventHandler? CloseRequested;
    public event EventHandler? SelectAllRequested;
    public event EventHandler? ClearRequested;
    public BatchOrganizeView() => InitializeComponent();

    public void Configure(BatchOrganizeViewModel model, LocalizationService strings,
        ObservableCollection<CollectionMetadata> collections, ObservableCollection<TagMetadata> tags)
    {
        if (_model is not null) _model.PropertyChanged -= OnChanged;
        (_model, _strings, _collections, _tags) = (model, strings, collections, tags);
        model.PropertyChanged += OnChanged;
        Results.ItemsSource = model.Results;
        Relocalize();
    }

    public void Relocalize()
    {
        if (_strings is null) return;
        Heading.Text = _strings["batch_organize"];
        CloseButton.Label = _strings["done"];
        AutomationProperties.SetName(CloseButton, _strings["done"]);
        ToolTipService.SetToolTip(CloseButton, _strings["done"]);
        SelectAllButton.Content = _strings["desktop_select_all"];
        ClearButton.Content = _strings["desktop_clear_selection"];
        OrganizeLabel.Text = _strings["organize"];
        AssignLabel.Text = _strings["assign_collection"];
        AddTagsLabel.Text = _strings["add_tags"];
        RemoveTagsLabel.Text = _strings["remove_tags"];
        StarButton.Content = _strings["favorite"];
        UnstarButton.Content = _strings["unfavorite"];
        DeleteLabel.Text = _strings["delete"];
        UpdateState();
    }

    public void RefreshCatalogs() => UpdateState();

    /// <summary>Select all only reaches the pages the gallery has loaded, and says so.</summary>
    public void SetLoadedOnly(bool loadedOnly)
    {
        if (_loadedOnly == loadedOnly) return;
        _loadedOnly = loadedOnly;
        UpdateState();
    }

    private void OnChanged(object? sender, PropertyChangedEventArgs args) => UpdateState();

    private void UpdateState()
    {
        if (_model is null || _strings is null) return;
        Count.Text = _model.CountText;
        Hint.Text = _model.Count == 0 ? _strings["desktop_selection_hint"]
            : _loadedOnly ? _strings["desktop_selection_loaded_only"] : "";
        Hint.Visibility = MainWindow.ToVisibility(Hint.Text.Length > 0);
        Progress.Text = _model.ProgressText;
        Progress.Visibility = MainWindow.ToVisibility(_model.ProgressText.Length > 0);
        ErrorBar.IsOpen = _model.ErrorText.Length > 0;
        ErrorBar.Message = _model.ErrorText;
        Actions.IsEnabled = _model.CanRun;
        AssignButton.IsEnabled = _collections?.Count > 0;
        AddTagsButton.IsEnabled = RemoveTagsButton.IsEnabled = _tags?.Count > 0;
        SelectAllButton.IsEnabled = !_model.IsRunning;
        ClearButton.IsEnabled = !_model.IsRunning && _model.Count > 0;
        Running.Visibility = StopButton.Visibility = MainWindow.ToVisibility(_model.IsRunning);
        StopButton.Content = _strings[_model.IsStopping ? "import_cancelling" : "import_cancel_remaining"];
        StopButton.IsEnabled = !_model.IsStopping;
        CloseButton.IsEnabled = !_model.IsRunning;
        Results.Visibility = MainWindow.ToVisibility(_model.Results.Count > 0);
    }

    private void OnCollectionMenuOpening(object sender, object args)
    {
        CollectionMenu.Items.Clear();
        if (_strings is null || _collections is null) return;
        foreach (var collection in _collections)
        {
            var item = new MenuFlyoutItem { Text = collection.Name };
            item.Click += async (_, _) => await RunAsync(new BatchAction.Assign(new(collection.Id, collection.Lifecycle.Generation)));
            CollectionMenu.Items.Add(item);
        }
        CollectionMenu.Items.Add(new MenuFlyoutSeparator());
        var clear = new MenuFlyoutItem { Text = _strings["clear_collection"] };
        clear.Click += async (_, _) => await RunAsync(new BatchAction.ClearCollection(null));
        CollectionMenu.Items.Add(clear);
    }

    private void OnAddTagsOpening(object sender, object args) => FillTags(AddTagsMenu, add: true);
    private void OnRemoveTagsOpening(object sender, object args) => FillTags(RemoveTagsMenu, add: false);

    private void FillTags(MenuFlyout menu, bool add)
    {
        menu.Items.Clear();
        if (_tags is null) return;
        foreach (var tag in _tags)
        {
            var reference = new EntityReference(tag.Id, tag.Lifecycle.Generation);
            var item = new MenuFlyoutItem { Text = tag.Name };
            item.Click += async (_, _) => await RunAsync(add ? new BatchAction.AddTags([reference]) : new BatchAction.RemoveTags([reference]));
            menu.Items.Add(item);
        }
    }

    private Task RunAsync(BatchAction action) => _model?.RunAsync(action) ?? Task.CompletedTask;

    private async void OnStar(object sender, RoutedEventArgs args) => await RunAsync(new BatchAction.Star(true));
    private async void OnUnstar(object sender, RoutedEventArgs args) => await RunAsync(new BatchAction.Star(false));
    private async void OnDelete(object sender, RoutedEventArgs args) => await RunAsync(new BatchAction.Delete());
    private void OnStop(object sender, RoutedEventArgs args) => _model?.Cancel();
    private void OnClose(object sender, RoutedEventArgs args) => CloseRequested?.Invoke(this, EventArgs.Empty);
    private void OnSelectAll(object sender, RoutedEventArgs args) => SelectAllRequested?.Invoke(this, EventArgs.Empty);
    private void OnClearSelection(object sender, RoutedEventArgs args) => ClearRequested?.Invoke(this, EventArgs.Empty);
    public void Shutdown() { if (_model is not null) _model.PropertyChanged -= OnChanged; }
}
