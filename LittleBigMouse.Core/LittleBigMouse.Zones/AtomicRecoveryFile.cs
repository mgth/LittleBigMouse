using System.Text;
using System.Xml;

namespace LittleBigMouse.Zoning;

public static class AtomicRecoveryFile
{
    public static async Task WriteAsync(string path, string content, CancellationToken token)
    {
        Validate(content);
        var directory = Path.GetDirectoryName(path)
            ?? throw new InvalidOperationException("Recovery file has no directory");
        Directory.CreateDirectory(directory);
        var temporary = Path.Combine(directory, $".{Path.GetFileName(path)}.{Guid.NewGuid():N}.tmp");
        var backup = path + ".bak";

        try
        {
            await using (var file = new FileStream(temporary, FileMode.CreateNew, FileAccess.Write,
                             FileShare.None, 4096, FileOptions.Asynchronous | FileOptions.WriteThrough))
            {
                var bytes = Encoding.UTF8.GetBytes(content);
                await file.WriteAsync(bytes, token);
                await file.FlushAsync(token);
                file.Flush(flushToDisk: true);
            }

            if (File.Exists(path))
                File.Replace(temporary, path, backup, ignoreMetadataErrors: true);
            else
                File.Move(temporary, path);
        }
        finally
        {
            if (File.Exists(temporary)) File.Delete(temporary);
        }
    }

    static void Validate(string content)
    {
        if (Encoding.UTF8.GetByteCount(content) > 1024 * 1024)
            throw new InvalidDataException("Recovery configuration exceeds 1 MiB");
        foreach (var line in content.Split('\n', StringSplitOptions.RemoveEmptyEntries))
        {
            using var reader = XmlReader.Create(new StringReader(line), new XmlReaderSettings
            {
                DtdProcessing = DtdProcessing.Prohibit,
                MaxCharactersInDocument = 1024 * 1024,
                XmlResolver = null,
            });
            while (reader.Read()) { }
        }
    }
}
