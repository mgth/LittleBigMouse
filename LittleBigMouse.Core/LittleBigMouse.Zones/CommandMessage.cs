namespace LittleBigMouse.Zoning;

public class CommandMessage : IZonesSerializable
{
    public CommandMessage()
    {
    }
    public CommandMessage(LittleBigMouseCommand command)
    {
        Command = command;
    }

    public CommandMessage(LittleBigMouseCommand command, ZonesLayout payload)
    {
        Command = command;
        Payload = payload;
    }
    /// <summary>
    /// A command whose payload is plain text rather than a layout. A factory rather
    /// than an overload: `new(command, null)` is already used for payload-less
    /// commands, and a second two-argument constructor would make it ambiguous.
    /// </summary>
    public static CommandMessage WithText(LittleBigMouseCommand command, string text)
        => new(command) { Text = text };

    public LittleBigMouseCommand Command { get; set; }
    public ZonesLayout? Payload { get; set; }

    /// <summary>
    /// Text payload, for the commands that carry one. Kept apart from
    /// <see cref="Payload"/> because they land in the same place on the wire — the
    /// daemon reads `Payload` either as an attribute or as an element — and a single
    /// property could not be both a layout and a string.
    /// </summary>
    public string? Text { get; set; }

    public string Serialize()
    {
        if (Payload is null && Text is not null)
            return $@"<CommandMessage Command=""{Command}"" Payload=""{
                System.Security.SecurityElement.Escape(Text)}""/>";

        return ZoneSerializer.Serialize(this,e => e.Command, e => e.Payload);
    }
}