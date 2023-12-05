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
    public LittleBigMouseCommand Command { get; set; }
    public ZonesLayout? Payload { get; set; }

    public string Serialize()
    {
        return ZoneSerializer.Serialize(this,e => e.Command, e => e.Payload);
    }
}