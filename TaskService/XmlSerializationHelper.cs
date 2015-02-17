using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Reflection;
using System.Text;
using System.Xml;
using System.Xml.Serialization;

namespace Microsoft.Win32.TaskScheduler
{
	internal static class XmlSerializationHelper
	{
		public static object GetDefaultValue(PropertyInfo prop)
		{
			var attributes = prop.GetCustomAttributes(typeof(DefaultValueAttribute), true);
			if (attributes.Length > 0)
			{
				var defaultAttr = (DefaultValueAttribute)attributes[0];
				return defaultAttr.Value;
			}

			// Attribute not found, fall back to default value for the type 
			if (prop.PropertyType.IsValueType)
				return Activator.CreateInstance(prop.PropertyType);
			return null;
		}

		private static bool GetPropertyValue(object obj, string property, ref object outVal)
		{
			if (obj != null)
			{
				PropertyInfo pi = obj.GetType().GetProperty(property);
				if (pi != null)
				{
					outVal = pi.GetValue(obj, null);
					return true;
				}
			}
			return false;
		}

		private static bool GetAttributeValue(Type objType, Type attrType, string property, bool inherit, ref object outVal)
		{
			object[] attrs = objType.GetCustomAttributes(attrType, inherit);
			if (attrs.Length > 0)
				return GetPropertyValue(attrs[0], property, ref outVal);
			return false;
		}

		private static bool GetAttributeValue(PropertyInfo propInfo, Type attrType, string property, bool inherit, ref object outVal)
		{
			Attribute attr = Attribute.GetCustomAttribute(propInfo, attrType, inherit);
			return GetPropertyValue(attr, property, ref outVal);
		}

		private static bool IsStandardType(Type type)
		{
			return type.IsPrimitive || type == typeof(DateTime) || type == typeof(DateTimeOffset) || type == typeof(Decimal) || type == typeof(Guid) || type == typeof(TimeSpan) || type == typeof(string) || type.IsEnum;
		}

		private static bool HasMembers(object obj)
		{
			if (obj is IXmlSerializable)
			{
				using (System.IO.MemoryStream mem = new System.IO.MemoryStream())
				{
					using (XmlTextWriter tw = new XmlTextWriter(mem, Encoding.UTF8))
					{
						((IXmlSerializable)obj).WriteXml(tw);
						tw.Flush();
						return mem.Length > 0;
					}
				}
			}
			else
			{
				// Enumerate each public property
				PropertyInfo[] props = obj.GetType().GetProperties(BindingFlags.Instance | BindingFlags.Public);
				foreach (var pi in props)
				{
					if (!Attribute.IsDefined(pi, typeof(XmlIgnoreAttribute), false))
					{
						object value = pi.GetValue(obj, null);
						if (!value.Equals(GetDefaultValue(pi)))
						{
							if (!IsStandardType(pi.PropertyType))
							{
								if (HasMembers(value))
									return true;
							}
							else
								return true;
						}
					}
				}
			}
			return false;
		}

		public static string GetPropertyElementName(PropertyInfo pi)
		{
			object oVal = null;
			string eName = pi.Name;
			if (GetAttributeValue(pi, typeof(XmlElementAttribute), "ElementName", false, ref oVal))
				eName = oVal.ToString();
			else if (GetAttributeValue(pi.PropertyType, typeof(XmlRootAttribute), "ElementName", true, ref oVal))
				eName = oVal.ToString();
			return eName;
		}

		public delegate bool PropertyConversionHandler(PropertyInfo pi, Object obj, ref Object value);

		public static bool WriteProperty(XmlWriter writer, PropertyInfo pi, Object obj, PropertyConversionHandler handler = null)
		{
			if (Attribute.IsDefined(pi, typeof(XmlIgnoreAttribute), false))
				return false;

			object value = pi.GetValue(obj, null);
			object defValue = GetDefaultValue(pi);
			if ((value == null && defValue == null) || (value != null && value.Equals(defValue)))
				return false;

			Type propType = pi.PropertyType;
			if (handler != null && handler(pi, obj, ref value))
				propType = value.GetType();

			bool isStdType = IsStandardType(propType);
			bool rw = pi.CanRead && pi.CanWrite;
			bool ro = pi.CanRead && !pi.CanWrite;
			string eName = GetPropertyElementName(pi);
			if (isStdType && rw)
			{
				string output = null;
				if (propType.IsEnum)
				{
					if (Attribute.IsDefined(propType, typeof(FlagsAttribute), false))
						output = Convert.ChangeType(value, Enum.GetUnderlyingType(propType)).ToString();
					else
						output = value.ToString();
				}
				else
				{
					switch (propType.FullName)
					{
						case "System.Boolean":
							output = XmlConvert.ToString((System.Boolean)value);
							break;
						case "System.Byte":
							output = XmlConvert.ToString((System.Byte)value);
							break;
						case "System.Char":
							output = XmlConvert.ToString((System.Char)value);
							break;
						case "System.DateTime":
							output = XmlConvert.ToString((System.DateTime)value, XmlDateTimeSerializationMode.RoundtripKind);
							break;
						case "System.DateTimeOffset":
							output = XmlConvert.ToString((System.DateTimeOffset)value);
							break;
						case "System.Decimal":
							output = XmlConvert.ToString((System.Decimal)value);
							break;
						case "System.Double":
							output = XmlConvert.ToString((System.Double)value);
							break;
						case "System.Single":
							output = XmlConvert.ToString((System.Single)value);
							break;
						case "System.Guid":
							output = XmlConvert.ToString((System.Guid)value);
							break;
						case "System.Int16":
							output = XmlConvert.ToString((System.Int16)value);
							break;
						case "System.Int32":
							output = XmlConvert.ToString((System.Int32)value);
							break;
						case "System.Int64":
							output = XmlConvert.ToString((System.Int64)value);
							break;
						case "System.SByte":
							output = XmlConvert.ToString((System.SByte)value);
							break;
						case "System.TimeSpan":
							output = XmlConvert.ToString((System.TimeSpan)value);
							break;
						case "System.UInt16":
							output = XmlConvert.ToString((System.UInt16)value);
							break;
						case "System.UInt32":
							output = XmlConvert.ToString((System.UInt32)value);
							break;
						case "System.UInt64":
							output = XmlConvert.ToString((System.UInt64)value);
							break;
						default:
							output = value == null ? string.Empty : value.ToString();
							break;
					}
					if (output != null)
						writer.WriteElementString(eName, output);
				}
			}
			else if (!isStdType)
			{
				WriteObject(writer, value);
			}
			return false;
		}

		public static void WriteObjectProperties(XmlWriter writer, object obj, PropertyConversionHandler handler = null)
		{
			// Enumerate each public property
			foreach (var pi in obj.GetType().GetProperties(BindingFlags.Instance | BindingFlags.Public))
				WriteProperty(writer, pi, obj, handler);
		}

		public static void WriteObject(XmlWriter writer, object obj, PropertyConversionHandler handler = null)
		{
			if (obj == null)
				return;

			// Get name of top level element
			object oVal = null;
			string oName = GetAttributeValue(obj.GetType(), typeof(XmlRootAttribute), "ElementName", true, ref oVal) ? oVal.ToString() : obj.GetType().Name;

			// Get namespace of top level element
			string ns = GetAttributeValue(obj.GetType(), typeof(XmlRootAttribute), "Namespace", true, ref oVal) ? oVal.ToString() : null;

			if (!HasMembers(obj))
				return;

			writer.WriteStartElement(oName, ns);

			if (obj is IXmlSerializable)
			{
				((IXmlSerializable)obj).WriteXml(writer);
			}
			else
			{
				WriteObjectProperties(writer, obj, handler);
			}

			writer.WriteEndElement();
		}

		public static void ReadObjectProperties(XmlReader reader, object obj, PropertyConversionHandler handler = null)
		{
			// Build property lookup table
			PropertyInfo[] props = obj.GetType().GetProperties(BindingFlags.Instance | BindingFlags.Public);
			Dictionary<string, PropertyInfo> propHash = new Dictionary<string, PropertyInfo>(props.Length);
			foreach (var pi in props)
				if (!Attribute.IsDefined(pi, typeof(XmlIgnoreAttribute), false))
					propHash.Add(GetPropertyElementName(pi), pi);

			while (reader.MoveToContent() == System.Xml.XmlNodeType.Element)
			{
				PropertyInfo pi;
				if (propHash.TryGetValue(reader.LocalName, out pi))
				{
					if (IsStandardType(pi.PropertyType))
					{
						object value = null;
						if (pi.PropertyType.IsEnum)
							value = Enum.Parse(pi.PropertyType, reader.ReadElementContentAsString());
						else
							value = reader.ReadElementContentAs(pi.PropertyType, null);

						if (handler != null)
							handler(pi, obj, ref value);

						pi.SetValue(obj, value, null);
					}
					else
					{
						ReadObject(reader, pi.GetValue(obj, null));
					}
				}
				else
				{
					reader.Skip();
					reader.MoveToContent();
				}
			}
		}

		public static void ReadObject(XmlReader reader, object obj, PropertyConversionHandler handler = null)
		{
			if (obj == null)
				throw new ArgumentNullException("obj");

			reader.MoveToContent();

			if (obj is IXmlSerializable)
			{
				((IXmlSerializable)obj).ReadXml(reader);
			}
			else
			{
				object oVal = null;
				string oName = GetAttributeValue(obj.GetType(), typeof(XmlRootAttribute), "ElementName", true, ref oVal) ? oVal.ToString() : obj.GetType().Name;
				if (reader.LocalName != oName)
					throw new XmlException("XML element name does not match object.");

				if (!reader.IsEmptyElement)
				{
					reader.ReadStartElement();
					reader.MoveToContent();
					ReadObjectProperties(reader, obj, handler);
					reader.ReadEndElement();
				}
				else
					reader.Skip();
			}
		}

		public static void ReadObjectFromXmlText(string xml, object obj, PropertyConversionHandler handler = null)
		{
			using (System.IO.StringReader sr = new System.IO.StringReader(xml))
			{
				using (XmlReader reader = XmlReader.Create(sr))
				{
					reader.MoveToContent();
					ReadObject(reader, obj, handler);
				}
			}
		}

		public static string WriteObjectToXmlText(object obj, PropertyConversionHandler handler = null)
		{
			StringBuilder sb = new StringBuilder();
			using (XmlWriter writer = XmlWriter.Create(sb, new XmlWriterSettings() { Indent = true }))
				WriteObject(writer, obj, handler);
			return sb.ToString();
		}
	}
}
