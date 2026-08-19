using System;
using System.Reactive.Disposables;

namespace LittleBigMouse.Ui.Avalonia.Main;

/// <summary>
/// Turns a classic .NET event into a subscription that can actually be released.
/// <para>
/// <c>source.Event += (s, e) => ...</c> is a one-way door: the delegate has no name left to
/// hand back to <c>-=</c>, so the handler — and everything it captured — lives exactly as
/// long as the event <em>source</em> does, not as long as the subscriber. That costs nothing
/// while the subscriber is a process-lifetime singleton and turns into a leak the day it
/// stops being one, which is the kind of change nobody remembers to audit for.
/// </para>
/// <para>
/// Pairing the add with its remove at the call site makes the lifetime a local decision and
/// hands back an <see cref="IDisposable"/> the owner keeps with the rest of its resources.
/// </para>
/// </summary>
static class OwnedSubscription
{
    /// <summary>
    /// Subscribe <paramref name="handler"/> through <paramref name="add"/> and return the
    /// token that undoes it. The handler must be a named method or a variable — passing a
    /// lambda expression here works, because the same delegate instance reaches both
    /// <paramref name="add"/> and <paramref name="remove"/>.
    /// </summary>
    public static IDisposable Create<THandler>(
        THandler handler, Action<THandler> add, Action<THandler> remove)
    {
        add(handler);
        return Disposable.Create(() => remove(handler));
    }
}
