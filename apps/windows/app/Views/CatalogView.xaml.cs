using System.Collections.ObjectModel;
using System.ComponentModel;
using MemeDock.Bridge;
using MemeDock.Windows.Localization;
using MemeDock.Windows.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Windows.ApplicationModel.DataTransfer;

namespace MemeDock.Windows.Views;

public sealed partial class CatalogView : UserControl
{
    private CatalogViewModel? _model;
    private LocalizationService? _strings;
    private ObservableCollection<CollectionMetadata>? _collections;
    private ObservableCollection<TagMetadata>? _tags;
    private ContentDialog? _dialog;
    public CatalogView() => InitializeComponent();

    public void Configure(CatalogViewModel model, LocalizationService strings,
        ObservableCollection<CollectionMetadata> collections, ObservableCollection<TagMetadata> tags)
    {
        if (_model is not null) _model.PropertyChanged -= OnChanged;
        (_model, _strings, _collections, _tags) = (model, strings, collections, tags);
        model.PropertyChanged += OnChanged;
        Collections.ItemsSource = collections;
        Tags.ItemsSource = tags;
        Relocalize();
    }
    public void Relocalize()
    {
        if (_strings is null) return;
        Heading.Text = _strings["organize"];
        CollectionsHeading.Text = _strings["tab_collections"];
        TagsHeading.Text = _strings["tags_title"];
        NewCollectionLabel.Text = _strings["new_collection"];
        NewTagLabel.Text = _strings["new_tag"];
        AutomationProperties.SetName(NewCollection, _strings["new_collection"]);
        AutomationProperties.SetName(NewTag, _strings["new_tag"]);
        EmptyCollections.Text = _strings["collections_empty_hint"];
        EmptyTags.Text = _strings["batch_no_tags"];
        UndoButton.Content = _strings["restore"];
        AutomationProperties.SetName(Collections, _strings["tab_collections"]);
        AutomationProperties.SetName(Tags, _strings["tags_title"]);
        UpdateState();
    }
    public void RefreshCatalogs() => UpdateState();
    private void OnChanged(object? sender, PropertyChangedEventArgs args) => UpdateState();
    private void UpdateState()
    {
        if (_model is null) return;
        ErrorBar.IsOpen = _model.ErrorKey is not null;
        ErrorBar.Message = _model.ErrorText;
        MessageBar.IsOpen = _model.MessageKey is not null;
        MessageBar.Message = _model.MessageText;
        UndoButton.Visibility = MainWindow.ToVisibility(_model.LastDeleted is not null);
        UndoButton.IsEnabled = !_model.IsBusy;
        Actions.IsEnabled = !_model.IsBusy;
        Progress.Visibility = MainWindow.ToVisibility(_model.IsBusy);
        EmptyCollections.Visibility = MainWindow.ToVisibility(_collections?.Count == 0);
        EmptyTags.Visibility = MainWindow.ToVisibility(_tags?.Count == 0);
    }
    private async void OnNewCollection(object sender, RoutedEventArgs args) => await NameAsync(CatalogKind.Collection, null);
    private async void OnNewTag(object sender, RoutedEventArgs args) => await NameAsync(CatalogKind.Tag, null);
    private async Task NameAsync(CatalogKind kind, CatalogTarget? target)
    {
        if (_dialog is not null || _model is null || _strings is null || _model.IsBusy) return;
        var input = new TextBox { Header = _strings["name"], Text = target?.Name ?? "" };
        var error = new InfoBar { IsClosable = false, Severity = InfoBarSeverity.Error };
        var panel = new StackPanel { Spacing = 12 };
        panel.Children.Add(input);
        panel.Children.Add(error);
        var dialog = new ContentDialog
        {
            XamlRoot = XamlRoot,
            Title = _strings[target is null ? kind == CatalogKind.Collection ? "new_collection" : "new_tag" : "rename"],
            Content = panel, PrimaryButtonText = _strings["done"], CloseButtonText = _strings["cancel"],
            DefaultButton = ContentDialogButton.Primary, IsPrimaryButtonEnabled = !string.IsNullOrWhiteSpace(input.Text)
        };
        input.TextChanged += (_, _) => dialog.IsPrimaryButtonEnabled = !string.IsNullOrWhiteSpace(input.Text) && !_model.IsBusy;
        dialog.Opened += (_, _) => { input.Focus(FocusState.Programmatic); input.SelectAll(); };
        dialog.Closing += (_, args) => { if (_model.IsBusy) args.Cancel = true; };
        dialog.PrimaryButtonClick += async (_, args) =>
        {
            var deferral = args.GetDeferral();
            dialog.IsPrimaryButtonEnabled = input.IsEnabled = false;
            try
            {
                var success = target is null ? await _model.CreateAsync(kind, input.Text) : await _model.RenameAsync(target, input.Text);
                args.Cancel = !success;
                if (!success) { error.Message = _model.ErrorText; error.IsOpen = true; }
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
    private void OnMenuLoaded(object sender, RoutedEventArgs args)
    {
        if (sender is Button button && _strings is not null)
        {
            AutomationProperties.SetName(button, _strings["more_actions"]);
            ToolTipService.SetToolTip(button, _strings["more_actions"]);
        }
    }
    private void OnCollectionMenu(object sender, RoutedEventArgs args)
    {
        if (sender is Button { Tag: CollectionMetadata item } button) ShowMenu(button, CatalogTarget.From(item), item);
    }
    private void OnTagMenu(object sender, RoutedEventArgs args)
    {
        if (sender is Button { Tag: TagMetadata item } button) ShowMenu(button, CatalogTarget.From(item), null);
    }
    private void ShowMenu(FrameworkElement anchor, CatalogTarget target, CollectionMetadata? collection)
    {
        if (_strings is null || _model is null || _model.IsBusy) return;
        var menu = new MenuFlyout();
        var rename = new MenuFlyoutItem { Text = _strings["rename"], Icon = new SymbolIcon(Symbol.Edit) };
        rename.Click += async (_, _) => await NameAsync(target.Kind, target);
        menu.Items.Add(rename);
        if (collection is not null && _collections is not null)
        {
            var index = _collections.IndexOf(collection);
            var up = new MenuFlyoutItem { Text = _strings["move_up"], IsEnabled = index > 0 };
            var down = new MenuFlyoutItem { Text = _strings["move_down"], IsEnabled = index >= 0 && index < _collections.Count - 1 };
            up.Click += async (_, _) => { var current = _collections.IndexOf(collection); if (current > 0) await _model.MoveAsync(collection, _collections[current - 1].Id); };
            down.Click += async (_, _) => { var current = _collections.IndexOf(collection); if (current >= 0 && current < _collections.Count - 1) await _model.MoveAsync(collection, current + 2 < _collections.Count ? _collections[current + 2].Id : null); };
            menu.Items.Add(up);
            menu.Items.Add(down);
        }
        menu.Items.Add(new MenuFlyoutSeparator());
        var delete = new MenuFlyoutItem { Text = _strings["delete"], Icon = new SymbolIcon(Symbol.Delete) };
        delete.Click += async (_, _) => await DeleteAsync(target);
        menu.Items.Add(delete);
        menu.ShowAt(anchor);
    }
    private async Task DeleteAsync(CatalogTarget target)
    {
        if (_dialog is not null || _model is null || _strings is null) return;
        _dialog = new ContentDialog
        {
            XamlRoot = XamlRoot, Title = _strings["delete"] + " · " + target.Name,
            Content = _strings[target.Kind == CatalogKind.Collection ? "desktop_collection_delete_hint" : "delete_hint"],
            PrimaryButtonText = _strings["delete"], CloseButtonText = _strings["cancel"], DefaultButton = ContentDialogButton.Close
        };
        ContentDialogResult result;
        try { result = await _dialog.ShowAsync(); }
        finally { _dialog = null; }
        if (result == ContentDialogResult.Primary) await _model.DeleteAsync(target);
    }
    private async void OnCollectionsReordered(ListViewBase sender, DragItemsCompletedEventArgs args)
    {
        if (_model is null || _collections is null || args.DropResult == DataPackageOperation.None
            || args.Items.Count != 1 || args.Items[0] is not CollectionMetadata item) return;
        var index = _collections.IndexOf(item);
        if (index >= 0) await _model.MoveAsync(item, index + 1 < _collections.Count ? _collections[index + 1].Id : null);
    }
    private async void OnUndo(object sender, RoutedEventArgs args) { if (_model is not null) await _model.UndoAsync(); }
    public void Shutdown()
    {
        _dialog?.Hide();
        if (_model is not null) _model.PropertyChanged -= OnChanged;
    }
}
