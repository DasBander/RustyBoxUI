using View = RustyBoxUI.View;

namespace DotnetHost;

internal static class Program
{
    [STAThread]
    private static void Main(string[] args)
    {
        ApplicationConfiguration.Initialize();
        var profile = args is ["--profile", var directory] ? Path.GetFullPath(directory) :
            Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "RustyBoxUI", "dotnet-example");
        Application.Run(new EditorForm(profile));
    }
}

internal sealed record Greeting(string Name);
internal sealed record Message(string Text);

internal sealed class EditorForm : Form
{
    private readonly Panel surface = new() { Dock = DockStyle.Fill };
    private readonly System.Windows.Forms.Timer timer = new() { Interval = 16 };
    private readonly CancellationTokenSource lifetime = new();
    private View? view;

    private readonly string profile;

    internal EditorForm(string profile)
    {
        this.profile = profile;
        Text = "RustyBoxUI · .NET host";
        ClientSize = new Size(760, 440);
        Controls.Add(surface);
        surface.Resize += (_, _) => view?.Resize((uint)surface.ClientSize.Width, (uint)surface.ClientSize.Height);
        timer.Tick += (_, _) => { if (view?.Pump() == false) Close(); };
    }

    protected override async void OnShown(EventArgs e)
    {
        base.OnShown(e);

        try
        {
            view = await View.OpenAsync(surface.Handle, Text, Path.Combine(AppContext.BaseDirectory, "ui"),
                profile, (uint)surface.ClientSize.Width, (uint)surface.ClientSize.Height, lifetime.Token);
            view.Register<Greeting, Message>("host.greet", request =>
                new Message($"Hello, {request.Name.Trim()}."));
            timer.Start();
        }
        catch (OperationCanceledException) when (lifetime.IsCancellationRequested) { }
        catch (Exception error)
        {
            MessageBox.Show(this, error.Message, "Could not open the view");
            Close();
        }
    }

    protected override void OnFormClosed(FormClosedEventArgs e)
    {
        timer.Stop();
        lifetime.Cancel();
        view?.Dispose();
        timer.Dispose();
        lifetime.Dispose();
        base.OnFormClosed(e);
    }
}
