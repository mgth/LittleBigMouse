using System.Text;
using LittleBigMouse.Plugin.Vcp.Avalonia.HisenseVidaa;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.Tests;

/// <summary>
/// A VIDAA broker that never leaves the process: it records what a session publishes and
/// subscribes, hands messages back to it, and can refuse a connection or drop one the way a real
/// broker does — by failing the write that discovers it.
/// </summary>
sealed class FakeVidaaConnection : IVidaaConnection
{
    public List<(string Topic, string Payload)> Published { get; } = [];
    public List<string> Subscribed { get; } = [];
    public List<string> Unsubscribed { get; } = [];
    public bool Connected { get; private set; } = true;
    public bool Disposed { get; private set; }

    /// <summary>Fails the next publish, as a broker that dropped the session does.</summary>
    public Exception? PublishError { get; set; }

    /// <summary>Answers a published request, the way the device would.</summary>
    public Action<FakeVidaaConnection, string, string>? Answer { get; set; }

    public event Action<string, byte[], bool>? MessageReceived;

    /// <summary>
    /// Publishes a message to the session, as the device does — and, as a broker does, only to a
    /// session that subscribed a filter matching it. A session listening on the wrong topic
    /// therefore hears nothing, which is the failure this fake exists to reproduce.
    /// </summary>
    public void Receive(string topic, string payload, bool retained = false)
    {
        if (!Subscribed.Any(filter => Matches(filter, topic))) return;
        MessageReceived?.Invoke(topic, Encoding.UTF8.GetBytes(payload), retained);
    }

    /// <summary>Matches an MQTT 3.1.1 topic filter, wildcards included, against a topic name.</summary>
    internal static bool Matches(string filter, string topic)
    {
        var filterLevels = filter.Split('/');
        var topicLevels = topic.Split('/');
        for (var level = 0; level < filterLevels.Length; level++)
        {
            if (filterLevels[level] == "#") return true;
            if (level >= topicLevels.Length) return false;
            if (filterLevels[level] == "+") continue;
            if (!string.Equals(filterLevels[level], topicLevels[level], StringComparison.Ordinal))
                return false;
        }
        return filterLevels.Length == topicLevels.Length;
    }

    public Task SubscribeAsync(IEnumerable<string> topics, CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        Subscribed.AddRange(topics);
        return Task.CompletedTask;
    }

    public Task UnsubscribeAsync(IEnumerable<string> topics, CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        Unsubscribed.AddRange(topics);
        return Task.CompletedTask;
    }

    public Task PublishAsync(string topic, string payload, CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        if (PublishError is { } error)
        {
            PublishError = null;
            Connected = false;
            return Task.FromException(error);
        }
        Published.Add((topic, payload));
        Answer?.Invoke(this, topic, payload);
        return Task.CompletedTask;
    }

    public ValueTask DisposeAsync()
    {
        Disposed = true;
        Connected = false;
        return ValueTask.CompletedTask;
    }
}

/// <summary>Opens <see cref="FakeVidaaConnection"/>s, and records or refuses the attempts.</summary>
sealed class FakeVidaaTransport
{
    readonly Queue<Exception> _refusals = new();

    public List<VidaaConnectionRequest> Requests { get; } = [];
    public List<FakeVidaaConnection> Connections { get; } = [];

    /// <summary>The connection the session is using, once it has opened one.</summary>
    public FakeVidaaConnection Current => Connections[^1];

    /// <summary>Refuses the next attempt with <paramref name="error"/>.</summary>
    public void Refuse(Exception error) => _refusals.Enqueue(error);

    /// <summary>Prepares every connection this transport will open.</summary>
    public Action<FakeVidaaConnection>? OnOpened { get; set; }

    public Task<IVidaaConnection> OpenAsync(
        VidaaConnectionRequest request,
        CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        Requests.Add(request);
        if (_refusals.Count > 0) return Task.FromException<IVidaaConnection>(_refusals.Dequeue());

        var connection = new FakeVidaaConnection();
        OnOpened?.Invoke(connection);
        Connections.Add(connection);
        return Task.FromResult<IVidaaConnection>(connection);
    }
}
