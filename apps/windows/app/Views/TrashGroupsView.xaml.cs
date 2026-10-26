using System.Diagnostics;
using MemeDock.Bridge;
using MemeDock.Windows.Localization;
using MemeDock.Windows.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;

namespace MemeDock.Windows.Views;

public sealed partial class TrashGroupsView : UserControl
{
    private TrashGroupsViewModel? _model;
    private LocalizationService? _strings;
    private ContentDialog? _dialog;

    public TrashGroupsView() => InitializeComponent();

    public void Configure(TrashGroupsViewModel model, LocalizationService strings)
    {
        if (_model is not null) _model.PropertyChanged -= OnModelChanged;
        (_model, _strings) = (model, strings);
        model.PropertyChanged += OnModelChanged;
        Collections.ItemsSource = model.Collections;
        Tags.ItemsSource = model.Tags;
        Relocalize();
    }

    public void Relocalize()
    {
        if (_strings is null) return;
        Heading.Text = _strings["tab_collections"] + " / " + _strings["tags_title"];
        Hint.Text = _strings["trash_empty_hint"];
        CollectionsHeading.Text = _strings["tab_collections"];
        TagsHeading.Text = _strings["tags_title"];
        // Recreate the rows so their localized tooltips pick up the new language.
        if (_model is not null)
        {
            Collections.ItemsSource = null;
            Tags.ItemsSource = null;
            Collections.ItemsSource = _model.Collections;
            Tags.ItemsSource = _model.Tags;
        }
        UpdateState();
    }

    public void Shutdown()
    {
        try { _dialog?.Hide(); }
        catch (Exception error) { Trace.WriteLine(error); }
        _dialog = null;
        if (_model is not null) _model.PropertyChanged -= OnModelChanged;
        _model = null;
    }

    private void OnModelChanged(object? sender, System.ComponentModel.PropertyChangedEventArgs args)
        => DispatcherQueue?.TryEnqueue(UpdateState);

    private void UpdateState()
    {
        if (_model is null || _strings is null) return;
        Panel.Visibility = MainWindow.ToVisibility(_model.HasVisibleContent);
        if (_model.ErrorKey is not null || _model.MessageKey is not null)
            Section.IsExpanded = true;
        BusyRing.IsActive = _model.IsBusy;
        BusyRing.Visibility = MainWindow.ToVisibility(_model.IsBusy);
        Collections.IsEnabled = Tags.IsEnabled = !_model.IsBusy;
        CollectionsSection.Visibility = MainWindow.ToVisibility(_model.Collections.Count > 0);
        TagsSection.Visibility = MainWindow.ToVisibility(_model.Tags.Count > 0);
        CountText.Text = _model.TotalCount.ToString(_strings.Culture);
        ErrorBar.Message = _model.ErrorText;
        ErrorBar.IsOpen = _model.ErrorKey is not null;
        MessageBar.Message = _model.MessageText;
        MessageBar.IsOpen = _model.MessageKey is not null;
    }

    private void OnMessageClosed(InfoBar sender, InfoBarClosedEventArgs args) => _model?.DismissMessage();

    private void OnErrorClosed(InfoBar sender, InfoBarClosedEventArgs args) => _model?.DismissError();

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
        if (sender is Button { Tag: CollectionMetadata item } button && _model is { IsBusy: false } model)
        {
            ShowMenu(button,
                () => model.RestoreCollectionAsync(item),
                () => ConfirmAndPurgeAsync(model, item));
        }
    }

    private void OnTagMenu(object sender, RoutedEventArgs args)
    {
        if (sender is Button { Tag: TagMetadata item } button && _model is { IsBusy: false } model)
        {
            ShowMenu(button,
                () => model.RestoreTagAsync(item),
                () => ConfirmAndPurgeAsync(model, item));
        }
    }

    private void ShowMenu(FrameworkElement anchor, Func<Task> restore, Func<Task> purge)
    {
        if (_strings is null) return;
        var menu = new MenuFlyout();
        var restoreItem = new MenuFlyoutItem { Text = _strings["restore"], Icon = new SymbolIcon(Symbol.Undo) };
        restoreItem.Click += async (_, _) => await restore();
        var purgeItem = new MenuFlyoutItem { Text = _strings["purge_title"], Icon = new SymbolIcon(Symbol.Delete) };
        purgeItem.Click += async (_, _) => await purge();
        menu.Items.Add(restoreItem);
        menu.Items.Add(new MenuFlyoutSeparator());
        menu.Items.Add(purgeItem);
        menu.ShowAt(anchor);
    }

    private async Task ConfirmAndPurgeAsync(TrashGroupsViewModel model, CollectionMetadata collection)
        => await PurgeCollectionAsync(model, collection);

    private async Task ConfirmAndPurgeAsync(TrashGroupsViewModel model, TagMetadata tag)
        => await PurgeTagAsync(model, tag);

    private async Task PurgeCollectionAsync(TrashGroupsViewModel model, CollectionMetadata collection)
    {
        if (!await ConfirmPurgeAsync(isCollection: true)) return;
        await model.PurgeCollectionAsync(collection);
    }

    private async Task PurgeTagAsync(TrashGroupsViewModel model, TagMetadata tag)
    {
        if (!await ConfirmPurgeAsync(isCollection: false)) return;
        await model.PurgeTagAsync(tag);
    }

    /// <summary>
    /// The extra line promises surviving stickers only where that is true. A
    /// tag has no such promise, so it keeps the generic hint alone instead of
    /// reusing the collection-only wording.
    /// </summary>
    private async Task<bool> ConfirmPurgeAsync(bool isCollection)
    {
        if (_strings is null || XamlRoot is null || _dialog is not null) return false;
        var hint = _strings.Format("purge_hint", 1);
        var extra = TrashGroupsPresentation.ExtraPurgeHintKey(isCollection);
        var dialog = new ContentDialog
        {
            XamlRoot = XamlRoot,
            Title = _strings["purge_title"],
            Content = extra is null ? hint : hint + "\n\n" + _strings[extra],
            PrimaryButtonText = _strings["purge_title"],
            CloseButtonText = _strings["cancel"],
            DefaultButton = ContentDialogButton.Close
        };
        _dialog = dialog;
        try
        {
            return await dialog.ShowAsync() == ContentDialogResult.Primary;
        }
        catch (Exception error)
        {
            Trace.WriteLine(error);
            return false;
        }
        finally { _dialog = null; }
    }
}
