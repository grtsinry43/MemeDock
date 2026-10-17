using System.Collections.ObjectModel;
using System.ComponentModel;
using MemeDock.Bridge;
using MemeDock.Windows.Localization;
using MemeDock.Windows.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;

namespace MemeDock.Windows.Views;

public sealed partial class BatchOrganizeView : UserControl
{
    private BatchOrganizeViewModel? _model;
    private LocalizationService? _strings;
    private ObservableCollection<CollectionMetadata>? _collections;
    private ObservableCollection<TagMetadata>? _tags;
    public event EventHandler? CloseRequested;
    public BatchOrganizeView() => InitializeComponent();

    public void Configure(BatchOrganizeViewModel model, LocalizationService strings,
        ObservableCollection<CollectionMetadata> collections, ObservableCollection<TagMetadata> tags)
    {
        if (_model is not null) _model.PropertyChanged -= OnChanged;
        (_model, _strings, _collections, _tags) = (model, strings, collections, tags);
        model.PropertyChanged += OnChanged;
        Collections.ItemsSource = collections;
        Tags.ItemsSource = tags;
        Results.ItemsSource = model.Results;
        Relocalize();
        RefreshCatalogs();
    }
    public void Relocalize()
    {
        if (_strings is null) return;
        Heading.Text = _strings["batch_organize"];
        AutomationProperties.SetName(CloseButton, _strings["message_dismiss"]);
        ToolTipService.SetToolTip(CloseButton, _strings["message_dismiss"]);
        CollectionHeading.Text = _strings["assign_collection"];
        AssignButton.Label = _strings["tab_collections"];
        ClearButton.Label = _strings["clear_collection"];
        TagsButton.Label = _strings["tags_title"];
        AutomationProperties.SetName(Collections, _strings["select_collections"]);
        TagsHeading.Text = _strings["tags_title"];
        AutomationProperties.SetName(Tags, _strings["select_tags"]);
        NoTags.Text = _strings["batch_no_tags"];
        AddTagsButton.Label = _strings["add_tags"];
        RemoveTagsButton.Label = _strings["remove_tags"];
        UpdateState();
    }
    public void RefreshCatalogs()
    {
        if (Collections.SelectedItem is null && _collections?.Count > 0) Collections.SelectedIndex = 0;
        NoTags.Visibility = MainWindow.ToVisibility(_tags?.Count == 0);
        UpdateState();
    }
    private void OnChanged(object? sender, PropertyChangedEventArgs args) => UpdateState();
    private void UpdateState()
    {
        if (_model is null || _strings is null) return;
        Count.Text = _model.CountText;
        Progress.Text = _model.ProgressText;
        ErrorBar.IsOpen = _model.ErrorText.Length > 0;
        ErrorBar.Message = _model.ErrorText;
        Actions.IsEnabled = _model.CanRun;
        StarButton.IsChecked = _model.AllStarred;
        StarGlyph.Glyph = _model.AllStarred ? "\uE735" : "\uE734";
        StarButton.Label = _strings[_model.AllStarred ? "unfavorite" : "favorite"];
        AutomationProperties.SetName(StarButton, StarButton.Label);
        ToolTipService.SetToolTip(StarButton, StarButton.Label);
        AssignButton.IsEnabled = _collections?.Count > 0;
        AddTagsButton.IsEnabled = RemoveTagsButton.IsEnabled = Tags.SelectedItems.Count > 0;
        Running.Visibility = StopButton.Visibility = MainWindow.ToVisibility(_model.IsRunning);
        StopButton.Content = _strings[_model.IsStopping ? "import_cancelling" : "import_cancel_remaining"];
        StopButton.IsEnabled = !_model.IsStopping;
        CloseButton.IsEnabled = !_model.IsRunning;
    }
    private async void OnAssign(object sender, ItemClickEventArgs args)
    {
        if (_model is not null && args.ClickedItem is CollectionMetadata collection)
        {
            CollectionFlyout.Hide();
            await _model.RunAsync(new BatchAction.Assign(new(collection.Id, collection.Lifecycle.Generation)));
        }
    }
    private async void OnClear(object sender, RoutedEventArgs args) { if (_model is not null) await _model.RunAsync(new BatchAction.ClearCollection(null)); }
    private async void OnAddTags(object sender, RoutedEventArgs args) => await ChangeTagsAsync(add: true);
    private async void OnRemoveTags(object sender, RoutedEventArgs args) => await ChangeTagsAsync(add: false);
    private async Task ChangeTagsAsync(bool add)
    {
        var tags = Tags.SelectedItems.Cast<TagMetadata>().Select(tag => new EntityReference(tag.Id, tag.Lifecycle.Generation)).ToArray();
        if (_model is null || tags.Length == 0) return;
        TagFlyout.Hide();
        await _model.RunAsync(add ? new BatchAction.AddTags(tags) : new BatchAction.RemoveTags(tags));
    }
    private async void OnStar(object sender, RoutedEventArgs args) { if (_model is not null) await _model.RunAsync(new BatchAction.Star(!_model.AllStarred)); }
    private void OnStop(object sender, RoutedEventArgs args) => _model?.Cancel();
    private void OnClose(object sender, RoutedEventArgs args) => CloseRequested?.Invoke(this, EventArgs.Empty);
    private void OnTagsChanged(object sender, SelectionChangedEventArgs args) => UpdateState();
    public void Shutdown() { if (_model is not null) _model.PropertyChanged -= OnChanged; }
}
