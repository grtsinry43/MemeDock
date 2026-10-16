using System.Diagnostics;
using MemeDock.Windows.InteropProbe;
using Native = MemeDock.Windows.InteropProbe.MemedockWindowsInteropProbeMethods;

internal static class Program
{
    private static readonly TimeSpan Timeout = TimeSpan.FromSeconds(10);

    private static async Task<int> Main()
    {
        try
        {
            Check(Native.LiveProbeCount() == 0, "The native fixture must start with no live handles.");
            await Run("Unicode, bytes, optional values and unsigned 64-bit values", PayloadRoundTrip);
            await Run("Suspended future, busy reader and explicit cancellation", Cancellation);
            await Run("Closing wakes a suspended reader", Close);
            await Run("Owned handles survive another wrapper's disposal", HandleLifetime);
            await Run("Repeated cancellation releases native objects", RepeatedCancellation);
            Check(Native.LiveProbeCount() == 0, "Native objects leaked after the integration checks.");
            Console.WriteLine("PASS: UniFFI 0.32.2 native/C# compatibility probe.");
            return 0;
        }
        catch (Exception error)
        {
            Console.Error.WriteLine(error);
            return 1;
        }
    }

    private static async Task Run(string name, Func<Task> test)
    {
        await test().WaitAsync(Timeout);
        Console.WriteLine($"PASS: {name}");
    }

    private static async Task PayloadRoundTrip()
    {
        var payload = new Payload(
            Text: "\uFEFF表情库🙂\0Windows",
            Bytes: Enumerable.Range(0, 65536).Select(i => (byte)i).ToArray(),
            Number: ulong.MaxValue,
            Labels: ["收藏", "🙂", ""],
            Optional: null);
        AssertPayload(payload, Native.RoundTrip(payload));
        var withOptional = payload with { Optional = "存在\0🙂", Number = 0 };
        AssertPayload(withOptional, await Native.RoundTripAsync(withOptional).WaitAsync(Timeout));
    }

    private static void AssertPayload(Payload expected, Payload actual)
    {
        Check(expected.Text == actual.Text, "UTF-8 text changed across FFI.");
        Check(expected.Bytes.SequenceEqual(actual.Bytes), "Byte payload changed across FFI.");
        Check(expected.Number == actual.Number, "Unsigned integer changed across FFI.");
        Check(expected.Labels.SequenceEqual(actual.Labels), "Sequence changed across FFI.");
        Check(expected.Optional == actual.Optional, "Optional value changed across FFI.");
    }

    private static async Task Cancellation()
    {
        using var probe = Native.NewProbe();
        var pending = probe.WaitUntilCancelled();
        try
        {
            await Until(probe.HasWaiter);
            Check(!pending.IsCompleted, "The native future did not suspend.");
            await Throws<ProbeException.Busy>(() => probe.WaitUntilCancelled());
            probe.Cancel();
            probe.Cancel();
            await Throws<ProbeException.Cancelled>(() => pending);
            Check(!probe.HasWaiter(), "Completed future retained its reader reservation.");
        }
        finally
        {
            probe.Close();
        }
    }

    private static async Task Close()
    {
        using var probe = Native.NewProbe();
        var pending = probe.WaitUntilCancelled();
        try
        {
            await Until(probe.HasWaiter);
        }
        finally
        {
            probe.Close();
        }
        await Throws<ProbeException.Closed>(() => pending);
        await Throws<ProbeException.Closed>(() => probe.WaitUntilCancelled());
    }

    private static async Task HandleLifetime()
    {
        var baseline = Native.LiveProbeCount();
        using var original = Native.NewProbe();
        using var retained = Native.RetainProbe(original);
        var pending = original.WaitUntilCancelled();
        try
        {
            await Until(retained.HasWaiter);
            original.Dispose();
            original.Dispose();
            Check(Native.LiveProbeCount() == baseline + 1, "Disposal freed a still-owned native object.");
            await Throws<ObjectDisposedException>(() =>
            {
                original.HasWaiter();
                return Task.CompletedTask;
            });
        }
        finally
        {
            retained.Close();
        }
        await Throws<ProbeException.Closed>(() => pending);
        retained.Dispose();
        await Until(() => Native.LiveProbeCount() == baseline);
    }

    private static async Task RepeatedCancellation()
    {
        var baseline = Native.LiveProbeCount();
        for (var iteration = 0; iteration < 50; iteration++)
        {
            await Cancellation();
            await Until(() => Native.LiveProbeCount() == baseline);
        }
    }

    private static async Task Until(Func<bool> condition)
    {
        var elapsed = Stopwatch.StartNew();
        while (!condition())
        {
            if (elapsed.Elapsed >= Timeout)
                throw new TimeoutException("Native state did not reach the expected condition.");
            await Task.Delay(5);
        }
    }

    private static async Task Throws<T>(Func<Task> operation) where T : Exception
    {
        try
        {
            await operation().WaitAsync(Timeout);
        }
        catch (T)
        {
            return;
        }
        throw new InvalidOperationException($"Expected {typeof(T).Name}.");
    }

    private static void Check(bool condition, string message)
    {
        if (!condition)
            throw new InvalidOperationException(message);
    }
}
