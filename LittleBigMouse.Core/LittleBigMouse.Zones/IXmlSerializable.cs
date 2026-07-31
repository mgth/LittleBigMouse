using System.Collections;
using System.Collections.Concurrent;
using System.Globalization;
using System.Linq.Expressions;
using System.Reflection;
using HLab.Geo;

namespace LittleBigMouse.Zoning;

public interface IZonesSerializable
{
    string Serialize();
}

public class ZoneSerializer
{
    /// <summary>
    /// One compiled reader per member, for the whole process. The getters handed to
    /// <see cref="Serialize{T}"/> are rebuilt as fresh expression trees on every call, so
    /// compiling them there cost an <c>Expression.Compile</c> per property per zone per
    /// link — about 31 ms to serialize a four-monitor layout, against 0.1 ms to compute
    /// its zones. Keyed by the member itself, which is the only thing the compiled
    /// delegate depends on.
    /// </summary>
    static readonly ConcurrentDictionary<MemberInfo, Func<object?, object?>> Readers = new();

    static Func<object?, object?> ReaderFor(MemberInfo member) => Readers.GetOrAdd(member, static m =>
    {
        var instance = Expression.Parameter(typeof(object), "instance");
        var body = Expression.Convert(
            Expression.MakeMemberAccess(Expression.Convert(instance, m.DeclaringType!), m),
            typeof(object));
        return Expression.Lambda<Func<object?, object?>>(body, instance).Compile();
    });

    public static string Serialize<T>(T obj, params Expression<Func<T,object>>[] getters)
    {
        var name = typeof(T).Name;

        var inside = "";
        var p="";

        foreach(var getter in getters)
        {
            var e = getter.Body;

            if(e is UnaryExpression ue && ue.NodeType == ExpressionType.Convert)
            {
                e = ue.Operand;
            }

            if(e is MemberExpression m)
            {
                var member = m.Member;

                // Straight `x => x.Member` on the lambda parameter — the shape every
                // caller here uses — reads through the cached delegate. Anything else
                // (a nested path, a captured value) still compiles on the spot.
                var value = m.Expression is ParameterExpression && member.DeclaringType is not null
                    ? ReaderFor(member)(obj)
                    : getter.Compile()(obj);

                if(value is IZonesSerializable s)
                {
                    inside+=$@"<{member.Name}>{s.Serialize()}</{member.Name}>";
                }
                else if (value is not string && value is IEnumerable en)
                {
                    var list = "";

                    foreach(var i in en)
                    {
                        if(i is IZonesSerializable element)
                        {
                            list += element.Serialize();
                        }
                    }
                    inside+=$@"<{member.Name}>{list}</{member.Name}>";
                }
                else if (value is Rect r)
                {
                    inside+=$@"<{member.Name}>{Serialize(r)}</{member.Name}>";
                }
                else {
                    switch(value)
                    {
                        case double i: value = i.ToString(CultureInfo.InvariantCulture); break;
                        case float i: value = i.ToString(CultureInfo.InvariantCulture); break;
                        case decimal i: value = i.ToString(CultureInfo.InvariantCulture); break;
                    }

                    // Monitor names come straight from EDID/device strings: a raw
                    // & or " here breaks both the daemon's parser and the recovery
                    // file validation.
                    p+=$@" {member.Name}=""{EscapeAttribute(value)}""";
                }
            }
            else
            { }
                
        }

        //if(string.IsNullOrWhiteSpace(inside)) return $@"<{name}{p}/>";

        return $@"<{name}{p}>{inside}</{name}>";
    }

    static string EscapeAttribute(object? value) =>
        System.Security.SecurityElement.Escape(value?.ToString() ?? string.Empty);

    public static string Serialize(IZonesSerializable obj)
    {
        return obj.Serialize();
    }

    public static string Serialize(Rect rect)
    {
        return Serialize(rect,r=>r.Left,r=>r.Top,r=>r.Width,r=>r.Height);

        //return $@"<Rect><Top>{rect.Top}</Top><Left>{rect.Left}</Left><Width>{rect.Width}</Width><Height>{rect.Height}</Height></Rect>";
    }
}