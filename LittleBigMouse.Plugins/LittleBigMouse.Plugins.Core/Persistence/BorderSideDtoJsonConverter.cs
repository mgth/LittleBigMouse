#nullable enable
using System;
using System.Collections.Generic;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace LittleBigMouse.Plugins.Persistence;

/// <summary>
/// Reads both shapes of a stored border-resistance edge:
/// <list type="bullet">
/// <item>the historical <c>"Left": 20</c> — one number meaning "resist any crossing
/// by 20 mm", which maps to <c>Move == Drag == 20</c>;</item>
/// <item>the current <c>"Left": { "Move": …, "Drag": …, "Sections": [ … ] }</c>.</item>
/// </list>
/// <para>
/// Without this, deserializing an old layout would throw, and
/// <c>JsonLayoutStore.ReadJson</c> swallows exceptions and returns null — so every
/// existing user would silently lose their whole layout on first run. The registry
/// store has the same migration, written by hand in <c>ReadBorderResistance</c>.
/// </para>
/// </summary>
public class BorderSideDtoJsonConverter : JsonConverter<BorderSideDto>
{
    public override BorderSideDto? Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options)
    {
        switch (reader.TokenType)
        {
            case JsonTokenType.Null:
                return null;

            // Legacy: a single resistance governing every crossing.
            case JsonTokenType.Number:
            {
                var value = reader.GetDouble();
                return new BorderSideDto { Move = value, Drag = value };
            }

            case JsonTokenType.StartObject:
                break;

            default:
                throw new JsonException($"Unexpected token {reader.TokenType} for a border resistance side.");
        }

        var dto = new BorderSideDto();

        while (reader.Read())
        {
            if (reader.TokenType == JsonTokenType.EndObject) return dto;
            if (reader.TokenType != JsonTokenType.PropertyName) continue;

            var name = reader.GetString();
            reader.Read();

            switch (name)
            {
                case nameof(BorderSideDto.Move):
                    dto.Move = ReadNullableDouble(ref reader);
                    break;
                case nameof(BorderSideDto.MoveBlock):
                    dto.MoveBlock = ReadNullableBool(ref reader);
                    break;
                case nameof(BorderSideDto.Drag):
                    dto.Drag = ReadNullableDouble(ref reader);
                    break;
                case nameof(BorderSideDto.DragBlock):
                    dto.DragBlock = ReadNullableBool(ref reader);
                    break;
                case nameof(BorderSideDto.Sections):
                    dto.Sections = JsonSerializer.Deserialize<List<BorderSectionDto>>(ref reader, options);
                    break;
                default:
                    reader.Skip();
                    break;
            }
        }

        throw new JsonException("Unterminated border resistance side.");
    }

    public override void Write(Utf8JsonWriter writer, BorderSideDto value, JsonSerializerOptions options)
    {
        writer.WriteStartObject();

        if (value.Move is { } move) writer.WriteNumber(nameof(BorderSideDto.Move), move);
        if (value.MoveBlock is { } moveBlock) writer.WriteBoolean(nameof(BorderSideDto.MoveBlock), moveBlock);
        if (value.Drag is { } drag) writer.WriteNumber(nameof(BorderSideDto.Drag), drag);
        if (value.DragBlock is { } dragBlock) writer.WriteBoolean(nameof(BorderSideDto.DragBlock), dragBlock);

        if (value.Sections is { Count: > 0 } sections)
        {
            writer.WritePropertyName(nameof(BorderSideDto.Sections));
            JsonSerializer.Serialize(writer, sections, options);
        }

        writer.WriteEndObject();
    }

    static double? ReadNullableDouble(ref Utf8JsonReader reader) =>
        reader.TokenType == JsonTokenType.Number ? reader.GetDouble() : null;

    static bool? ReadNullableBool(ref Utf8JsonReader reader) => reader.TokenType switch
    {
        JsonTokenType.True => true,
        JsonTokenType.False => false,
        _ => null
    };
}
