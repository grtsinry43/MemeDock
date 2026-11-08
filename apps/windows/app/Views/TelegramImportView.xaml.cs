using System.Collections.ObjectModel;
using System.ComponentModel;
using CommunityToolkit.Mvvm.ComponentModel;
using MemeDock.Bridge;
using MemeDock.Windows.Localization;
using MemeDock.Windows.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media.Imaging;

namespace MemeDock.Windows.Views;

public sealed class TelegramCard(TelegramSticker sticker) : ObservableObject
{
    internal TelegramSticker Sticker { get; } = sticker;
    public string Emoji => Sticker.Emoji ?? "";
    private BitmapImage? _preview;
    public BitmapImage? Preview { get => _preview; internal set => SetProperty(ref _preview, value); }
    public bool Selected { get; internal set; }
    public bool Selectable { get; internal set; }
    public string Description { get; internal set; } = "";
    internal bool Realized { get; set; }
    internal void Notify() => OnPropertyChanged(string.Empty);
}

public sealed partial class TelegramImportView : UserControl
{
    private TelegramImportViewModel? _model;
    private LocalizationService? _strings;
    private TelegramPackMetadata? _metadata;
    private readonly ObservableCollection<TelegramCard> _cards = [];
    private bool _updating, _detached;
    public TelegramImportView() { InitializeComponent(); Gallery.ItemsSource = _cards; }
    internal void Configure(TelegramImportViewModel model, LocalizationService strings)
    {
        (_model, _strings) = (model, strings);
        model.PropertyChanged += OnChanged; strings.PropertyChanged += OnLanguage;
        Relocalize(); Update();
    }
    internal void Detach()
    {
        _detached = true;
        if (_model is not null) _model.PropertyChanged -= OnChanged;
        if (_strings is not null) _strings.PropertyChanged -= OnLanguage;
        foreach (var card in _cards) { card.Realized = false; card.Preview = null; }
        Token.Password = "";
    }
    private void OnChanged(object? sender, PropertyChangedEventArgs args) => Update();
    private void OnLanguage(object? sender, PropertyChangedEventArgs args) { Relocalize(); Update(); }
    private void Relocalize()
    {
        if (_strings is null) return;
        Token.Header = _strings["tg_token"]; TokenHint.Text = _strings["tg_token_hint"];
        Remember.Content = _strings["tg_remember"]; Forget.Content = _strings["tg_forget"];
        Query.Header = _strings["tg_pack"]; Query.PlaceholderText = _strings["tg_pack_placeholder"];
        Load.Content = _strings["tg_load"]; SelectNew.Content = _strings["tg_select_new"]; Clear.Content = _strings["desktop_clear_selection"];
        Import.Content = _strings["tg_import"]; Cancel.Content = _strings["tg_cancel"];
    }
    private void Update()
    {
        if (_model is null || _strings is null || _detached) return;
        _updating = true;
        Token.Password = _model.Token; Query.Text = _model.Query; Remember.IsChecked = _model.Remember;
        _updating = false;
        Token.IsEnabled = Remember.IsEnabled = Forget.IsEnabled = Query.IsEnabled = Load.IsEnabled = !_model.Busy;
        Forget.Visibility = _model.TokenSaved ? Visibility.Visible : Visibility.Collapsed;
        SelectNew.IsEnabled = Clear.IsEnabled = !_model.Busy && _model.Metadata is not null;
        Import.IsEnabled = !_model.Busy && _model.Selected.Count > 0;
        Cancel.Visibility = _model.Busy ? Visibility.Visible : Visibility.Collapsed;
        PackTitle.Text = _model.Metadata?.Title ?? "";
        Busy.IsActive = _model.Phase == TelegramPhase.Loading;
        Busy.Visibility = Busy.IsActive ? Visibility.Visible : Visibility.Collapsed;
        Progress.Visibility = _model.Phase == TelegramPhase.Importing ? Visibility.Visible : Visibility.Collapsed;
        Progress.IsIndeterminate = _model.Progress is null;
        Progress.Maximum = Math.Max(1, _model.Progress?.Items.Length ?? 1); Progress.Value = _model.Completed;
        Count.Text = _model.Phase == TelegramPhase.Importing ? _strings.Format("tg_progress", _model.Completed, _model.Progress?.Items.Length ?? 0)
            : _strings.Format("tg_selected", _model.Selected.Count);
        Feedback.Message = _model.Failure ?? _model.Summary ?? "";
        Feedback.Severity = _model.Failure is null ? InfoBarSeverity.Informational : InfoBarSeverity.Error;
        Feedback.IsOpen = Feedback.Message.Length > 0;
        if (!ReferenceEquals(_metadata, _model.Metadata))
        {
            foreach (var card in _cards) card.Realized = false;
            _cards.Clear(); _metadata = _model.Metadata;
            foreach (var sticker in _metadata?.Stickers ?? []) _cards.Add(new(sticker));
        }
        foreach (var card in _cards)
        {
            var id = card.Sticker.Id;
            card.Selected = _model.Selected.Contains(id);
            card.Selectable = !_model.Busy && _model.CanSelect(id);
            var key = _model.Outcomes.TryGetValue(id, out var outcome) ? outcome switch
            {
                TelegramImportOutcome.Created => "tg_added", TelegramImportOutcome.Reused => "tg_existing",
                TelegramImportOutcome.RestoreRequired => "tg_restore", TelegramImportOutcome.Failed => "tg_failed", _ => "tg_pending"
            } : _model.States.GetValueOrDefault(id) switch
            { TelegramImportState.Imported => "tg_existing", TelegramImportState.RestoreRequired => "tg_restore", TelegramImportState.OriginalMissing => "tg_missing_original", _ => "tg_available" };
            card.Description = _strings[key];
            card.Notify();
        }
    }
    private void OnToken(object sender, RoutedEventArgs args) { if (!_updating && _model is not null && !_detached) _model.Token = Token.Password; }
    private void OnRemember(object sender, RoutedEventArgs args) { if (!_updating && _model is not null) _model.Remember = Remember.IsChecked == true; }
    private void OnQuery(object sender, TextChangedEventArgs args) { if (!_updating && _model is not null) _model.Query = Query.Text; }
    private async void OnLoad(object sender, RoutedEventArgs args) { if (_model is not null) await _model.LoadAsync(); }
    private async void OnQueryKey(object sender, Microsoft.UI.Xaml.Input.KeyRoutedEventArgs args)
    { if (args.Key == global::Windows.System.VirtualKey.Enter && _model is not null) { args.Handled = true; await _model.LoadAsync(); } }
    private async void OnForget(object sender, RoutedEventArgs args) { if (_model is not null) await _model.ForgetAsync(); }
    private void OnSelectNew(object sender, RoutedEventArgs args) => _model?.SelectNew();
    private void OnClear(object sender, RoutedEventArgs args) => _model?.ClearSelection();
    private void OnCancel(object sender, RoutedEventArgs args) => _model?.Cancel();
    private async void OnImport(object sender, RoutedEventArgs args) { if (_model is not null) await _model.ImportAsync(); }
    private void OnItem(object sender, ItemClickEventArgs args) { if (args.ClickedItem is TelegramCard card) _model?.Toggle(card.Sticker.Id); }
    private async void OnContainer(ListViewBase sender, ContainerContentChangingEventArgs args)
    {
        if (args.Item is not TelegramCard card || _model is null) return;
        if (args.InRecycleQueue) { card.Realized = false; card.Preview = null; return; }
        if (card.Realized) return;
        card.Realized = true;
        if (card.Sticker.HasPreview && await _model.PreviewAsync(card.Sticker.Id) is { } path && card.Realized && !_detached)
            card.Preview = new BitmapImage(new Uri(path)) { DecodePixelWidth = 160, AutoPlay = false };
    }
}
