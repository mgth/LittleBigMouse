using System;
using System.Collections.Generic;
using System.Diagnostics.Eventing.Reader;

namespace Microsoft.Win32.TaskScheduler
{
	/// <summary>
	/// Historical event information for a task.
	/// </summary>
	public sealed class TaskEvent : IComparable<TaskEvent>
	{
		internal TaskEvent(EventRecord rec)
		{
			this.EventId = rec.Id;
			this.EventRecord = rec;
			this.Version = rec.Version;
			this.TaskCategory = rec.TaskDisplayName;
			this.OpCode = rec.OpcodeDisplayName;
			this.TimeCreated = rec.TimeCreated;
			this.RecordId = rec.RecordId;
			this.ActivityId = rec.ActivityId;
			this.Level = rec.LevelDisplayName;
			this.UserId = rec.UserId;
			this.ProcessId = rec.ProcessId;
			this.TaskPath = rec.Properties.Count > 0 ? rec.Properties[0].Value.ToString() : null;
		}

		/// <summary>
		/// Gets the activity id.
		/// </summary>
		public Guid? ActivityId
		{
			get; internal set;
		}

		/// <summary>
		/// Gets the data value from the task specific event data item list.
		/// </summary>
		/// <param name="name">The name of the data element.</param>
		/// <returns>Contents of the requested data element if found. <c>null</c> if no value found.</returns>
		/// <remarks>
		/// <list type="table">
		/// <listheader><term>EventID</term><description>Possible elements</description></listheader>
		/// <item><term>100 - Task Started</term>
		/// <description><list type="bullet">
		/// <item><term>TaskName</term><description>Full path of task.</description></item>
		/// <item><term>UserContext</term><description>User account.</description></item>
		/// <item><term>InstanceId</term><description>Task instance identifier.</description></item>
		/// </list></description>
		/// </item>
		/// <item><term>101 - Task Start Failed Event</term>
		/// <description><list type="bullet">
		/// <item><term>TaskName</term><description>Full path of task.</description></item>
		/// <item><term>UserContext</term><description>User account.</description></item>
		/// <item><term>ResultCode</term><description>Error code for failure.</description></item>
		/// </list></description>
		/// </item>
		/// <item><term>102 - Task Completed</term>
		/// <description><list type="bullet">
		/// <item><term>TaskName</term><description>Full path of task.</description></item>
		/// <item><term>UserContext</term><description>User account.</description></item>
		/// <item><term>InstanceId</term><description>Task instance identifier.</description></item>
		/// </list></description>
		/// </item>
		/// <item><term>103 - Task Failure Event</term>
		/// <description><list type="bullet">
		/// <item><term>TaskName</term><description>Full path of task.</description></item>
		/// <item><term>UserContext</term><description>User account.</description></item>
		/// <item><term>InstanceId</term><description>Task instance identifier.</description></item>
		/// <item><term>ResultCode</term><description>Error code for failure.</description></item>
		/// </list></description>
		/// </item>
		/// <item><term>106 - Task Registered Event</term>
		/// <description><list type="bullet">
		/// <item><term>TaskName</term><description>Full path of task.</description></item>
		/// <item><term>UserContext</term><description>User account.</description></item>
		/// </list></description>
		/// </item>
		/// <item><term>107 - Time Trigger Event</term>
		/// <description><list type="bullet">
		/// <item><term>TaskName</term><description>Full path of task.</description></item>
		/// <item><term>InstanceId</term><description>Task instance identifier.</description></item>
		/// </list></description>
		/// </item>
		/// <item><term>108 - Event Trigger Event</term>
		/// <description><list type="bullet">
		/// <item><term>TaskName</term><description>Full path of task.</description></item>
		/// <item><term>InstanceId</term><description>Task instance identifier.</description></item>
		/// </list></description>
		/// </item>
		/// <item><term>109 - Task Registration Trigger Event</term>
		/// <description><list type="bullet">
		/// <item><term>TaskName</term><description>Full path of task.</description></item>
		/// <item><term>InstanceId</term><description>Task instance identifier.</description></item>
		/// </list></description>
		/// </item>
		/// <item><term>110 - Task Run Event</term>
		/// <description><list type="bullet">
		/// <item><term>TaskName</term><description>Full path of task.</description></item>
		/// <item><term>InstanceId</term><description>Task instance identifier.</description></item>
		/// </list></description>
		/// </item>
		/// <item><term>111 - Task Termination Event</term>
		/// <description><list type="bullet">
		/// <item><term>TaskName</term><description>Full path of task.</description></item>
		/// <item><term>InstanceId</term><description>Task instance identifier.</description></item>
		/// </list></description>
		/// </item>
		/// <item><term>114 - Missed Task Launch Event</term>
		/// <description><list type="bullet">
		/// <item><term>TaskName</term><description>Full path of task.</description></item>
		/// <item><term>InstanceId</term><description>Task instance identifier.</description></item>
		/// </list></description>
		/// </item>
		/// <item><term>117 - Idle Trigger Event</term>
		/// <description><list type="bullet">
		/// <item><term>TaskName</term><description>Full path of task.</description></item>
		/// <item><term>InstanceId</term><description>Task instance identifier.</description></item>
		/// </list></description>
		/// </item>
		/// <item><term>118 - Boot Trigger Event</term>
		/// <description><list type="bullet">
		/// <item><term>TaskName</term><description>Full path of task.</description></item>
		/// <item><term>InstanceId</term><description>Task instance identifier.</description></item>
		/// </list></description>
		/// </item>
		/// <item><term>119 - Logon Trigger Event</term>
		/// <description><list type="bullet">
		/// <item><term>TaskName</term><description>Full path of task.</description></item>
		/// <item><term>UserName</term><description>User account.</description></item>
		/// <item><term>InstanceId</term><description>Task instance identifier.</description></item>
		/// </list></description>
		/// </item>
		/// <item><term>126 - Failed Task Restart Event</term>
		/// <description><list type="bullet">
		/// <item><term>TaskName</term><description>Full path of task.</description></item>
		/// <item><term>ResultCode</term><description>Error code for failure.</description></item>
		/// </list></description>
		/// </item>
		/// <item><term>129 - Created Task Process Event</term>
		/// <description><list type="bullet">
		/// <item><term>TaskName</term><description>Full path of task.</description></item>
		/// <item><term>Path</term><description>Task engine.</description></item>
		/// <item><term>ProcessID</term><description>Process ID.</description></item>
		/// <item><term>Priority</term><description>Process priority.</description></item>
		/// </list></description>
		/// </item>
		/// <item><term>135 - Task Not Started Without Idle</term>
		/// <description><list type="bullet">
		/// <item><term>TaskName</term><description>Full path of task.</description></item>
		/// </list></description>
		/// </item>
		/// <item><term>140 - Task Updated</term>
		/// <description><list type="bullet">
		/// <item><term>TaskName</term><description>Full path of task.</description></item>
		/// <item><term>UserName</term><description>User account.</description></item>
		/// </list></description>
		/// </item>
		/// <item><term>141 - Task deleted</term>
		/// <description><list type="bullet">
		/// <item><term>TaskName</term><description>Full path of task.</description></item>
		/// <item><term>UserName</term><description>User account.</description></item>
		/// </list></description>
		/// </item>
		/// <item><term>142 - Task disabled</term>
		/// <description><list type="bullet">
		/// <item><term>TaskName</term><description>Full path of task.</description></item>
		/// <item><term>UserName</term><description>User account.</description></item>
		/// </list></description>
		/// </item>
		/// <item><term>200 - Action start</term>
		/// <description><list type="bullet">
		/// <item><term>TaskName</term><description>Full path of task.</description></item>
		/// <item><term>ActionName</term><description>Path of executable.</description></item>
		/// <item><term>TaskInstanceId</term><description>Task instance ID.</description></item>
		/// <item><term>EnginePID</term><description>Engine process ID.</description></item>
		/// </list></description>
		/// </item>
		/// <item><term>201 - Action success</term>
		/// <description><list type="bullet">
		/// <item><term>TaskName</term><description>Full path of task.</description></item>
		/// <item><term>ActionName</term><description>Path of executable.</description></item>
		/// <item><term>TaskInstanceId</term><description>Task instance ID.</description></item>
		/// <item><term>ResultCode</term><description>Executable result code.</description></item>
		/// <item><term>EnginePID</term><description>Engine process ID.</description></item>
		/// </list></description>
		/// </item>
		/// <item><term>202 - Action failure</term>
		/// <description><list type="bullet">
		/// <item><term>TaskName</term><description>Full path of task.</description></item>
		/// <item><term>ActionName</term><description>Path of executable.</description></item>
		/// <item><term>TaskInstanceId</term><description>Task instance ID.</description></item>
		/// <item><term>ResultCode</term><description>Executable result code.</description></item>
		/// <item><term>EnginePID</term><description>Engine process ID.</description></item>
		/// </list></description>
		/// </item>
		/// <item><term>203 - Action launch failure</term>
		/// <description><list type="bullet">
		/// <item><term>TaskName</term><description>Full path of task.</description></item>
		/// <item><term>ActionName</term><description>Path of executable.</description></item>
		/// <item><term>TaskInstanceId</term><description>Task instance ID.</description></item>
		/// <item><term>ResultCode</term><description>Executable result code.</description></item>
		/// </list></description>
		/// </item>
		/// <item><term>204 - Action launch failure</term>
		/// <description><list type="bullet">
		/// <item><term>TaskName</term><description>Full path of task.</description></item>
		/// <item><term>ActionName</term><description>Path of executable.</description></item>
		/// <item><term>TaskInstanceId</term><description>Task instance ID.</description></item>
		/// <item><term>ResultCode</term><description>Executable result code.</description></item>
		/// </list></description>
		/// </item>
		/// <item><term>301 - Task engine exit event</term>
		/// <description><list type="bullet">
		/// <item><term>TaskEngineName</term><description>Permissions and priority for engine process.</description></item>
		/// </list></description>
		/// </item>
		/// <item><term>310 - Task engine process started</term>
		/// <description><list type="bullet">
		/// <item><term>TaskEngineName</term><description>Permissions and priority for engine process.</description></item>
		/// <item><term>Command</term><description>Engine path.</description></item>
		/// <item><term>ProcessID</term><description>Process ID.</description></item>
		/// <item><term>ThreadID</term><description>Thread ID.</description></item>
		/// </list></description>
		/// </item>
		/// <item><term>314 - Task engine idle</term>
		/// <description><list type="bullet">
		/// <item><term>TaskEngineName</term><description>Permissions and priority for engine process.</description></item>
		/// </list></description>
		/// </item>
		/// </list>
		/// </remarks>
		public string GetDataValue(string name)
		{
			var propsel = new System.Diagnostics.Eventing.Reader.EventLogPropertySelector(new string[] { string.Format("Event/EventData/Data[@Name='{0}']", name) });
			try
			{
				var logEventProps = ((EventLogRecord)this.EventRecord).GetPropertyValues(propsel);
				return logEventProps[0].ToString();
			}
			catch { }
			return null;
		}

		/// <summary>
		/// Gets the event id.
		/// </summary>
		public int EventId
		{
			get; internal set;
		}

		/// <summary>
		/// Gets the underlying <see cref="EventRecord"/>.
		/// </summary>
		public EventRecord EventRecord
		{
			get; internal set;
		}

		/// <summary>
		/// Gets the level.
		/// </summary>
		public string Level
		{
			get; internal set;
		}

		/// <summary>
		/// Gets the op code.
		/// </summary>
		public string OpCode
		{
			get; internal set;
		}

		/// <summary>
		/// Gets the process id.
		/// </summary>
		public int? ProcessId
		{
			get; internal set;
		}

		/// <summary>
		/// Gets the record id.
		/// </summary>
		public long? RecordId
		{
			get; internal set;
		}

		/// <summary>
		/// Gets the task category.
		/// </summary>
		public string TaskCategory
		{
			get; internal set;
		}

		/// <summary>
		/// Gets the task path.
		/// </summary>
		public string TaskPath
		{
			get; internal set;
		}

		/// <summary>
		/// Gets the time created.
		/// </summary>
		public DateTime? TimeCreated
		{
			get; internal set;
		}

		/// <summary>
		/// Gets the user id.
		/// </summary>
		public System.Security.Principal.SecurityIdentifier UserId
		{
			get; internal set;
		}

		/// <summary>
		/// Gets the version.
		/// </summary>
		public byte? Version
		{
			get; internal set;
		}

		/// <summary>
		/// Returns a <see cref="System.String"/> that represents this instance.
		/// </summary>
		/// <returns>
		/// A <see cref="System.String"/> that represents this instance.
		/// </returns>
		public override string ToString()
		{
			return EventRecord.FormatDescription();
		}

		/// <summary>
		/// Compares the current object with another object of the same type.
		/// </summary>
		/// <param name="other">An object to compare with this object.</param>
		/// <returns>
		/// A 32-bit signed integer that indicates the relative order of the objects being compared. The return value has the following meanings: Value Meaning Less than zero This object is less than the other parameter.Zero This object is equal to other. Greater than zero This object is greater than other.
		/// </returns>
		public int CompareTo(TaskEvent other)
		{
			int i = this.TaskPath.CompareTo(other.TaskPath);
			if (i == 0)
			{
				i = this.ActivityId.ToString().CompareTo(other.ActivityId.ToString());
				if (i == 0)
					i = Convert.ToInt32(this.RecordId - other.RecordId);
			}
			return i;
		}
	}

	/// <summary>
	/// An enumerator over a task's history of events.
	/// </summary>
	public sealed class TaskEventEnumerator : IEnumerator<TaskEvent>, IDisposable
	{
		private EventRecord curRec;
		private EventLogReader log;

		internal TaskEventEnumerator(EventLogReader log)
		{
			this.log = log;
		}

		/// <summary>
		/// Gets the element in the collection at the current position of the enumerator.
		/// </summary>
		/// <returns>
		/// The element in the collection at the current position of the enumerator.
		///   </returns>
		public TaskEvent Current
		{
			get { return new TaskEvent(curRec); }
		}

		/// <summary>
		/// Gets the element in the collection at the current position of the enumerator.
		/// </summary>
		/// <returns>
		/// The element in the collection at the current position of the enumerator.
		///   </returns>
		object System.Collections.IEnumerator.Current
		{
			get { return this.Current; }
		}

		/// <summary>
		/// Performs application-defined tasks associated with freeing, releasing, or resetting unmanaged resources.
		/// </summary>
		public void Dispose()
		{
			log.CancelReading();
			log.Dispose();
			log = null;
		}

		/// <summary>
		/// Advances the enumerator to the next element of the collection.
		/// </summary>
		/// <returns>
		/// true if the enumerator was successfully advanced to the next element; false if the enumerator has passed the end of the collection.
		/// </returns>
		/// <exception cref="T:System.InvalidOperationException">
		/// The collection was modified after the enumerator was created.
		///   </exception>
		public bool MoveNext()
		{
			return (curRec = log.ReadEvent()) != null;
		}

		/// <summary>
		/// Sets the enumerator to its initial position, which is before the first element in the collection.
		/// </summary>
		/// <exception cref="T:System.InvalidOperationException">
		/// The collection was modified after the enumerator was created.
		///   </exception>
		public void Reset()
		{
			log.Seek(System.IO.SeekOrigin.Begin, 0L);
		}

		/// <summary>
		/// Seeks the specified bookmark.
		/// </summary>
		/// <param name="bookmark">The bookmark.</param>
		/// <param name="offset">The offset.</param>
		public void Seek(EventBookmark bookmark, long offset = 0L)
		{
			log.Seek(bookmark, offset);
		}

		/// <summary>
		/// Seeks the specified origin.
		/// </summary>
		/// <param name="origin">The origin.</param>
		/// <param name="offset">The offset.</param>
		public void Seek(System.IO.SeekOrigin origin, long offset)
		{
			log.Seek(origin, offset);
		}
	}

	/// <summary>
	/// Historical event log for a task. Only available for Windows Vista and Windows Server 2008 and later systems.
	/// </summary>
	public sealed class TaskEventLog : IEnumerable<TaskEvent>
	{
		private EventLogQuery q;
		private const string TSEventLogPath = "Microsoft-Windows-TaskScheduler/Operational";

		/// <summary>
		/// Initializes a new instance of the <see cref="TaskEventLog"/> class.
		/// </summary>
		/// <param name="taskPath">The task path. This can be retrieved using the <see cref="Task.Path"/> property.</param>
		/// <exception cref="NotSupportedException">Thrown when instantiated on an OS prior to Windows Vista.</exception>
		public TaskEventLog(string taskPath) : this(".", taskPath)
		{
			Initialize(".", BuildQuery(taskPath), true);
		}

		/// <summary>
		/// Initializes a new instance of the <see cref="TaskEventLog" /> class.
		/// </summary>
		/// <param name="machineName">Name of the machine.</param>
		/// <param name="taskPath">The task path. This can be retrieved using the <see cref="Task.Path" /> property.</param>
		/// <param name="domain">The domain.</param>
		/// <param name="user">The user.</param>
		/// <param name="password">The password.</param>
		/// <exception cref="NotSupportedException">Thrown when instantiated on an OS prior to Windows Vista.</exception>
		public TaskEventLog(string machineName, string taskPath, string domain = null, string user = null, string password = null)
		{
			Initialize(machineName, BuildQuery(taskPath), true, domain, user, password);
		}

		/// <summary>
		/// Initializes a new instance of the <see cref="TaskEventLog" /> class that looks at all task events from a specified time.
		/// </summary>
		/// <param name="startTime">The start time.</param>
		/// <param name="taskName">Name of the task.</param>
		/// <param name="machineName">Name of the machine (optional).</param>
		/// <param name="domain">The domain.</param>
		/// <param name="user">The user.</param>
		/// <param name="password">The password.</param>
		public TaskEventLog(DateTime startTime, string taskName = null, string machineName = null, string domain = null, string user = null, string password = null)
		{
			int[] numArray = new int[] { 100, 102, 103, 107, 108, 109, 111, 117, 118, 119, 120, 121, 122, 123, 124, 125 };
			Initialize(machineName, BuildQuery(taskName, numArray, startTime), false, domain, user, password);
		}

		/// <summary>
		/// Initializes a new instance of the <see cref="TaskEventLog"/> class.
		/// </summary>
		/// <param name="taskName">Name of the task.</param>
		/// <param name="eventIDs">The event i ds.</param>
		/// <param name="startTime">The start time.</param>
		/// <param name="machineName">Name of the machine (optional).</param>
		/// <param name="domain">The domain.</param>
		/// <param name="user">The user.</param>
		/// <param name="password">The password.</param>
		public TaskEventLog(string taskName = null, int[] eventIDs = null, DateTime? startTime = null, string machineName = null, string domain = null, string user = null, string password = null)
		{
			Initialize(machineName, BuildQuery(taskName, eventIDs, startTime), true, domain, user, password);
		}

		internal static string BuildQuery(string taskName = null, int[] eventIDs = null, DateTime? startTime = null)
		{
			const string queryString =
				"<QueryList>" +
				"  <Query Id=\"0\" Path=\"" + TSEventLogPath + "\">" +
				"    <Select Path=\"" + TSEventLogPath + "\">{0}</Select>" +
				"  </Query>" +
				"</QueryList>";
			const string OR = " or ";
			const string AND = " and ";

			System.Text.StringBuilder sb = new System.Text.StringBuilder("*");
			if (eventIDs != null && eventIDs.Length > 0)
			{
				if (sb.Length > 1) sb.Append(AND);
				sb.AppendFormat("({0})", string.Join(OR, Array.ConvertAll<int, string>(eventIDs, i => string.Format("EventID={0}", i))));
			}
			if (startTime.HasValue)
			{
				if (sb.Length > 1) sb.Append(AND);
				sb.AppendFormat("TimeCreated[@SystemTime>='{0}']", System.Xml.XmlConvert.ToString(startTime.Value, System.Xml.XmlDateTimeSerializationMode.RoundtripKind));
			}
			if (sb.Length > 1)
			{
				sb.Insert(1, "[System[Provider[@Name='Microsoft-Windows-TaskScheduler'] and ");
				sb.Append("]");
			}
			if (!string.IsNullOrEmpty(taskName))
			{
				if (sb.Length == 1)
					sb.Append("[");
				else
					sb.Append("]" + AND + "*[");
				sb.AppendFormat("EventData[Data[@Name='TaskName']='{0}']", taskName);
			}
			if (sb.Length > 1)
				sb.Append("]");
			return string.Format(queryString, sb.ToString());
		}

		private void Initialize(string machineName, string query, bool revDir, string domain = null, string user = null, string password = null)
		{
			if (System.Environment.OSVersion.Version.Major < 6)
				throw new NotSupportedException("Enumeration of task history not available on systems prior to Windows Vista and Windows Server 2008.");

			System.Security.SecureString spwd = null;
			if (password != null)
			{
				spwd = new System.Security.SecureString();
				int l = password.Length;
				foreach (char c in password.ToCharArray(0, l))
					spwd.AppendChar(c);
			}

			q = new EventLogQuery(TSEventLogPath, PathType.LogName, query) { ReverseDirection = revDir };
			if (machineName != null && machineName != "." && !machineName.Equals(Environment.MachineName, StringComparison.InvariantCultureIgnoreCase))
				q.Session = new EventLogSession(machineName, domain, user, spwd, SessionAuthentication.Default);
		}

		/// <summary>
		/// Gets the total number of events for this task.
		/// </summary>
		public long Count
		{
			get
			{
				using (EventLogReader log = new EventLogReader(q))
				{
					long seed = 64L, l = 0L, h = seed;
					while (log.ReadEvent() != null)
						log.Seek(System.IO.SeekOrigin.Begin, l += seed);
					bool foundLast = false;
					while (l > 0L && h >= 1L)
					{
						if (foundLast)
							l += (h /= 2L);
						else
							l -= (h /= 2L);
						log.Seek(System.IO.SeekOrigin.Begin, l);
						foundLast = (log.ReadEvent() != null);
					}
					return foundLast ? l + 1L : l;
				}
			}
		}

		/// <summary>
		/// Gets or sets a value indicating whether to enumerate in reverse when calling the default enumerator (typically with foreach statement).
		/// </summary>
		/// <value>
		///   <c>true</c> if enumerates in reverse (newest to oldest) by default; otherwise, <c>false</c> to enumerate oldest to newest.
		/// </value>
		[System.ComponentModel.DefaultValue(false)]
		public bool EnumerateInReverse { get; set; }

		/// <summary>
		/// Returns an enumerator that iterates through the collection.
		/// </summary>
		/// <returns>
		/// A <see cref="T:System.Collections.Generic.IEnumerator`1"/> that can be used to iterate through the collection.
		/// </returns>
		public IEnumerator<TaskEvent> GetEnumerator()
		{
			return GetEnumerator(EnumerateInReverse);
		}

		/// <summary>
		/// Returns an enumerator that iterates through the collection.
		/// </summary>
		/// <param name="reverse">if set to <c>true</c> reverse.</param>
		/// <returns>
		/// A <see cref="T:System.Collections.Generic.IEnumerator`1" /> that can be used to iterate through the collection.
		/// </returns>
		public IEnumerator<TaskEvent> GetEnumerator(bool reverse)
		{
			q.ReverseDirection = !reverse;
			return new TaskEventEnumerator(new EventLogReader(q));
		}

		/// <summary>
		/// Returns an enumerator that iterates through a collection.
		/// </summary>
		/// <returns>
		/// An <see cref="T:System.Collections.IEnumerator"/> object that can be used to iterate through the collection.
		/// </returns>
		System.Collections.IEnumerator System.Collections.IEnumerable.GetEnumerator()
		{
			return GetEnumerator();
		}
	}
}