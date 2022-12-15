using System;
using System.Collections;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using System.Linq.Expressions;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using Avalonia;

namespace LittleBigMouse.Zoning
{
    public interface IXmlSerializable
    {
        string Serialize();
    }

    public class XmlSerializer
    {
        public static string Serialize<T>(T obj, params Expression<Func<T,object>>[] getters)
        {
            var name = typeof(T).Name;

            var inside = "";
            var p="";

            foreach(var getter in getters)
            {
                System.Linq.Expressions.Expression e = getter.Body;

                if(e is UnaryExpression ue && ue.NodeType == ExpressionType.Convert)
                {
                    e = ue.Operand;
                }

                if(e is MemberExpression m)
                {
                    var member = m.Member;

                    var lambda = getter.Compile();

                    var value = lambda(obj);

                    if(value is IXmlSerializable s)
                    {
                        inside+=$@"<{member.Name}>{s.Serialize()}</{member.Name}>";
                    }
                    else if (value is not string && value is IEnumerable en)
                    {
                        var list = "";

                        foreach(var i in en)
                        {
                            if(i is IXmlSerializable element)
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
                            
                        p+=$@" {member.Name}=""{value}""";
                    }
                }
                else
                { }
                
            }

            //if(string.IsNullOrWhiteSpace(inside)) return $@"<{name}{p}/>";

            return $@"<{name}{p}>{inside}</{name}>";
        }


        public static string Serialize(IXmlSerializable obj)
        {
            return obj.Serialize();
        }
        public static string Serialize(Rect rect)
        {
            return Serialize(rect,r=>r.Left,r=>r.Top,r=>r.Width,r=>r.Height);

            //return $@"<Rect><Top>{rect.Top}</Top><Left>{rect.Left}</Left><Width>{rect.Width}</Width><Height>{rect.Height}</Height></Rect>";
        }
    }
}
