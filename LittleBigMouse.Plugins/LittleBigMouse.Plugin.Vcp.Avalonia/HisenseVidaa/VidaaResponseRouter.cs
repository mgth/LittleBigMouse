#nullable enable
using System.Text;
using System.Text.Json;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.HisenseVidaa;

/// <summary>
/// One answer a command is waiting for on the shared session.
///
/// A VIDAA device does not answer a request on a channel of its own: replies and unsolicited
/// notifications arrive on the same subscribed topics. A command therefore arms the wait before
/// publishing its request, and the first matching message completes it — which is also why the
/// wait is disarmed afterwards, so a later notification does not resolve a command nobody asked.
/// </summary>
internal sealed class VidaaResponse<T>
{
    TaskCompletionSource<T>? _pending;

    /// <summary>
    /// Arms the wait and returns it, so the caller awaits the answer it armed rather than
    /// whichever one is pending when it gets around to waiting.
    /// </summary>
    public Task<T> Expect()
    {
        var pending = NewPending();
        _pending = pending;
        return pending.Task;
    }

    /// <summary>
    /// Arms the wait, publishes <paramref name="request"/>, and gives the device
    /// <paramref name="timeout"/> to answer.
    /// </summary>
    public async Task<T> CollectAsync(
        Func<CancellationToken, Task> request,
        TimeSpan timeout,
        CancellationToken cancellationToken)
    {
        var pending = NewPending();
        _pending = pending;
        try
        {
            await request(cancellationToken).ConfigureAwait(false);
            return await pending.Task.WaitAsync(timeout, cancellationToken).ConfigureAwait(false);
        }
        finally
        {
            if (ReferenceEquals(_pending, pending)) _pending = null;
        }
    }

    /// <summary>Hands the answer to whoever is waiting, and ignores it when nobody is.</summary>
    public void Complete(T value) => _pending?.TrySetResult(value);

    /// <inheritdoc cref="Complete"/>
    public void Fail(Exception error) => _pending?.TrySetException(error);

    static TaskCompletionSource<T> NewPending() => new(TaskCreationOptions.RunContinuationsAsynchronously);
}

/// <summary>
/// Reads what the projector publishes: completes the answer a command is waiting for, and stores
/// the tokens the pairing hands out. Recognising and decoding the messages stays in
/// <see cref="HisenseVidaaProtocol"/>; what is here is which waiting command each one belongs to.
/// </summary>
internal sealed class VidaaResponseRouter(HisenseVidaaConfiguration configuration)
{
    /// <summary>Completed by the projector accepting the PIN, failed by it rejecting it.</summary>
    public VidaaResponse<bool> PinAccepted { get; } = new();

    /// <summary>Completed once the access token of a fresh pairing has been stored.</summary>
    public VidaaResponse<bool> TokenIssued { get; } = new();

    public VidaaResponse<int> Volume { get; } = new();

    public VidaaResponse<IReadOnlyList<VidaaPictureSetting>> PictureSettings { get; } = new();

    /// <summary>
    /// Routes one message from the broker, retained replays included. Never throws: a malformed
    /// message is reported and dropped, because it arrives on the receive loop rather than on the
    /// thread of the command it would otherwise fault.
    /// </summary>
    public void Handle(string topic, byte[] bytes, bool retained)
    {
        try
        {
            Route(topic, bytes);
        }
        catch (Exception e)
        {
            Console.Error.WriteLine($"Invalid VIDAA response on {topic}: {e.Message}");
        }
    }

    void Route(string topic, byte[] bytes)
    {
        var payload = Encoding.UTF8.GetString(bytes);
        if (HisenseVidaaProtocol.TryParsePictureSettings(topic, payload, out var settings))
        {
            PictureSettings.Complete(settings);
            return;
        }
        if (HisenseVidaaProtocol.TryParseVolume(topic, payload, out var volume))
        {
            Volume.Complete(volume);
            return;
        }
        if (topic.Contains("tokenissuance", StringComparison.OrdinalIgnoreCase))
        {
            StoreIssuedTokens(bytes);
            return;
        }
        if (topic.Contains("authentication", StringComparison.OrdinalIgnoreCase))
            RouteAuthentication(bytes);
    }

    void StoreIssuedTokens(byte[] bytes)
    {
        using var json = JsonDocument.Parse(bytes);
        var root = json.RootElement;
        if (!root.TryGetProperty("accesstoken", out var accessToken)) return;

        configuration.AccessToken = accessToken.GetString() ?? "";
        configuration.RefreshToken = root.TryGetProperty("refreshtoken", out var refreshToken)
            ? refreshToken.GetString() ?? ""
            : "";
        var now = DateTimeOffset.UtcNow;
        configuration.AccessTokenIssuedAt = UnixTimeOr(root, "accesstoken_time", now);
        configuration.RefreshTokenIssuedAt = UnixTimeOr(root, "refreshtoken_time", now);
        configuration.AccessTokenDurationDays = IntegerOr(root, "accesstoken_duration_day", 7);
        configuration.RefreshTokenDurationDays = IntegerOr(root, "refreshtoken_duration_day", 30);
        TokenIssued.Complete(true);
    }

    void RouteAuthentication(byte[] bytes)
    {
        if (bytes.Length == 0)
        {
            // RemoteNOW acknowledges the PIN with an empty message on the authentication topic.
            PinAccepted.Complete(true);
            return;
        }

        using var response = JsonDocument.Parse(bytes);
        var result = response.RootElement;
        if (result.ValueKind != JsonValueKind.Object) return;
        if (result.TryGetProperty("result", out var accepted) && accepted.TryGetInt32(out var code))
        {
            if (code == 1) PinAccepted.Complete(true);
            else PinAccepted.Fail(new UnauthorizedAccessException("The VIDAA PIN was rejected."));
        }
        else if (result.TryGetProperty("statetype", out var state)
                 && state.GetString() == "authenticationcode")
        {
            // The PIN dialog is visible; the pairing exchange supplies the code.
        }
    }

    static int IntegerOr(JsonElement root, string property, int fallback)
        => root.TryGetProperty(property, out var value) && value.TryGetInt32(out var result) ? result : fallback;

    static DateTimeOffset UnixTimeOr(JsonElement root, string property, DateTimeOffset fallback)
        => root.TryGetProperty(property, out var value) && value.TryGetInt64(out var seconds)
            ? DateTimeOffset.FromUnixTimeSeconds(seconds)
            : fallback;
}
