using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Xml.Serialization;

namespace Microsoft.Win32.TaskScheduler
{
	/// <summary>
	/// Defines the type of actions a task can perform.
	/// </summary>
	/// <remarks>The action type is defined when the action is created and cannot be changed later. See <see cref="ActionCollection.AddNew"/>.</remarks>
	public enum TaskActionType
	{
		/// <summary>This action fires a handler.</summary>
		ComHandler = 5,
		/// <summary>This action performs a command-line operation. For example, the action can run a script, launch an executable, or, if the name of a document is provided, find its associated application and launch the application with the document.</summary>
		Execute = 0,
		/// <summary>This action sends and e-mail.</summary>
		SendEmail = 6,
		/// <summary>This action shows a message box.</summary>
		ShowMessage = 7
	}

	/// <summary>
	/// Abstract base class that provides the common properties that are inherited by all action objects. An action object is created by the <see cref="ActionCollection.AddNew"/> method.
	/// </summary>
	public abstract class Action : IDisposable, ICloneable, IEquatable<Action>
	{
		internal V2Interop.IAction iAction = null;

		/// <summary>List of unbound values when working with Actions not associated with a registered task.</summary>
		protected Dictionary<string, object> unboundValues = new Dictionary<string, object>();

		internal virtual bool Bound { get { return this.iAction != null; } }

		internal virtual void Bind(V1Interop.ITask iTask)
		{
		}

		internal virtual void Bind(V2Interop.ITaskDefinition iTaskDef)
		{
			V2Interop.IActionCollection iActions = iTaskDef.Actions;

			switch (this.GetType().Name)
			{
				case "ComHandlerAction":
					iAction = iActions.Create(TaskActionType.ComHandler);
					break;
				case "ExecAction":
					iAction = iActions.Create(TaskActionType.Execute);
					break;
				case "EmailAction":
					iAction = iActions.Create(TaskActionType.SendEmail);
					break;
				case "ShowMessageAction":
					iAction = iActions.Create(TaskActionType.ShowMessage);
					break;
				default:
					throw new ArgumentException();
			}
			Marshal.ReleaseComObject(iActions);
			foreach (string key in unboundValues.Keys)
			{
				try
				{
					iAction.GetType().InvokeMember(key, System.Reflection.BindingFlags.SetProperty, null, iAction, new object[] { unboundValues[key] });
				}
				catch (System.Reflection.TargetInvocationException tie) { throw tie.InnerException; }
				catch { }
			}
			unboundValues.Clear();
		}

		/// <summary>
		/// Creates a new object that is a copy of the current instance.
		/// </summary>
		/// <returns>
		/// A new object that is a copy of this instance.
		/// </returns>
		public object Clone()
		{
			Action ret = CreateAction(this.ActionType);
			ret.CopyProperties(this);
			return ret;
		}

		/// <summary>
		/// Copies the properties from another <see cref="Action"/> the current instance.
		/// </summary>
		/// <param name="sourceAction">The source <see cref="Action"/>.</param>
		protected virtual void CopyProperties(Action sourceAction)
		{
			this.Id = sourceAction.Id;
		}

		/// <summary>
		/// Releases all resources used by this class.
		/// </summary>
		public virtual void Dispose()
		{
			if (iAction != null)
				Marshal.ReleaseComObject(iAction);
		}

		/// <summary>
		/// Determines whether the specified <see cref="System.Object" />, is equal to this instance.
		/// </summary>
		/// <param name="obj">The <see cref="System.Object" /> to compare with this instance.</param>
		/// <returns>
		///   <c>true</c> if the specified <see cref="System.Object" /> is equal to this instance; otherwise, <c>false</c>.
		/// </returns>
		public override bool Equals(object obj)
		{
			if (obj is Action)
				return this.Equals((Action)obj);
			return base.Equals(obj);
		}

		/// <summary>
		/// Indicates whether the current object is equal to another object of the same type.
		/// </summary>
		/// <param name="other">An object to compare with this object.</param>
		/// <returns>
		/// true if the current object is equal to the <paramref name="other" /> parameter; otherwise, false.
		/// </returns>
		public virtual bool Equals(Action other)
		{
			return this.ActionType == other.ActionType && this.Id == other.Id;
		}

		/// <summary>
		/// Returns a hash code for this instance.
		/// </summary>
		/// <returns>
		/// A hash code for this instance, suitable for use in hashing algorithms and data structures like a hash table. 
		/// </returns>
		public override int GetHashCode()
		{
			return new { A = this.ActionType, B = this.Id }.GetHashCode();
		}

		/// <summary>
		/// Gets the type of the action.
		/// </summary>
		/// <value>The type of the action.</value>
		[XmlIgnore]
		public TaskActionType ActionType
		{
			get
			{
				if (iAction != null)
					return iAction.Type;
				if (this is ComHandlerAction)
					return TaskActionType.ComHandler;
				if (this is ShowMessageAction)
					return TaskActionType.ShowMessage;
				if (this is EmailAction)
					return TaskActionType.SendEmail;
				return TaskActionType.Execute;
			}
		}

		/// <summary>
		/// Gets or sets the identifier of the action.
		/// </summary>
		[DefaultValue(null)]
		[XmlAttribute(AttributeName = "id", DataType = "ID")]
		public virtual string Id
		{
			get { return (iAction == null) ? (unboundValues.ContainsKey("Id") ? (string)unboundValues["Id"] : null) : this.iAction.Id; }
			set { if (iAction == null) unboundValues["Id"] = value; else this.iAction.Id = value; }
		}

		/// <summary>
		/// Returns the action Id.
		/// </summary>
		/// <returns>String representation of action.</returns>
		public override string ToString()
		{
			return this.Id;
		}

		/// <summary>
		/// Returns a <see cref="System.String"/> that represents this action.
		/// </summary>
		/// <param name="culture">The culture.</param>
		/// <returns>String representation of action.</returns>
		public virtual string ToString(System.Globalization.CultureInfo culture)
		{
			using (new CultureSwitcher(culture))
				return this.ToString();
		}

		/// <summary>
		/// Creates a specialized class from a defined interface.
		/// </summary>
		/// <param name="iAction">Version 2.0 Action interface.</param>
		/// <returns>Specialized action class</returns>
		internal static Action CreateAction(V2Interop.IAction iAction)
		{
			switch (iAction.Type)
			{
				case TaskActionType.ComHandler:
					return new ComHandlerAction((V2Interop.IComHandlerAction)iAction);
				case TaskActionType.SendEmail:
					return new EmailAction((V2Interop.IEmailAction)iAction);
				case TaskActionType.ShowMessage:
					return new ShowMessageAction((V2Interop.IShowMessageAction)iAction);
				case TaskActionType.Execute:
				default:
					return new ExecAction((V2Interop.IExecAction)iAction);
			}
		}

		/// <summary>
		/// Creates the specified action.
		/// </summary>
		/// <param name="actionType">Type of the action to instantiate.</param>
		/// <returns><see cref="Action"/> of specified type.</returns>
		public static Action CreateAction(TaskActionType actionType)
		{
			switch (actionType)
			{
				case TaskActionType.ComHandler:
					return new ComHandlerAction();
				case TaskActionType.SendEmail:
					return new EmailAction();
				case TaskActionType.ShowMessage:
					return new ShowMessageAction();
				case TaskActionType.Execute:
				default:
					return new ExecAction();
			}
		}
	}

	/// <summary>
	/// Represents an action that fires a handler. Only available on Task Scheduler 2.0.
	/// </summary>
	[XmlType(IncludeInSchema = false)]
	[XmlRoot("ComHandler", Namespace = TaskDefinition.tns, IsNullable = false)]
	public sealed class ComHandlerAction : Action
	{
		/// <summary>
		/// Creates an unbound instance of <see cref="ComHandlerAction"/>.
		/// </summary>
		public ComHandlerAction()
		{
		}

		/// <summary>
		/// Creates an unbound instance of <see cref="ComHandlerAction"/>.
		/// </summary>
		/// <param name="classId">Identifier of the handler class.</param>
		/// <param name="data">Addition data associated with the handler.</param>
		public ComHandlerAction(Guid classId, string data)
		{
			this.ClassId = classId;
			this.Data = data;
		}

		internal ComHandlerAction(V2Interop.IComHandlerAction action)
		{
			iAction = action;
		}

		/// <summary>
		/// Gets or sets the identifier of the handler class.
		/// </summary>
		public Guid ClassId
		{
			get { return (iAction == null) ? (unboundValues.ContainsKey("ClassId") ? (Guid)unboundValues["ClassId"] : Guid.Empty) : new Guid(((V2Interop.IComHandlerAction)iAction).ClassId); }
			set { if (iAction == null) unboundValues["ClassId"] = value.ToString(); else ((V2Interop.IComHandlerAction)iAction).ClassId = value.ToString(); }
		}

		/// <summary>
		/// Gets the name of the object referred to by <see cref="ClassId"/>.
		/// </summary>
		public string ClassName
		{
			get { return GetNameForCLSID(this.ClassId); }
		}

		/// <summary>
		/// Gets or sets additional data that is associated with the handler.
		/// </summary>
		[DefaultValue(null)]
		public string Data
		{
			get { return (iAction == null) ? (unboundValues.ContainsKey("Data") ? (string)unboundValues["Data"] : null) : ((V2Interop.IComHandlerAction)iAction).Data; }
			set { if (iAction == null) unboundValues["Data"] = value; else ((V2Interop.IComHandlerAction)iAction).Data = value; }
		}

		/// <summary>
		/// Copies the properties from another <see cref="Action"/> the current instance.
		/// </summary>
		/// <param name="sourceAction">The source <see cref="Action"/>.</param>
		protected override void CopyProperties(Action sourceAction)
		{
			if (sourceAction.GetType() == this.GetType())
			{
				base.CopyProperties(sourceAction);
				this.ClassId = ((ComHandlerAction)sourceAction).ClassId;
				this.Data = ((ComHandlerAction)sourceAction).Data;
			}
		}

		/// <summary>
		/// Indicates whether the current object is equal to another object of the same type.
		/// </summary>
		/// <param name="other">An object to compare with this object.</param>
		/// <returns>
		/// true if the current object is equal to the <paramref name="other" /> parameter; otherwise, false.
		/// </returns>
		public override bool Equals(Action other)
		{
			return base.Equals(other) && this.ClassId == ((ComHandlerAction)other).ClassId && this.Data == ((ComHandlerAction)other).Data;
		}

		/// <summary>
		/// Gets a string representation of the <see cref="ComHandlerAction"/>.
		/// </summary>
		/// <returns>String represention this action.</returns>
		public override string ToString()
		{
			return string.Format(Properties.Resources.ComHandlerAction, this.ClassId, this.Data, this.Id, this.ClassName);
		}

		/// <summary>
		/// Gets the name for CLSID.
		/// </summary>
		/// <param name="guid">The unique identifier.</param>
		/// <returns></returns>
		internal static string GetNameForCLSID(Guid guid)
		{
			using (RegistryKey k = Registry.ClassesRoot.OpenSubKey("CLSID", false))
			{
				if (k != null)
				{
					using (RegistryKey k2 = k.OpenSubKey(guid.ToString("B"), false))
						return k2 != null ? k2.GetValue(null) as string : null;
				}
			}
			return null;
		}
	}

	/// <summary>
	/// Represents an action that executes a command-line operation.
	/// </summary>
	[XmlRoot("Exec", Namespace = TaskDefinition.tns, IsNullable = false)]
	public sealed class ExecAction : Action
	{
#if DEBUG
		internal const string PowerShellArgFormat = "-NoExit -Command \"& {{<# {0}:{1} #> {2}}}\"";
#else
		internal const string PowerShellArgFormat = "-NoLogo -NonInteractive -WindowStyle Hidden -Command \"& {{<# {0}:{1} #> {2}}}\"";
#endif
		internal const string PowerShellPath = "powershell";
		internal const string ScriptIdentifer = "TSML_20140424";

		private V1Interop.ITask v1Task;

		/// <summary>
		/// Creates a new instance of an <see cref="ExecAction"/> that can be added to <see cref="TaskDefinition.Actions"/>.
		/// </summary>
		public ExecAction() { }

		/// <summary>
		/// Creates a new instance of an <see cref="ExecAction"/> that can be added to <see cref="TaskDefinition.Actions"/>.
		/// </summary>
		/// <param name="path">Path to an executable file.</param>
		/// <param name="arguments">Arguments associated with the command-line operation. This value can be null.</param>
		/// <param name="workingDirectory">Directory that contains either the executable file or the files that are used by the executable file. This value can be null.</param>
		public ExecAction(string path, string arguments = null, string workingDirectory = null)
		{
			this.Path = path;
			this.Arguments = arguments;
			this.WorkingDirectory = workingDirectory;
		}

		internal ExecAction(V1Interop.ITask task)
		{
			v1Task = task;
		}

		internal ExecAction(V2Interop.IExecAction action)
		{
			iAction = action;
		}

		internal override bool Bound
		{
			get
			{
				if (v1Task != null)
					return true;
				return base.Bound;
			}
		}

		internal override void Bind(V1Interop.ITask v1Task)
		{
			object o = null;
			unboundValues.TryGetValue("Path", out o);
			v1Task.SetApplicationName(o == null ? string.Empty : o.ToString());
			o = null;
			unboundValues.TryGetValue("Arguments", out o);
			v1Task.SetParameters(o == null ? string.Empty : o.ToString());
			o = null;
			unboundValues.TryGetValue("WorkingDirectory", out o);
			v1Task.SetWorkingDirectory(o == null ? string.Empty : o.ToString());
		}

		internal static string BuildPowerShellCmd(string actionType, string cmd)
		{
			return string.Format(PowerShellArgFormat, ScriptIdentifer, actionType, cmd);
		}

		internal static ExecAction AsPowerShellCmd(string actionType, string cmd)
		{
			return new ExecAction(PowerShellPath, BuildPowerShellCmd(actionType, cmd));
		}

		/// <summary>
		/// Gets or sets the identifier of the action.
		/// </summary>
		/// <exception cref="NotV1SupportedException">Not supported under Task Scheduler 1.0.</exception>
		[DefaultValue(null)]
		[XmlAttribute(AttributeName = "id", DataType = "ID")]
		[XmlIgnore]
		public override string Id
		{
			get
			{
				if (v1Task != null)
					return System.IO.Path.GetFileNameWithoutExtension(Task.GetV1Path(v1Task)) + "_Action";
				return base.Id;
			}
			set
			{
				if (v1Task != null)
					throw new NotV1SupportedException();
				base.Id = value;
			}
		}

		/// <summary>
		/// Gets or sets the path to an executable file.
		/// </summary>
		[XmlElement("Command")]
		public string Path
		{
			get
			{
				if (v1Task != null)
					return v1Task.GetApplicationName();
				if (iAction != null)
					return ((V2Interop.IExecAction)iAction).Path;
				return unboundValues.ContainsKey("Path") ? (string)unboundValues["Path"] : null;
			}
			set
			{
				if (v1Task != null)
					v1Task.SetApplicationName(value);
				else if (iAction != null)
					((V2Interop.IExecAction)iAction).Path = value;
				else
					unboundValues["Path"] = value;
			}
		}

		/// <summary>
		/// Gets or sets the arguments associated with the command-line operation.
		/// </summary>
		[DefaultValue("")]
		public string Arguments
		{
			get
			{
				if (v1Task != null)
					return v1Task.GetParameters();
				if (iAction != null)
					return ((V2Interop.IExecAction)iAction).Arguments;
				return unboundValues.ContainsKey("Arguments") ? (string)unboundValues["Arguments"] : null;
			}
			set
			{
				if (v1Task != null)
					v1Task.SetParameters(value);
				else if (iAction != null)
					((V2Interop.IExecAction)iAction).Arguments = value;
				else
					unboundValues["Arguments"] = value;
			}
		}

		/// <summary>
		/// Gets or sets the directory that contains either the executable file or the files that are used by the executable file.
		/// </summary>
		[DefaultValue("")]
		public string WorkingDirectory
		{
			get
			{
				if (v1Task != null)
					return v1Task.GetWorkingDirectory();
				if (iAction != null)
					return ((V2Interop.IExecAction)iAction).WorkingDirectory;
				return unboundValues.ContainsKey("WorkingDirectory") ? (string)unboundValues["WorkingDirectory"] : null;
			}
			set
			{
				if (v1Task != null)
					v1Task.SetWorkingDirectory(value);
				else if (iAction != null)
					((V2Interop.IExecAction)iAction).WorkingDirectory = value;
				else
					unboundValues["WorkingDirectory"] = value;
			}
		}

		/// <summary>
		/// Gets a value indicating whether this instance is a powershell command.
		/// </summary>
		/// <value>
		/// <c>true</c> if this instance is a powershell command; otherwise, <c>false</c>.
		/// </value>
		internal bool IsPowerShellCmd
		{
			get { return this.Path != null && (this.Path.EndsWith(PowerShellPath, StringComparison.InvariantCultureIgnoreCase) || this.Path.EndsWith(PowerShellPath + ".exe", StringComparison.InvariantCultureIgnoreCase)); }
		}

		/// <summary>
		/// Copies the properties from another <see cref="Action"/> the current instance.
		/// </summary>
		/// <param name="sourceAction">The source <see cref="Action"/>.</param>
		protected override void CopyProperties(Action sourceAction)
		{
			if (sourceAction.GetType() == this.GetType())
			{
				base.CopyProperties(sourceAction);
				this.Path = ((ExecAction)sourceAction).Path;
				this.Arguments = ((ExecAction)sourceAction).Arguments;
				this.WorkingDirectory = ((ExecAction)sourceAction).WorkingDirectory;
			}
		}

		/// <summary>
		/// Indicates whether the current object is equal to another object of the same type.
		/// </summary>
		/// <param name="other">An object to compare with this object.</param>
		/// <returns>
		/// true if the current object is equal to the <paramref name="other" /> parameter; otherwise, false.
		/// </returns>
		public override bool Equals(Action other)
		{
			return base.Equals(other) && this.Path == ((ExecAction)other).Path && this.Arguments == ((ExecAction)other).Arguments && this.WorkingDirectory == ((ExecAction)other).WorkingDirectory;
		}

		/// <summary>
		/// Gets a string representation of the <see cref="ExecAction"/>.
		/// </summary>
		/// <returns>String represention this action.</returns>
		public override string ToString()
		{
			return string.Format(Properties.Resources.ExecAction, this.Path, this.Arguments, this.WorkingDirectory, this.Id);
		}
	}

	/// <summary>
	/// An interface that exposes the ability to convert an actions functionality to a PowerShell script.
	/// </summary>
	public interface IBindAsExecAction
	{
		/// <summary>
		/// Gets the PowerShell script for an action.
		/// </summary>
		/// <returns>Single line PowerShell script string.</returns>
		string GetPowerShellCommand();
	}

	/// <summary>
	/// Represents an action that sends an e-mail.
	/// </summary>
	[XmlType(IncludeInSchema = false)]
	[XmlRoot("SendEmail", Namespace = TaskDefinition.tns, IsNullable = false)]
	public sealed class EmailAction : Action, IBindAsExecAction
	{
		/// <summary>
		/// Creates an unbound instance of <see cref="EmailAction"/>.
		/// </summary>
		public EmailAction()
		{
		}

		/// <summary>
		/// Creates an unbound instance of <see cref="EmailAction"/>.
		/// </summary>
		/// <param name="subject">Subject of the e-mail.</param>
		/// <param name="from">E-mail address that you want to send the e-mail from.</param>
		/// <param name="to">E-mail address or addresses that you want to send the e-mail to.</param>
		/// <param name="body">Body of the e-mail that contains the e-mail message.</param>
		/// <param name="mailServer">Name of the server that you use to send e-mail from.</param>
		public EmailAction(string subject, string from, string to, string body, string mailServer)
		{
			this.Subject = subject;
			this.From = from;
			this.To = to;
			this.Body = body;
			this.Server = mailServer;
		}

		internal EmailAction(V2Interop.IEmailAction action)
		{
			iAction = action;
		}

		internal override void Bind(Microsoft.Win32.TaskScheduler.V2Interop.ITaskDefinition iTaskDef)
		{
			base.Bind(iTaskDef);
			if (nvc != null)
				nvc.Bind(((V2Interop.IEmailAction)iAction).HeaderFields);
		}

		/// <summary>
		/// Gets or sets the name of the server that you use to send e-mail from.
		/// </summary>
		public string Server
		{
			get { return (iAction == null) ? (unboundValues.ContainsKey("Server") ? (string)unboundValues["Server"] : null) : ((V2Interop.IEmailAction)iAction).Server; }
			set { if (iAction == null) unboundValues["Server"] = value; else ((V2Interop.IEmailAction)iAction).Server = value; }
		}

		/// <summary>
		/// Gets or sets the subject of the e-mail.
		/// </summary>
		[DefaultValue(null)]
		public string Subject
		{
			get { return (iAction == null) ? (unboundValues.ContainsKey("Subject") ? (string)unboundValues["Subject"] : null) : ((V2Interop.IEmailAction)iAction).Subject; }
			set { if (iAction == null) unboundValues["Subject"] = value; else ((V2Interop.IEmailAction)iAction).Subject = value; }
		}

		/// <summary>
		/// Gets or sets the e-mail address or addresses that you want to send the e-mail to.
		/// </summary>
		[DefaultValue(null)]
		public string To
		{
			get { return (iAction == null) ? (unboundValues.ContainsKey("To") ? (string)unboundValues["To"] : null) : ((V2Interop.IEmailAction)iAction).To; }
			set { if (iAction == null) unboundValues["To"] = value; else ((V2Interop.IEmailAction)iAction).To = value; }
		}

		/// <summary>
		/// Gets or sets the e-mail address or addresses that you want to Cc in the e-mail.
		/// </summary>
		[DefaultValue(null)]
		public string Cc
		{
			get { return (iAction == null) ? (unboundValues.ContainsKey("Cc") ? (string)unboundValues["Cc"] : null) : ((V2Interop.IEmailAction)iAction).Cc; }
			set { if (iAction == null) unboundValues["Cc"] = value; else ((V2Interop.IEmailAction)iAction).Cc = value; }
		}

		/// <summary>
		/// Gets or sets the e-mail address or addresses that you want to Bcc in the e-mail.
		/// </summary>
		[DefaultValue(null)]
		public string Bcc
		{
			get { return (iAction == null) ? (unboundValues.ContainsKey("Bcc") ? (string)unboundValues["Bcc"] : null) : ((V2Interop.IEmailAction)iAction).Bcc; }
			set { if (iAction == null) unboundValues["Bcc"] = value; else ((V2Interop.IEmailAction)iAction).Bcc = value; }
		}

		/// <summary>
		/// Gets or sets the e-mail address that you want to reply to.
		/// </summary>
		[DefaultValue(null)]
		public string ReplyTo
		{
			get { return (iAction == null) ? (unboundValues.ContainsKey("ReplyTo") ? (string)unboundValues["ReplyTo"] : null) : ((V2Interop.IEmailAction)iAction).ReplyTo; }
			set { if (iAction == null) unboundValues["ReplyTo"] = value; else ((V2Interop.IEmailAction)iAction).ReplyTo = value; }
		}

		/// <summary>
		/// Gets or sets the e-mail address that you want to send the e-mail from.
		/// </summary>
		[DefaultValue(null)]
		public string From
		{
			get { return (iAction == null) ? (unboundValues.ContainsKey("From") ? (string)unboundValues["From"] : null) : ((V2Interop.IEmailAction)iAction).From; }
			set { if (iAction == null) unboundValues["From"] = value; else ((V2Interop.IEmailAction)iAction).From = value; }
		}

		private NamedValueCollection nvc = null;

		/// <summary>
		/// Gets or sets the header information in the e-mail message to send.
		/// </summary>
		[XmlArray]
		[XmlArrayItem("HeaderField", typeof(NameValuePair))]
		public NamedValueCollection HeaderFields
		{
			get
			{
				if (nvc == null)
				{
					if (iAction != null)
						nvc = new NamedValueCollection(((V2Interop.IEmailAction)iAction).HeaderFields);
					else
						nvc = new NamedValueCollection();
				}
				return nvc;
			}
		}

		/// <summary>
		/// Gets or sets the body of the e-mail that contains the e-mail message.
		/// </summary>
		[DefaultValue(null)]
		public string Body
		{
			get { return (iAction == null) ? (unboundValues.ContainsKey("Body") ? (string)unboundValues["Body"] : null) : ((V2Interop.IEmailAction)iAction).Body; }
			set { if (iAction == null) unboundValues["Body"] = value; else ((V2Interop.IEmailAction)iAction).Body = value; }
		}

		/// <summary>
		/// Gets or sets an array of file paths to be sent as attachments with the e-mail. Each item must be a <see cref="System.String"/> value containing a path to file.
		/// </summary>
		[XmlArray("Attachments", IsNullable=true)]
		[XmlArrayItem(typeof(string), ElementName = "File")]
		public object[] Attachments
		{
			get { return (iAction == null) ? (unboundValues.ContainsKey("Attachments") ? (object[])unboundValues["Attachments"] : null) : ((V2Interop.IEmailAction)iAction).Attachments; }
			set
			{
				if (value != null)
				{
					if (value.Length > 8)
						throw new ArgumentOutOfRangeException("Attachments", "Attachments array cannot contain more than 8 items.");
					foreach (var o in value)
						if (!(o is string) || !System.IO.File.Exists((string)o))
							throw new ArgumentException("Each value of the array must contain a valid file reference.", "Attachments");
				}
				if (iAction == null)
				{
					if (value == null || value.Length == 0)
						unboundValues.Remove("Attachments");
					else
						unboundValues["Attachments"] = value;
				}
				else
					((V2Interop.IEmailAction)iAction).Attachments = value;
			}
		}

		/// <summary>
		/// Copies the properties from another <see cref="Action"/> the current instance.
		/// </summary>
		/// <param name="sourceAction">The source <see cref="Action"/>.</param>
		protected override void CopyProperties(Action sourceAction)
		{
			if (sourceAction.GetType() == this.GetType())
			{
				base.CopyProperties(sourceAction);
				if (((EmailAction)sourceAction).Attachments != null)
					this.Attachments = (object[])((EmailAction)sourceAction).Attachments.Clone();
				this.Bcc = ((EmailAction)sourceAction).Bcc;
				this.Body = ((EmailAction)sourceAction).Body;
				this.Cc = ((EmailAction)sourceAction).Cc;
				this.From = ((EmailAction)sourceAction).From;
				if (((EmailAction)sourceAction).nvc != null)
					((EmailAction)sourceAction).HeaderFields.CopyTo(this.HeaderFields);
				this.ReplyTo = ((EmailAction)sourceAction).ReplyTo;
				this.Server = ((EmailAction)sourceAction).Server;
				this.Subject = ((EmailAction)sourceAction).Subject;
				this.To = ((EmailAction)sourceAction).To;
			}
		}

		/// <summary>
		/// Indicates whether the current object is equal to another object of the same type.
		/// </summary>
		/// <param name="other">An object to compare with this object.</param>
		/// <returns>
		/// true if the current object is equal to the <paramref name="other" /> parameter; otherwise, false.
		/// </returns>
		public override bool Equals(Action other)
		{
			return base.Equals(other) && (this as IBindAsExecAction).GetPowerShellCommand() == (other as IBindAsExecAction).GetPowerShellCommand();
		}

		/// <summary>
		/// Gets a string representation of the <see cref="EmailAction"/>.
		/// </summary>
		/// <returns>String represention this action.</returns>
		public override string ToString()
		{
			return string.Format(Properties.Resources.EmailAction, this.Subject, this.To, this.Cc, this.Bcc, this.From, this.ReplyTo, this.Body, this.Server, this.Id);
		}

		private static string FromUTF8(string s)
		{
			byte[] bytes = System.Text.Encoding.UTF8.GetBytes(s);
			return System.Text.Encoding.Default.GetString(bytes);
		}

		private static string ToUTF8(string s)
		{
			byte[] bytes = System.Text.Encoding.Default.GetBytes(s);
			return System.Text.Encoding.UTF8.GetString(bytes);
		}

		string IBindAsExecAction.GetPowerShellCommand()
		{
			// Send-MailMessage [-To] <String[]> [-Subject] <String> [[-Body] <String> ] [[-SmtpServer] <String> ] -From <String> [-Attachments <String[]> ]
			//    [-Bcc <String[]> ] [-BodyAsHtml] [-Cc <String[]> ] [-Credential <PSCredential> ] [-DeliveryNotificationOption <DeliveryNotificationOptions> ]
			//    [-Encoding <Encoding> ] [-Port <Int32> ] [-Priority <MailPriority> ] [-UseSsl] [ <CommonParameters>]
			bool bodyIsHtml = this.Body != null && this.Body.Trim().StartsWith("<") && this.Body.Trim().EndsWith(">");
			var sb = new System.Text.StringBuilder();
			sb.AppendFormat("Send-MailMessage -From '{0}' -Subject '{1}' -SmtpServer '{2}' -Encoding UTF8", Prep(this.From), ToUTF8(Prep(this.Subject)), Prep(this.Server));
			if (!string.IsNullOrEmpty(this.To))
				sb.AppendFormat(" -To {0}", ToPS(this.To));
			if (!string.IsNullOrEmpty(this.Cc))
				sb.AppendFormat(" -Cc {0}", ToPS(this.Cc));
			if (!string.IsNullOrEmpty(this.Bcc))
				sb.AppendFormat(" -Bcc {0}", ToPS(this.Bcc));
			if (bodyIsHtml)
				sb.Append(" -BodyAsHtml");
			if (!string.IsNullOrEmpty(this.Body))
				sb.AppendFormat(" -Body '{0}'", ToUTF8(Prep(this.Body)));
			if (this.Attachments != null && this.Attachments.Length > 0)
				sb.AppendFormat(" -Attachments {0}", ToPS(Array.ConvertAll<object, string>(this.Attachments, o => Prep(o.ToString()))));
			return sb.ToString();

			/*var msg = new System.Net.Mail.MailMessage(this.From, this.To, this.Subject, this.Body);
			if (!string.IsNullOrEmpty(this.Bcc))
				msg.Bcc.Add(this.Bcc);
			if (!string.IsNullOrEmpty(this.Cc))
				msg.CC.Add(this.Cc);
			if (!string.IsNullOrEmpty(this.ReplyTo))
				msg.ReplyTo = new System.Net.Mail.MailAddress(this.ReplyTo);
			if (this.Attachments != null && this.Attachments.Length > 0)
				foreach (string s in this.Attachments)
					msg.Attachments.Add(new System.Net.Mail.Attachment(s));
			if (this.nvc != null)
				foreach (var ha in this.HeaderFields)
					msg.Headers.Add(ha.Name, ha.Value);
			var client = new System.Net.Mail.SmtpClient(this.Server);
			client.Send(msg);*/
		}

		private static string Prep(string s)
		{
			return s.Replace("'", "''");
		}

		private static string ToPS(string input, char[] delimeters = null)
		{
			if (delimeters == null)
				delimeters = new char[] { ';', ',' };
			return ToPS(Array.ConvertAll<string, string>(input.Split(delimeters), i => Prep(i.Trim())));
		}

		private static string ToPS(string[] input)
		{
			return string.Join(", ", Array.ConvertAll<string, string>(input, i => string.Concat("'", i.Trim(), "'")));
		}

		internal static Action FromPowerShellCommand(string p)
		{

			var match = System.Text.RegularExpressions.Regex.Match(p, @"^Send-MailMessage -From '(?<from>(?:[^']|'')*)' -Subject '(?<subject>(?:[^']|'')*)' -SmtpServer '(?<server>(?:[^']|'')*)'(?: -Encoding UTF8)?(?: -To (?<to>'(?:(?:[^']|'')*)'(?:, '(?:(?:[^']|'')*)')*))?(?: -Cc (?<cc>'(?:(?:[^']|'')*)'(?:, '(?:(?:[^']|'')*)')*))?(?: -Bcc (?<bcc>'(?:(?:[^']|'')*)'(?:, '(?:(?:[^']|'')*)')*))?(?:(?: -BodyAsHtml)? -Body '(?<body>(?:[^']|'')*)')?(?: -Attachments (?<att>'(?:(?:[^']|'')*)'(?:, '(?:(?:[^']|'')*)')*))?\s*$");
			if (match.Success)
			{
				EmailAction action = new EmailAction(UnPrep(FromUTF8(match.Groups["subject"].Value)), UnPrep(match.Groups["from"].Value), FromPS(match.Groups["to"]), UnPrep(FromUTF8(match.Groups["body"].Value)), UnPrep(match.Groups["server"].Value))
					{ Cc = FromPS(match.Groups["cc"]), Bcc = FromPS(match.Groups["bcc"]) };
				if (match.Groups["att"].Success)
					action.Attachments = Array.ConvertAll<string, object>(FromPS(match.Groups["att"].Value), s => s);
				return action;
			}
			return null;
		}

		private static string UnPrep(string s)
		{
			return s.Replace("''", "'");
		}

		private static string[] FromPS(string p)
		{
			var list = p.Split(new string[] { ", " }, StringSplitOptions.RemoveEmptyEntries);
			return Array.ConvertAll<string, string>(list, i => UnPrep(i).Trim('\''));
		}

		private static string FromPS(System.Text.RegularExpressions.Group g, string delimeter = ";")
		{
			if (g.Success)
				return string.Join(delimeter, FromPS(g.Value));
			return null;
		}
	}

	/// <summary>
	/// Represents an action that shows a message box when a task is activated.
	/// </summary>
	[XmlType(IncludeInSchema = false)]
	[XmlRoot("ShowMessage", Namespace = TaskDefinition.tns, IsNullable = false)]
	public sealed class ShowMessageAction : Action, IBindAsExecAction
	{
		/// <summary>
		/// Creates a new unbound instance of <see cref="ShowMessageAction"/>.
		/// </summary>
		public ShowMessageAction()
		{
		}

		/// <summary>
		/// Creates a new unbound instance of <see cref="ShowMessageAction"/>.
		/// </summary>
		/// <param name="messageBody">Message text that is displayed in the body of the message box.</param>
		/// <param name="title">Title of the message box.</param>
		public ShowMessageAction(string messageBody, string title)
		{
			this.MessageBody = messageBody;
			this.Title = title;
		}

		internal ShowMessageAction(V2Interop.IShowMessageAction action)
		{
			iAction = action;
		}

		/// <summary>
		/// Gets or sets the title of the message box.
		/// </summary>
		public string Title
		{
			get { return (iAction == null) ? (unboundValues.ContainsKey("Title") ? (string)unboundValues["Title"] : null) : ((V2Interop.IShowMessageAction)iAction).Title; }
			set { if (iAction == null) unboundValues["Title"] = value; else ((V2Interop.IShowMessageAction)iAction).Title = value; }
		}

		/// <summary>
		/// Gets or sets the message text that is displayed in the body of the message box.
		/// </summary>
		[XmlElement("Body")]
		public string MessageBody
		{
			get { return (iAction == null) ? (unboundValues.ContainsKey("MessageBody") ? (string)unboundValues["MessageBody"] : null) : ((V2Interop.IShowMessageAction)iAction).MessageBody; }
			set { if (iAction == null) unboundValues["MessageBody"] = value; else ((V2Interop.IShowMessageAction)iAction).MessageBody = value; }
		}

		/// <summary>
		/// Copies the properties from another <see cref="Action"/> the current instance.
		/// </summary>
		/// <param name="sourceAction">The source <see cref="Action"/>.</param>
		protected override void CopyProperties(Action sourceAction)
		{
			if (sourceAction.GetType() == this.GetType())
			{
				base.CopyProperties(sourceAction);
				this.Title = ((ShowMessageAction)sourceAction).Title;
				this.MessageBody = ((ShowMessageAction)sourceAction).MessageBody;
			}
		}

		/// <summary>
		/// Indicates whether the current object is equal to another object of the same type.
		/// </summary>
		/// <param name="other">An object to compare with this object.</param>
		/// <returns>
		/// true if the current object is equal to the <paramref name="other" /> parameter; otherwise, false.
		/// </returns>
		public override bool Equals(Action other)
		{
			return base.Equals(other) && (this as IBindAsExecAction).GetPowerShellCommand() == (other as IBindAsExecAction).GetPowerShellCommand();
		}

		/// <summary>
		/// Gets a string representation of the <see cref="ShowMessageAction"/>.
		/// </summary>
		/// <returns>String represention this action.</returns>
		public override string ToString()
		{
			return string.Format(Properties.Resources.ShowMessageAction, this.Title, this.MessageBody, this.Id);
		}

		string IBindAsExecAction.GetPowerShellCommand()
		{
			// [System.Reflection.Assembly]::LoadWithPartialName('System.Windows.Forms'); [System.Windows.Forms.MessageBox]::Show('Your_Desired_Message','Your_Desired_Title')
			var sb = new System.Text.StringBuilder("[System.Reflection.Assembly]::LoadWithPartialName('System.Windows.Forms'); [System.Windows.Forms.MessageBox]::Show('");
			sb.Append(this.MessageBody.Replace("'", "''"));
			if (this.Title != null)
			{
				sb.Append("','");
				sb.Append(this.Title.Replace("'", "''"));
			}
			sb.Append("')");
			return sb.ToString();
		}

		internal static Action FromPowerShellCommand(string p)
		{
			var match = System.Text.RegularExpressions.Regex.Match(p, @"^\[System.Reflection.Assembly\]::LoadWithPartialName\('System.Windows.Forms'\); \[System.Windows.Forms.MessageBox\]::Show\('(?<msg>(?:[^']|'')*)'(?:,'(?<t>(?:[^']|'')*)')?\)$");
			if (match.Success)
			{
				return new ShowMessageAction(match.Groups["msg"].Value.Replace("''", "'"), match.Groups["t"].Success ? match.Groups["t"].Value.Replace("''", "'") : null);
			}
			return null;
		}
	}
}
