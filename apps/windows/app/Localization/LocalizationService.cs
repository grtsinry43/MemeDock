using System.ComponentModel;
using System.Globalization;
using MemeDock.Bridge;
using MemeDock.Windows.Services;
using Microsoft.Windows.ApplicationModel.Resources;

namespace MemeDock.Windows.Localization;

public sealed class LocalizationService : INotifyPropertyChanged, ICatalogStrings
{
    private readonly ResourceManager _manager = new(Path.ChangeExtension(typeof(LocalizationService).Assembly.Location, ".pri"));
    private readonly ResourceMap _strings;
    private readonly ResourceContext _context;
    public CultureInfo Culture { get; private set; } = CultureInfo.GetCultureInfo("en-US");
    public LanguageMode Mode { get; private set; }
    public event PropertyChangedEventHandler? PropertyChanged;

    public LocalizationService()
    {
        _strings = _manager.MainResourceMap.GetSubtree("Resources");
        _context = _manager.CreateResourceContext();
        SetLanguage(LanguageMode.System);
    }

    public string this[string key] => _strings.GetValue(key, _context).ValueAsString;
    public string Format(string key, params object[] arguments) => string.Format(Culture, this[key], arguments);

    public void SetLanguage(LanguageMode mode)
    {
        Mode = mode;
        var language = mode switch
        {
            LanguageMode.Chinese => "zh-CN",
            LanguageMode.English => "en-US",
            _ => CultureInfo.CurrentUICulture.TwoLetterISOLanguageName == "zh" ? "zh-CN" : "en-US"
        };
        Culture = CultureInfo.GetCultureInfo(language);
        _context.QualifierValues["Language"] = language;
        PropertyChanged?.Invoke(this, new PropertyChangedEventArgs("Item[]"));
    }

    public string ErrorKey(Exception error) => error switch
    {
        BridgeException.Failure failure => failure.code switch
        {
            ErrorCode.PermissionDenied => "failure_permission",
            ErrorCode.Io => "failure_io",
            ErrorCode.StorageFull => "failure_full",
            ErrorCode.UnsupportedFormat => "failure_format",
            ErrorCode.UnsupportedColorProfile => "failure_color_profile",
            ErrorCode.InvalidImage => "failure_image",
            ErrorCode.ResourceLimit => "failure_limit",
            ErrorCode.Busy or ErrorCode.AlreadyOpen => "failure_busy",
            ErrorCode.NotFound => "failure_missing",
            ErrorCode.Conflict => "failure_changed",
            ErrorCode.EntityDeleted => "failure_deleted",
            ErrorCode.InvalidInput => "failure_input",
            ErrorCode.CorruptData or ErrorCode.UnsupportedSchema or ErrorCode.Database => "failure_library",
            _ => "failure_unknown"
        },
        UnauthorizedAccessException => "failure_permission",
        IOException => "failure_io",
        _ => "failure_unknown"
    };
}
