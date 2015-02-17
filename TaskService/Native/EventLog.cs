#if NET20
/* ++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
 * Event methods and classes literally pulled from the .NET 4.0 implementation.
 * None of this is original work. It comes straight from decompiled Microsoft
 * assemblies.
 * ++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++*/
using Microsoft.Win32.SafeHandles;
using System.Collections.Generic;
using System.ComponentModel;
using System.Globalization;
using System.IO;
using System.Runtime.ConstrainedExecution;
using System.Runtime.InteropServices;
using System.Runtime.Serialization;
using System.Security;
using System.Security.Permissions;
using System.Security.Principal;
using System.Text;

namespace System.Diagnostics.Eventing.Reader
{
	/// <summary>
	/// PathType
	/// </summary>
	public enum PathType
	{
		/// <summary>
		/// The file path
		/// </summary>
		FilePath = 2,
		/// <summary>
		/// The log name
		/// </summary>
		LogName = 1
	}

	/// <summary>
	/// SessionAuthentication
	/// </summary>
	public enum SessionAuthentication
	{
		/// <summary>
		/// The default
		/// </summary>
		Default,
		/// <summary>
		/// The negotiate
		/// </summary>
		Negotiate,
		/// <summary>
		/// The kerberos
		/// </summary>
		Kerberos,
		/// <summary>
		/// The NTLM
		/// </summary>
		Ntlm
	}

	/// <summary>
	/// 
	/// </summary>
	public enum EventLogIsolation
	{
		/// <summary>
		/// The application
		/// </summary>
		Application,
		/// <summary>
		/// The system
		/// </summary>
		System,
		/// <summary>
		/// The custom
		/// </summary>
		Custom
	}

	/// <summary>
	/// 
	/// </summary>
	public enum EventLogMode
	{
		/// <summary>
		/// The circular
		/// </summary>
		Circular,
		/// <summary>
		/// The automatic backup
		/// </summary>
		AutoBackup,
		/// <summary>
		/// The retain
		/// </summary>
		Retain
	}

	/// <summary>
	/// 
	/// </summary>
	public enum EventLogType
	{
		/// <summary>
		/// The administrative
		/// </summary>
		Administrative,
		/// <summary>
		/// The operational
		/// </summary>
		Operational,
		/// <summary>
		/// The analytical
		/// </summary>
		Analytical,
		/// <summary>
		/// The debug
		/// </summary>
		Debug
	}

	/// <summary>
	/// Defines the standard keywords that are attached to events by the event provider. For more information about keywords, see <see cref="EventKeyword"/>.
	/// </summary>
	[Flags]
	public enum StandardEventKeywords : long
	{
		/// <summary>
		/// The audit failure
		/// </summary>
		AuditFailure =     0x10000000000000L,
		/// <summary>
		/// The audit success
		/// </summary>
		AuditSuccess =     0x20000000000000L,
		/// <summary>
		/// The correlation hint
		/// </summary>
		[Obsolete("Incorrect value: use CorrelationHint2 instead", false)]
		CorrelationHint =  0x10000000000000L,
		/// <summary>
		/// The correlation hint2
		/// </summary>
		CorrelationHint2 = 0x40000000000000L,
		/// <summary>
		/// The event log classic
		/// </summary>
		EventLogClassic =  0x80000000000000L,
		/// <summary>
		/// The none
		/// </summary>
		None =             0L,
		/// <summary>
		/// The response time
		/// </summary>
		ResponseTime =     0x01000000000000L,
		/// <summary>
		/// The SQM
		/// </summary>
		Sqm =              0x08000000000000L,
		/// <summary>
		/// The wdi context
		/// </summary>
		WdiContext =       0x02000000000000L,
		/// <summary>
		/// The wdi diagnostic
		/// </summary>
		WdiDiagnostic =    0x04000000000000L
	}

	/// <summary>
	/// Represents a placeholder (bookmark) within an event stream. You can use the placeholder to mark a position and return to this position in a stream of events. An instance of this object can be obtained from an EventRecord object, in which case it corresponds to the position of that event record.
	/// </summary>
	[Serializable, HostProtection(SecurityAction.LinkDemand, MayLeakOnAbort = true)]
	public class EventBookmark : ISerializable
	{
		// Fields
		private string bookmark;

		// Methods
		internal EventBookmark(string bookmarkText)
		{
			if (bookmarkText == null)
			{
				throw new ArgumentNullException("bookmarkText");
			}
			this.bookmark = bookmarkText;
		}

		/// <summary>
		/// Initializes a new instance of the <see cref="EventBookmark"/> class.
		/// </summary>
		/// <param name="info">The information.</param>
		/// <param name="context">The context.</param>
		protected EventBookmark(SerializationInfo info, StreamingContext context)
		{
			if (info == null)
			{
				throw new ArgumentNullException("info");
			}
			this.bookmark = info.GetString("BookmarkText");
		}

		// Properties
		internal string BookmarkText
		{
			get
			{
				return this.bookmark;
			}
		}

		[SecurityCritical, SecurityPermission(SecurityAction.LinkDemand, Flags = SecurityPermissionFlag.SerializationFormatter)]
		void ISerializable.GetObjectData(SerializationInfo info, StreamingContext context)
		{
			this.GetObjectData(info, context);
		}

		/// <summary>
		/// Gets the object data.
		/// </summary>
		/// <param name="info">The information.</param>
		/// <param name="context">The context.</param>
		[SecurityCritical, SecurityPermission(SecurityAction.LinkDemand, Flags = SecurityPermissionFlag.SerializationFormatter)]
		protected virtual void GetObjectData(SerializationInfo info, StreamingContext context)
		{
			if (info == null)
			{
				throw new ArgumentNullException("info");
			}
			info.AddValue("BookmarkText", this.bookmark);
		}
	}

	/// <summary>
	/// Represents a keyword for an event. Keywords are defined in an event provider and are used to group the event with other similar events (based on the usage of the events).
	/// </summary>
	[HostProtection(SecurityAction.LinkDemand, MayLeakOnAbort = true)]
	public sealed class EventKeyword
	{
		// Fields
		private bool dataReady;

		private string displayName;
		private string name;
		private ProviderMetadata pmReference;
		private object syncObject;
		private long value;

		// Methods
		internal EventKeyword(long value, ProviderMetadata pmReference)
		{
			this.value = value;
			this.pmReference = pmReference;
			this.syncObject = new object();
		}

		internal EventKeyword(string name, long value, string displayName)
		{
			this.value = value;
			this.name = name;
			this.displayName = displayName;
			this.dataReady = true;
			this.syncObject = new object();
		}

		// Properties
		/// <summary>
		/// Gets the display name.
		/// </summary>
		/// <value>
		/// The display name.
		/// </value>
		public string DisplayName
		{
			get
			{
				this.PrepareData();
				return this.displayName;
			}
		}

		/// <summary>
		/// Gets the name.
		/// </summary>
		/// <value>
		/// The name.
		/// </value>
		public string Name
		{
			get
			{
				this.PrepareData();
				return this.name;
			}
		}

		/// <summary>
		/// Gets the value.
		/// </summary>
		/// <value>
		/// The value.
		/// </value>
		public long Value
		{
			get
			{
				return this.value;
			}
		}

		internal void PrepareData()
		{
			if (!this.dataReady)
			{
				lock (this.syncObject)
				{
					if (!this.dataReady)
					{
						IEnumerable<EventKeyword> keywords = this.pmReference.Keywords;
						this.name = null;
						this.displayName = null;
						this.dataReady = true;
						foreach (EventKeyword keyword in keywords)
						{
							if (keyword.Value == this.value)
							{
								this.name = keyword.Name;
								this.displayName = keyword.DisplayName;
								break;
							}
						}
					}
				}
			}
		}
	}

	/// <summary>
	/// Contains an event level that is defined in an event provider. The level signifies the severity of the event.
	/// </summary>
	[HostProtection(SecurityAction.LinkDemand, MayLeakOnAbort = true)]
	public sealed class EventLevel
	{
		// Fields
		private bool dataReady;

		private string displayName;
		private string name;
		private ProviderMetadata pmReference;
		private object syncObject;
		private int value;

		// Methods
		internal EventLevel(int value, ProviderMetadata pmReference)
		{
			this.value = value;
			this.pmReference = pmReference;
			this.syncObject = new object();
		}

		internal EventLevel(string name, int value, string displayName)
		{
			this.value = value;
			this.name = name;
			this.displayName = displayName;
			this.dataReady = true;
			this.syncObject = new object();
		}

		// Properties
		/// <summary>
		/// Gets the display name.
		/// </summary>
		/// <value>
		/// The display name.
		/// </value>
		public string DisplayName
		{
			get
			{
				this.PrepareData();
				return this.displayName;
			}
		}

		/// <summary>
		/// Gets the name.
		/// </summary>
		/// <value>
		/// The name.
		/// </value>
		public string Name
		{
			get
			{
				this.PrepareData();
				return this.name;
			}
		}

		/// <summary>
		/// Gets the value.
		/// </summary>
		/// <value>
		/// The value.
		/// </value>
		public int Value
		{
			get
			{
				return this.value;
			}
		}

		internal void PrepareData()
		{
			if (!this.dataReady)
			{
				lock (this.syncObject)
				{
					if (!this.dataReady)
					{
						IEnumerable<EventLevel> levels = this.pmReference.Levels;
						this.name = null;
						this.displayName = null;
						this.dataReady = true;
						foreach (EventLevel level in levels)
						{
							if (level.Value == this.value)
							{
								this.name = level.Name;
								this.displayName = level.DisplayName;
								break;
							}
						}
					}
				}
			}
		}
	}

	/// <summary>
	/// EventLogConfiguration
	/// </summary>
	[HostProtection(SecurityAction.LinkDemand, MayLeakOnAbort = true)]
	public class EventLogConfiguration : IDisposable
	{
		private string channelName;
		private EventLogHandle handle;
		private EventLogSession session;

		/// <summary>
		/// Initializes a new instance of the <see cref="EventLogConfiguration"/> class.
		/// </summary>
		/// <param name="logName">Name of the log.</param>
		public EventLogConfiguration(string logName) : this(logName, null) { }

		/// <summary>
		/// Initializes a new instance of the <see cref="EventLogConfiguration"/> class.
		/// </summary>
		/// <param name="logName">Name of the log.</param>
		/// <param name="session">The session.</param>
		[SecurityCritical]
		public EventLogConfiguration(string logName, EventLogSession session)
		{
			this.handle = EventLogHandle.Zero;
			EventLogPermissionHolder.GetEventLogPermission().Demand();
			if (session == null)
			{
				session = EventLogSession.GlobalSession;
			}
			this.session = session;
			this.channelName = logName;
			this.handle = NativeWrapper.EvtOpenChannelConfig(this.session.Handle, this.channelName, 0);
		}

		/// <summary>
		/// Performs application-defined tasks associated with freeing, releasing, or resetting unmanaged resources.
		/// </summary>
		public void Dispose()
		{
			this.Dispose(true);
			GC.SuppressFinalize(this);
		}

		/// <summary>
		/// Releases unmanaged and - optionally - managed resources.
		/// </summary>
		/// <param name="disposing"><c>true</c> to release both managed and unmanaged resources; <c>false</c> to release only unmanaged resources.</param>
		[SecuritySafeCritical]
		protected virtual void Dispose(bool disposing)
		{
			if (disposing)
			{
				EventLogPermissionHolder.GetEventLogPermission().Demand();
			}
			if ((this.handle != null) && !this.handle.IsInvalid)
			{
				this.handle.Dispose();
			}
		}

		/// <summary>
		/// Saves the changes.
		/// </summary>
		public void SaveChanges()
		{
			NativeWrapper.EvtSaveChannelConfig(this.handle, 0);
		}

		// Properties
		/// <summary>
		/// Gets a value indicating whether this instance is classic log.
		/// </summary>
		/// <value>
		/// <c>true</c> if this instance is classic log; otherwise, <c>false</c>.
		/// </value>
		public bool IsClassicLog
		{
			get
			{
				return (bool)NativeWrapper.EvtGetChannelConfigProperty(this.handle, UnsafeNativeMethods.EvtChannelConfigPropertyId.EvtChannelConfigClassicEventlog);
			}
		}

		/// <summary>
		/// Gets or sets a value indicating whether this instance is enabled.
		/// </summary>
		/// <value>
		/// <c>true</c> if this instance is enabled; otherwise, <c>false</c>.
		/// </value>
		public bool IsEnabled
		{
			get
			{
				return (bool)NativeWrapper.EvtGetChannelConfigProperty(this.handle, UnsafeNativeMethods.EvtChannelConfigPropertyId.EvtChannelConfigEnabled);
			}
			set
			{
				NativeWrapper.EvtSetChannelConfigProperty(this.handle, UnsafeNativeMethods.EvtChannelConfigPropertyId.EvtChannelConfigEnabled, value);
			}
		}

		/// <summary>
		/// Gets or sets the log file path.
		/// </summary>
		/// <value>
		/// The log file path.
		/// </value>
		public string LogFilePath
		{
			get
			{
				return (string)NativeWrapper.EvtGetChannelConfigProperty(this.handle, UnsafeNativeMethods.EvtChannelConfigPropertyId.EvtChannelLoggingConfigLogFilePath);
			}
			set
			{
				NativeWrapper.EvtSetChannelConfigProperty(this.handle, UnsafeNativeMethods.EvtChannelConfigPropertyId.EvtChannelLoggingConfigLogFilePath, value);
			}
		}

		/// <summary>
		/// Gets the log isolation.
		/// </summary>
		/// <value>
		/// The log isolation.
		/// </value>
		public EventLogIsolation LogIsolation
		{
			get
			{
				return (EventLogIsolation)((uint)NativeWrapper.EvtGetChannelConfigProperty(this.handle, UnsafeNativeMethods.EvtChannelConfigPropertyId.EvtChannelConfigIsolation));
			}
		}

		/// <summary>
		/// Gets or sets the log mode.
		/// </summary>
		/// <value>
		/// The log mode.
		/// </value>
		public EventLogMode LogMode
		{
			get
			{
				object obj2 = NativeWrapper.EvtGetChannelConfigProperty(this.handle, UnsafeNativeMethods.EvtChannelConfigPropertyId.EvtChannelLoggingConfigRetention);
				object obj3 = NativeWrapper.EvtGetChannelConfigProperty(this.handle, UnsafeNativeMethods.EvtChannelConfigPropertyId.EvtChannelLoggingConfigAutoBackup);
				bool flag = (obj2 != null) && ((bool)obj2);
				if ((obj3 != null) && ((bool)obj3))
				{
					return EventLogMode.AutoBackup;
				}
				if (flag)
				{
					return EventLogMode.Retain;
				}
				return EventLogMode.Circular;
			}
			set
			{
				switch (value)
				{
					case EventLogMode.Circular:
						NativeWrapper.EvtSetChannelConfigProperty(this.handle, UnsafeNativeMethods.EvtChannelConfigPropertyId.EvtChannelLoggingConfigAutoBackup, false);
						NativeWrapper.EvtSetChannelConfigProperty(this.handle, UnsafeNativeMethods.EvtChannelConfigPropertyId.EvtChannelLoggingConfigRetention, false);
						return;
					case EventLogMode.AutoBackup:
						NativeWrapper.EvtSetChannelConfigProperty(this.handle, UnsafeNativeMethods.EvtChannelConfigPropertyId.EvtChannelLoggingConfigAutoBackup, true);
						NativeWrapper.EvtSetChannelConfigProperty(this.handle, UnsafeNativeMethods.EvtChannelConfigPropertyId.EvtChannelLoggingConfigRetention, true);
						return;
					case EventLogMode.Retain:
						NativeWrapper.EvtSetChannelConfigProperty(this.handle, UnsafeNativeMethods.EvtChannelConfigPropertyId.EvtChannelLoggingConfigAutoBackup, false);
						NativeWrapper.EvtSetChannelConfigProperty(this.handle, UnsafeNativeMethods.EvtChannelConfigPropertyId.EvtChannelLoggingConfigRetention, true);
						return;
				}
			}
		}

		/// <summary>
		/// Gets the name of the log.
		/// </summary>
		/// <value>
		/// The name of the log.
		/// </value>
		public string LogName
		{
			get
			{
				return this.channelName;
			}
		}

		/// <summary>
		/// Gets the type of the log.
		/// </summary>
		/// <value>
		/// The type of the log.
		/// </value>
		public EventLogType LogType
		{
			get
			{
				return (EventLogType)((uint)NativeWrapper.EvtGetChannelConfigProperty(this.handle, UnsafeNativeMethods.EvtChannelConfigPropertyId.EvtChannelConfigType));
			}
		}

		/// <summary>
		/// Gets or sets the maximum size in bytes.
		/// </summary>
		/// <value>
		/// The maximum size in bytes.
		/// </value>
		public long MaximumSizeInBytes
		{
			get
			{
				return (long)((ulong)NativeWrapper.EvtGetChannelConfigProperty(this.handle, UnsafeNativeMethods.EvtChannelConfigPropertyId.EvtChannelLoggingConfigMaxSize));
			}
			set
			{
				NativeWrapper.EvtSetChannelConfigProperty(this.handle, UnsafeNativeMethods.EvtChannelConfigPropertyId.EvtChannelLoggingConfigMaxSize, value);
			}
		}

		/// <summary>
		/// Gets the name of the owning provider.
		/// </summary>
		/// <value>
		/// The name of the owning provider.
		/// </value>
		public string OwningProviderName
		{
			get
			{
				return (string)NativeWrapper.EvtGetChannelConfigProperty(this.handle, UnsafeNativeMethods.EvtChannelConfigPropertyId.EvtChannelConfigOwningPublisher);
			}
		}

		/// <summary>
		/// Gets the size of the provider buffer.
		/// </summary>
		/// <value>
		/// The size of the provider buffer.
		/// </value>
		public int? ProviderBufferSize
		{
			get
			{
				uint? nullable = (uint?)NativeWrapper.EvtGetChannelConfigProperty(this.handle, UnsafeNativeMethods.EvtChannelConfigPropertyId.EvtChannelPublishingConfigBufferSize);
				if (!nullable.HasValue)
				{
					return null;
				}
				return new int?((int)nullable.GetValueOrDefault());
			}
		}

		/// <summary>
		/// Gets the provider control unique identifier.
		/// </summary>
		/// <value>
		/// The provider control unique identifier.
		/// </value>
		public Guid? ProviderControlGuid
		{
			get
			{
				return (Guid?)NativeWrapper.EvtGetChannelConfigProperty(this.handle, UnsafeNativeMethods.EvtChannelConfigPropertyId.EvtChannelPublishingConfigControlGuid);
			}
		}

		/// <summary>
		/// Gets or sets the provider keywords.
		/// </summary>
		/// <value>
		/// The provider keywords.
		/// </value>
		public long? ProviderKeywords
		{
			get
			{
				ulong? nullable = (ulong?)NativeWrapper.EvtGetChannelConfigProperty(this.handle, UnsafeNativeMethods.EvtChannelConfigPropertyId.EvtChannelPublishingConfigKeywords);
				if (!nullable.HasValue)
				{
					return null;
				}
				return new long?((long)nullable.GetValueOrDefault());
			}
			set
			{
				NativeWrapper.EvtSetChannelConfigProperty(this.handle, UnsafeNativeMethods.EvtChannelConfigPropertyId.EvtChannelPublishingConfigKeywords, value);
			}
		}

		/// <summary>
		/// Gets the provider latency.
		/// </summary>
		/// <value>
		/// The provider latency.
		/// </value>
		public int? ProviderLatency
		{
			get
			{
				uint? nullable = (uint?)NativeWrapper.EvtGetChannelConfigProperty(this.handle, UnsafeNativeMethods.EvtChannelConfigPropertyId.EvtChannelPublishingConfigLatency);
				if (!nullable.HasValue)
				{
					return null;
				}
				return new int?((int)nullable.GetValueOrDefault());
			}
		}

		/// <summary>
		/// Gets or sets the provider level.
		/// </summary>
		/// <value>
		/// The provider level.
		/// </value>
		public int? ProviderLevel
		{
			get
			{
				uint? nullable = (uint?)NativeWrapper.EvtGetChannelConfigProperty(this.handle, UnsafeNativeMethods.EvtChannelConfigPropertyId.EvtChannelPublishingConfigLevel);
				if (!nullable.HasValue)
				{
					return null;
				}
				return new int?((int)nullable.GetValueOrDefault());
			}
			set
			{
				NativeWrapper.EvtSetChannelConfigProperty(this.handle, UnsafeNativeMethods.EvtChannelConfigPropertyId.EvtChannelPublishingConfigLevel, value);
			}
		}

		/// <summary>
		/// Gets the provider maximum number of buffers.
		/// </summary>
		/// <value>
		/// The provider maximum number of buffers.
		/// </value>
		public int? ProviderMaximumNumberOfBuffers
		{
			get
			{
				uint? nullable = (uint?)NativeWrapper.EvtGetChannelConfigProperty(this.handle, UnsafeNativeMethods.EvtChannelConfigPropertyId.EvtChannelPublishingConfigMaxBuffers);
				if (!nullable.HasValue)
				{
					return null;
				}
				return new int?((int)nullable.GetValueOrDefault());
			}
		}

		/// <summary>
		/// Gets the provider minimum number of buffers.
		/// </summary>
		/// <value>
		/// The provider minimum number of buffers.
		/// </value>
		public int? ProviderMinimumNumberOfBuffers
		{
			get
			{
				uint? nullable = (uint?)NativeWrapper.EvtGetChannelConfigProperty(this.handle, UnsafeNativeMethods.EvtChannelConfigPropertyId.EvtChannelPublishingConfigMinBuffers);
				if (!nullable.HasValue)
				{
					return null;
				}
				return new int?((int)nullable.GetValueOrDefault());
			}
		}

		/// <summary>
		/// Gets the provider names.
		/// </summary>
		/// <value>
		/// The provider names.
		/// </value>
		public IEnumerable<string> ProviderNames
		{
			get
			{
				return (string[])NativeWrapper.EvtGetChannelConfigProperty(this.handle, UnsafeNativeMethods.EvtChannelConfigPropertyId.EvtChannelPublisherList);
			}
		}

		/// <summary>
		/// Gets or sets the security descriptor.
		/// </summary>
		/// <value>
		/// The security descriptor.
		/// </value>
		public string SecurityDescriptor
		{
			get
			{
				return (string)NativeWrapper.EvtGetChannelConfigProperty(this.handle, UnsafeNativeMethods.EvtChannelConfigPropertyId.EvtChannelConfigAccess);
			}
			set
			{
				NativeWrapper.EvtSetChannelConfigProperty(this.handle, UnsafeNativeMethods.EvtChannelConfigPropertyId.EvtChannelConfigAccess, value);
			}
		}
	}

	[Serializable, HostProtection(SecurityAction.LinkDemand, MayLeakOnAbort = true)]
	internal class EventLogException : Exception, ISerializable
	{
		// Fields
		private int errorCode;

		// Methods
		public EventLogException()
		{
		}

		public EventLogException(string message)
			: base(message)
		{
		}

		public EventLogException(string message, Exception innerException)
			: base(message, innerException)
		{
		}

		protected EventLogException(int errorCode)
		{
			this.errorCode = errorCode;
		}
		protected EventLogException(SerializationInfo serializationInfo, StreamingContext streamingContext)
			: base(serializationInfo, streamingContext)
		{
		}
		// Properties
		public override string Message
		{
			[SecurityCritical]
			get
			{
				EventLogPermissionHolder.GetEventLogPermission().Demand();
				Win32Exception exception = new Win32Exception(this.errorCode);
				return exception.Message;
			}
		}

		[SecurityCritical, SecurityPermission(SecurityAction.LinkDemand, Flags = SecurityPermissionFlag.SerializationFormatter)]
		public override void GetObjectData(SerializationInfo info, StreamingContext context)
		{
			if (info == null)
			{
				throw new ArgumentNullException("info");
			}
			info.AddValue("errorCode", this.errorCode);
			base.GetObjectData(info, context);
		}

		internal static void Throw(int errorCode)
		{
			switch (errorCode)
			{
				case 0x4c7:
				case 0x71a:
					throw new OperationCanceledException();

				case 2:
				case 3:
				case 0x3a9f:
				case 0x3a9a:
				case 0x3ab3:
				case 0x3ab4:
					throw new EventLogNotFoundException(errorCode);

				case 5:
					throw new UnauthorizedAccessException();

				case 13:
				case 0x3a9d:
					throw new EventLogInvalidDataException(errorCode);

				case 0x3aa3:
				case 0x3aa4:
					throw new EventLogReadingException(errorCode);

				case 0x3abd:
					throw new EventLogProviderDisabledException(errorCode);
			}
			throw new EventLogException(errorCode);
		}
	}

	/// <summary>
	/// Allows you to access the run-time properties of active event logs and event log files. These properties include the number of events in the log, the size of the log, a value that determines whether the log is full, and the last time the log was written to or accessed.
	/// </summary>
	[HostProtection(SecurityAction.LinkDemand, MayLeakOnAbort = true)]
	public sealed class EventLogInformation
	{
		// Fields
		private DateTime? creationTime;

		private int? fileAttributes;
		private long? fileSize;
		private bool? isLogFull;
		private DateTime? lastAccessTime;
		private DateTime? lastWriteTime;
		private long? oldestRecordNumber;
		private long? recordCount;

		// Methods
		[SecurityTreatAsSafe, SecurityCritical]
		internal EventLogInformation(EventLogSession session, string channelName, PathType pathType)
		{
			EventLogPermissionHolder.GetEventLogPermission().Demand();
			EventLogHandle handle = NativeWrapper.EvtOpenLog(session.Handle, channelName, pathType);
			using (handle)
			{
				this.creationTime = (DateTime?)NativeWrapper.EvtGetLogInfo(handle, UnsafeNativeMethods.EvtLogPropertyId.EvtLogCreationTime);
				this.lastAccessTime = (DateTime?)NativeWrapper.EvtGetLogInfo(handle, UnsafeNativeMethods.EvtLogPropertyId.EvtLogLastAccessTime);
				this.lastWriteTime = (DateTime?)NativeWrapper.EvtGetLogInfo(handle, UnsafeNativeMethods.EvtLogPropertyId.EvtLogLastWriteTime);
				ulong? nullable = (ulong?)NativeWrapper.EvtGetLogInfo(handle, UnsafeNativeMethods.EvtLogPropertyId.EvtLogFileSize);
				this.fileSize = nullable.HasValue ? (long?)(nullable.GetValueOrDefault()) : null;
				uint? nullable3 = (uint?)NativeWrapper.EvtGetLogInfo(handle, UnsafeNativeMethods.EvtLogPropertyId.EvtLogAttributes);
				this.fileAttributes = nullable3.HasValue ? (int?)(nullable3.GetValueOrDefault()) : null;
				ulong? nullable5 = (ulong?)NativeWrapper.EvtGetLogInfo(handle, UnsafeNativeMethods.EvtLogPropertyId.EvtLogNumberOfLogRecords);
				this.recordCount = nullable5.HasValue ? (long?)(nullable5.GetValueOrDefault()) : null;
				ulong? nullable7 = (ulong?)NativeWrapper.EvtGetLogInfo(handle, UnsafeNativeMethods.EvtLogPropertyId.EvtLogOldestRecordNumber);
				this.oldestRecordNumber = nullable7.HasValue ? (long?)(nullable7.GetValueOrDefault()) : null;
				this.isLogFull = (bool?)NativeWrapper.EvtGetLogInfo(handle, UnsafeNativeMethods.EvtLogPropertyId.EvtLogFull);
			}
		}

		// Properties
		/// <summary>
		/// Gets the attributes.
		/// </summary>
		/// <value>
		/// The attributes.
		/// </value>
		public int? Attributes
		{
			get
			{
				return this.fileAttributes;
			}
		}

		/// <summary>
		/// Gets the creation time.
		/// </summary>
		/// <value>
		/// The creation time.
		/// </value>
		public DateTime? CreationTime
		{
			get
			{
				return this.creationTime;
			}
		}

		/// <summary>
		/// Gets the size of the file.
		/// </summary>
		/// <value>
		/// The size of the file.
		/// </value>
		public long? FileSize
		{
			get
			{
				return this.fileSize;
			}
		}

		/// <summary>
		/// Gets the is log full.
		/// </summary>
		/// <value>
		/// The is log full.
		/// </value>
		public bool? IsLogFull
		{
			get
			{
				return this.isLogFull;
			}
		}

		/// <summary>
		/// Gets the last access time.
		/// </summary>
		/// <value>
		/// The last access time.
		/// </value>
		public DateTime? LastAccessTime
		{
			get
			{
				return this.lastAccessTime;
			}
		}

		/// <summary>
		/// Gets the last write time.
		/// </summary>
		/// <value>
		/// The last write time.
		/// </value>
		public DateTime? LastWriteTime
		{
			get
			{
				return this.lastWriteTime;
			}
		}

		/// <summary>
		/// Gets the oldest record number.
		/// </summary>
		/// <value>
		/// The oldest record number.
		/// </value>
		public long? OldestRecordNumber
		{
			get
			{
				return this.oldestRecordNumber;
			}
		}

		/// <summary>
		/// Gets the record count.
		/// </summary>
		/// <value>
		/// The record count.
		/// </value>
		public long? RecordCount
		{
			get
			{
				return this.recordCount;
			}
		}
	}

	[Serializable, HostProtection(SecurityAction.LinkDemand, MayLeakOnAbort = true)]
	internal class EventLogInvalidDataException : EventLogException
	{
		// Methods
		public EventLogInvalidDataException()
		{
		}

		public EventLogInvalidDataException(string message)
			: base(message)
		{
		}

		public EventLogInvalidDataException(string message, Exception innerException)
			: base(message, innerException)
		{
		}

		internal EventLogInvalidDataException(int errorCode)
			: base(errorCode)
		{
		}
		protected EventLogInvalidDataException(SerializationInfo serializationInfo, StreamingContext streamingContext)
			: base(serializationInfo, streamingContext)
		{
		}
	}

	/// <summary>
	/// Represents a link between an event provider and an event log that the provider publishes events into. This object cannot be instantiated.
	/// </summary>
	[HostProtection(SecurityAction.LinkDemand, MayLeakOnAbort = true)]
	public sealed class EventLogLink
	{
		// Fields
		private uint channelId;

		private string channelName;
		private bool dataReady;
		private string displayName;
		private bool isImported;
		private ProviderMetadata pmReference;
		private object syncObject;

		// Methods
		internal EventLogLink(uint channelId, ProviderMetadata pmReference)
		{
			this.channelId = channelId;
			this.pmReference = pmReference;
			this.syncObject = new object();
		}

		internal EventLogLink(string channelName, bool isImported, string displayName, uint channelId)
		{
			this.channelName = channelName;
			this.isImported = isImported;
			this.displayName = displayName;
			this.channelId = channelId;
			this.dataReady = true;
			this.syncObject = new object();
		}

		/// <summary>
		/// Gets the display name.
		/// </summary>
		/// <value>
		/// The display name.
		/// </value>
		public string DisplayName
		{
			get
			{
				this.PrepareData();
				return this.displayName;
			}
		}

		/// <summary>
		/// Gets a value indicating whether this instance is imported.
		/// </summary>
		/// <value>
		/// 	<c>true</c> if this instance is imported; otherwise, <c>false</c>.
		/// </value>
		public bool IsImported
		{
			get
			{
				this.PrepareData();
				return this.isImported;
			}
		}

		/// <summary>
		/// Gets the name of the log.
		/// </summary>
		/// <value>
		/// The name of the log.
		/// </value>
		public string LogName
		{
			get
			{
				this.PrepareData();
				return this.channelName;
			}
		}

		// Properties
		internal uint ChannelId
		{
			get
			{
				return this.channelId;
			}
		}

		private void PrepareData()
		{
			if (!this.dataReady)
			{
				lock (this.syncObject)
				{
					if (!this.dataReady)
					{
						IEnumerable<EventLogLink> logLinks = this.pmReference.LogLinks;
						this.channelName = null;
						this.isImported = false;
						this.displayName = null;
						this.dataReady = true;
						foreach (EventLogLink link in logLinks)
						{
							if (link.ChannelId == this.channelId)
							{
								this.channelName = link.LogName;
								this.isImported = link.IsImported;
								this.displayName = link.DisplayName;
								this.dataReady = true;
								break;
							}
						}
					}
				}
			}
		}
	}

	[Serializable, HostProtection(SecurityAction.LinkDemand, MayLeakOnAbort = true)]
	internal class EventLogNotFoundException : EventLogException
	{
		// Methods
		public EventLogNotFoundException()
		{
		}

		public EventLogNotFoundException(string message)
			: base(message)
		{
		}

		public EventLogNotFoundException(string message, Exception innerException)
			: base(message, innerException)
		{
		}

		internal EventLogNotFoundException(int errorCode)
			: base(errorCode)
		{
		}
		protected EventLogNotFoundException(SerializationInfo serializationInfo, StreamingContext streamingContext)
			: base(serializationInfo, streamingContext)
		{
		}
	}

	/// <summary>
	/// Contains an array of strings that represent XPath queries for elements in the XML representation of an event, which is based on the Event Schema. The queries in this object are used to extract values from the event.
	/// </summary>
	[HostProtection(SecurityAction.LinkDemand, MayLeakOnAbort = true)]
	public class EventLogPropertySelector : IDisposable
	{
		// Fields
		private EventLogHandle renderContextHandleValues;

		// Methods
		/// <summary>
		/// Initializes a new instance of the <see cref="EventLogPropertySelector"/> class.
		/// </summary>
		/// <param name="propertyQueries">The property queries.</param>
		[SecurityCritical]
		public EventLogPropertySelector(IEnumerable<string> propertyQueries)
		{
			string[] strArray;
			EventLogPermissionHolder.GetEventLogPermission().Demand();
			if (propertyQueries == null)
			{
				throw new ArgumentNullException("propertyQueries");
			}
			ICollection<string> is2 = propertyQueries as ICollection<string>;
			if (is2 != null)
			{
				strArray = new string[is2.Count];
				is2.CopyTo(strArray, 0);
			}
			else
			{
				strArray = new List<string>(propertyQueries).ToArray();
			}
			this.renderContextHandleValues = NativeWrapper.EvtCreateRenderContext(strArray.Length, strArray, UnsafeNativeMethods.EvtRenderContextFlags.EvtRenderContextValues);
		}

		// Properties
		internal EventLogHandle Handle
		{
			get
			{
				return this.renderContextHandleValues;
			}
		}

		/// <summary>
		/// Releases unmanaged and - optionally - managed resources.
		/// </summary>
		public void Dispose()
		{
			this.Dispose(true);
			GC.SuppressFinalize(this);
		}

		/// <summary>
		/// Releases unmanaged and - optionally - managed resources.
		/// </summary>
		/// <param name="disposing"><c>true</c> to release both managed and unmanaged resources; <c>false</c> to release only unmanaged resources.</param>
		[SecurityTreatAsSafe, SecurityCritical]
		protected virtual void Dispose(bool disposing)
		{
			if (disposing)
			{
				EventLogPermissionHolder.GetEventLogPermission().Demand();
			}
			if ((this.renderContextHandleValues != null) && !this.renderContextHandleValues.IsInvalid)
			{
				this.renderContextHandleValues.Dispose();
			}
		}
	}

	[Serializable, HostProtection(SecurityAction.LinkDemand, MayLeakOnAbort = true)]
	internal class EventLogProviderDisabledException : EventLogException
	{
		// Methods
		public EventLogProviderDisabledException()
		{
		}

		public EventLogProviderDisabledException(string message)
			: base(message)
		{
		}

		public EventLogProviderDisabledException(string message, Exception innerException)
			: base(message, innerException)
		{
		}

		internal EventLogProviderDisabledException(int errorCode)
			: base(errorCode)
		{
		}
		protected EventLogProviderDisabledException(SerializationInfo serializationInfo, StreamingContext streamingContext)
			: base(serializationInfo, streamingContext)
		{
		}
	}

	internal class EventLogQuery
	{
		// Fields
		private string path;

		private PathType pathType;
		private string query;
		private bool reverseDirection;
		private EventLogSession session;
		private bool tolerateErrors;

		// Methods
		public EventLogQuery(string path, PathType pathType)
			: this(path, pathType, null)
		{
		}

		public EventLogQuery(string path, PathType pathType, string query)
		{
			this.session = EventLogSession.GlobalSession;
			this.path = path;
			this.pathType = pathType;
			if (query == null)
			{
				if (path == null)
				{
					throw new ArgumentNullException("path");
				}
			}
			else
			{
				this.query = query;
			}
		}

		public bool ReverseDirection
		{
			get
			{
				return this.reverseDirection;
			}
			set
			{
				this.reverseDirection = value;
			}
		}

		public EventLogSession Session
		{
			get
			{
				return this.session;
			}
			set
			{
				this.session = value;
			}
		}

		public bool TolerateQueryErrors
		{
			get
			{
				return this.tolerateErrors;
			}
			set
			{
				this.tolerateErrors = value;
			}
		}

		// Properties
		internal string Path
		{
			get
			{
				return this.path;
			}
		}

		internal string Query
		{
			get
			{
				return this.query;
			}
		}

		internal PathType ThePathType
		{
			get
			{
				return this.pathType;
			}
		}
	}

	[HostProtection(SecurityAction.LinkDemand, MayLeakOnAbort = true), HostProtection(SecurityAction.LinkDemand, MayLeakOnAbort = true)]
	internal class EventLogReader : IDisposable
	{
		// Fields
		private int batchSize;

		private ProviderMetadataCachedInformation cachedMetadataInformation;
		private int currentIndex;
		private int eventCount;
		private EventLogQuery eventQuery;
		private IntPtr[] eventsBuffer;
		private EventLogHandle handle;
		private bool isEof;

		// Methods
		public EventLogReader(EventLogQuery eventQuery)
			: this(eventQuery, null)
		{
		}

		public EventLogReader(string path)
			: this(new EventLogQuery(path, PathType.LogName), null)
		{
		}

		[SecurityCritical]
		public EventLogReader(EventLogQuery eventQuery, EventBookmark bookmark)
		{
			if (eventQuery == null)
			{
				throw new ArgumentNullException("eventQuery");
			}
			string logfile = null;
			if (eventQuery.ThePathType == PathType.FilePath)
			{
				logfile = eventQuery.Path;
			}
			this.cachedMetadataInformation = new ProviderMetadataCachedInformation(eventQuery.Session, logfile, 50);
			this.eventQuery = eventQuery;
			this.batchSize = 0x40;
			this.eventsBuffer = new IntPtr[this.batchSize];
			int flags = 0;
			if (this.eventQuery.ThePathType == PathType.LogName)
			{
				flags |= 1;
			}
			else
			{
				flags |= 2;
			}
			if (this.eventQuery.ReverseDirection)
			{
				flags |= 0x200;
			}
			if (this.eventQuery.TolerateQueryErrors)
			{
				flags |= 0x1000;
			}
			EventLogPermissionHolder.GetEventLogPermission().Demand();
			this.handle = NativeWrapper.EvtQuery(this.eventQuery.Session.Handle, this.eventQuery.Path, this.eventQuery.Query, flags);
			EventLogHandle bookmarkHandleFromBookmark = EventLogRecord.GetBookmarkHandleFromBookmark(bookmark);
			if (!bookmarkHandleFromBookmark.IsInvalid)
			{
				using (bookmarkHandleFromBookmark)
				{
					NativeWrapper.EvtSeek(this.handle, 1L, bookmarkHandleFromBookmark, 0, UnsafeNativeMethods.EvtSeekFlags.EvtSeekRelativeToBookmark);
				}
			}
		}

		public EventLogReader(string path, PathType pathType)
			: this(new EventLogQuery(path, pathType), null)
		{
		}

		// Properties
		public int BatchSize
		{
			get
			{
				return this.batchSize;
			}
			set
			{
				if (value < 1)
				{
					throw new ArgumentOutOfRangeException("value");
				}
				this.batchSize = value;
			}
		}

		public IList<EventLogStatus> LogStatus
		{
			[SecurityCritical]
			get
			{
				EventLogPermissionHolder.GetEventLogPermission().Demand();
				List<EventLogStatus> list = null;
				string[] strArray = null;
				int[] numArray = null;
				EventLogHandle handle = this.handle;
				if (handle.IsInvalid)
				{
					throw new InvalidOperationException();
				}
				strArray = (string[])NativeWrapper.EvtGetQueryInfo(handle, UnsafeNativeMethods.EvtQueryPropertyId.EvtQueryNames);
				numArray = (int[])NativeWrapper.EvtGetQueryInfo(handle, UnsafeNativeMethods.EvtQueryPropertyId.EvtQueryStatuses);
				if (strArray.Length != numArray.Length)
				{
					throw new InvalidOperationException();
				}
				list = new List<EventLogStatus>(strArray.Length);
				for (int i = 0; i < strArray.Length; i++)
				{
					EventLogStatus item = new EventLogStatus(strArray[i], numArray[i]);
					list.Add(item);
				}
				return list.AsReadOnly();
			}
		}

		public void CancelReading()
		{
			NativeWrapper.EvtCancel(this.handle);
		}

		public void Dispose()
		{
			this.Dispose(true);
			GC.SuppressFinalize(this);
		}

		public EventRecord ReadEvent()
		{
			return this.ReadEvent(TimeSpan.MaxValue);
		}

		[SecurityCritical]
		public EventRecord ReadEvent(TimeSpan timeout)
		{
			EventLogPermissionHolder.GetEventLogPermission().Demand();
			if (this.isEof)
			{
				throw new InvalidOperationException();
			}
			if (this.currentIndex >= this.eventCount)
			{
				this.GetNextBatch(timeout);
				if (this.currentIndex >= this.eventCount)
				{
					this.isEof = true;
					return null;
				}
			}
			EventLogRecord record = new EventLogRecord(new EventLogHandle(this.eventsBuffer[this.currentIndex], true), this.eventQuery.Session, this.cachedMetadataInformation);
			this.currentIndex++;
			return record;
		}

		public void Seek(EventBookmark bookmark)
		{
			this.Seek(bookmark, 0L);
		}

		[SecurityCritical]
		public void Seek(EventBookmark bookmark, long offset)
		{
			if (bookmark == null)
			{
				throw new ArgumentNullException("bookmark");
			}
			EventLogPermissionHolder.GetEventLogPermission().Demand();
			this.SeekReset();
			using (EventLogHandle handle = EventLogRecord.GetBookmarkHandleFromBookmark(bookmark))
			{
				NativeWrapper.EvtSeek(this.handle, offset, handle, 0, UnsafeNativeMethods.EvtSeekFlags.EvtSeekRelativeToBookmark);
			}
		}

		[SecurityCritical]
		public void Seek(SeekOrigin origin, long offset)
		{
			EventLogPermissionHolder.GetEventLogPermission().Demand();
			switch (origin)
			{
				case SeekOrigin.Begin:
					this.SeekReset();
					NativeWrapper.EvtSeek(this.handle, offset, EventLogHandle.Zero, 0, UnsafeNativeMethods.EvtSeekFlags.EvtSeekRelativeToFirst);
					return;

				case SeekOrigin.Current:
					if (offset < 0L)
					{
						if ((this.currentIndex + offset) >= 0L)
						{
							this.SeekCommon(offset);
						}
						else
						{
							this.SeekCommon(offset);
						}
						return;
					}
					if ((this.currentIndex + offset) >= this.eventCount)
					{
						this.SeekCommon(offset);
						return;
					}
					for (int i = this.currentIndex; i < (this.currentIndex + offset); i++)
					{
						NativeWrapper.EvtClose(this.eventsBuffer[i]);
					}
					this.currentIndex += (int)offset;
					return;

				case SeekOrigin.End:
					this.SeekReset();
					NativeWrapper.EvtSeek(this.handle, offset, EventLogHandle.Zero, 0, UnsafeNativeMethods.EvtSeekFlags.EvtSeekRelativeToLast);
					return;
			}
		}

		[SecurityCritical]
		internal void SeekCommon(long offset)
		{
			offset -= this.eventCount - this.currentIndex;
			this.SeekReset();
			NativeWrapper.EvtSeek(this.handle, offset, EventLogHandle.Zero, 0, UnsafeNativeMethods.EvtSeekFlags.EvtSeekRelativeToCurrent);
		}

		[SecurityCritical]
		internal void SeekReset()
		{
			while (this.currentIndex < this.eventCount)
			{
				NativeWrapper.EvtClose(this.eventsBuffer[this.currentIndex]);
				this.currentIndex++;
			}
			this.currentIndex = 0;
			this.eventCount = 0;
			this.isEof = false;
		}

		[SecurityCritical, SecurityTreatAsSafe]
		protected virtual void Dispose(bool disposing)
		{
			if (disposing)
			{
				EventLogPermissionHolder.GetEventLogPermission().Demand();
			}
			while (this.currentIndex < this.eventCount)
			{
				NativeWrapper.EvtClose(this.eventsBuffer[this.currentIndex]);
				this.currentIndex++;
			}
			if ((this.handle != null) && !this.handle.IsInvalid)
			{
				this.handle.Dispose();
			}
		}

		[SecurityCritical]
		private bool GetNextBatch(TimeSpan ts)
		{
			int totalMilliseconds;
			if (ts == TimeSpan.MaxValue)
			{
				totalMilliseconds = -1;
			}
			else
			{
				totalMilliseconds = (int)ts.TotalMilliseconds;
			}
			if (this.batchSize != this.eventsBuffer.Length)
			{
				this.eventsBuffer = new IntPtr[this.batchSize];
			}
			int returned = 0;
			if (!NativeWrapper.EvtNext(this.handle, this.batchSize, this.eventsBuffer, totalMilliseconds, 0, ref returned))
			{
				this.eventCount = 0;
				this.currentIndex = 0;
				return false;
			}
			this.currentIndex = 0;
			this.eventCount = returned;
			return true;
		}
	}

	[Serializable, HostProtection(SecurityAction.LinkDemand, MayLeakOnAbort = true)]
	internal class EventLogReadingException : EventLogException
	{
		// Methods
		public EventLogReadingException()
		{
		}

		public EventLogReadingException(string message)
			: base(message)
		{
		}

		public EventLogReadingException(string message, Exception innerException)
			: base(message, innerException)
		{
		}

		internal EventLogReadingException(int errorCode)
			: base(errorCode)
		{
		}
		protected EventLogReadingException(SerializationInfo serializationInfo, StreamingContext streamingContext)
			: base(serializationInfo, streamingContext)
		{
		}
	}

	/// <summary>
	/// Enables you to read events from an event log based on an event query. The events that are read by this object are returned as EventRecord objects.
	/// </summary>
	[HostProtection(SecurityAction.LinkDemand, MayLeakOnAbort = true)]
	public class EventLogRecord : EventRecord
	{
		private const int SYSTEM_PROPERTY_COUNT = 0x12;

		private ProviderMetadataCachedInformation cachedMetadataInformation;
		private string containerChannel;
		[SecurityTreatAsSafe]
		private EventLogHandle handle;
		private IEnumerable<string> keywordsNames;
		private string levelName;
		private bool levelNameReady;
		private int[] matchedQueryIds;
		private string opcodeName;
		private bool opcodeNameReady;
		private EventLogSession session;
		private object syncObject;
		private NativeWrapper.SystemProperties systemProperties;
		private string taskName;
		private bool taskNameReady;

		[SecurityTreatAsSafe]
		internal EventLogRecord(EventLogHandle handle, EventLogSession session, ProviderMetadataCachedInformation cachedMetadataInfo)
		{
			this.cachedMetadataInformation = cachedMetadataInfo;
			this.handle = handle;
			this.session = session;
			this.systemProperties = new NativeWrapper.SystemProperties();
			this.syncObject = new object();
		}

		/// <summary>
		/// Gets the activity identifier.
		/// </summary>
		/// <value>
		/// The activity identifier.
		/// </value>
		public override Guid? ActivityId
		{
			get
			{
				this.PrepareSystemData();
				return this.systemProperties.ActivityId;
			}
		}

		/// <summary>
		/// Gets the bookmark.
		/// </summary>
		/// <value>
		/// The bookmark.
		/// </value>
		public override EventBookmark Bookmark
		{
			[SecurityTreatAsSafe, SecurityCritical]
			get
			{
				EventLogPermissionHolder.GetEventLogPermission().Demand();
				EventLogHandle bookmark = NativeWrapper.EvtCreateBookmark(null);
				NativeWrapper.EvtUpdateBookmark(bookmark, this.handle);
				return new EventBookmark(NativeWrapper.EvtRenderBookmark(bookmark));
			}
		}

		/// <summary>
		/// Gets the container log.
		/// </summary>
		/// <value>
		/// The container log.
		/// </value>
		public string ContainerLog
		{
			get
			{
				if (this.containerChannel != null)
				{
					return this.containerChannel;
				}
				lock (this.syncObject)
				{
					if (this.containerChannel == null)
					{
						this.containerChannel = (string)NativeWrapper.EvtGetEventInfo(this.Handle, UnsafeNativeMethods.EvtEventPropertyId.EvtEventPath);
					}
					return this.containerChannel;
				}
			}
		}

		/// <summary>
		/// Gets the identifier.
		/// </summary>
		/// <value>
		/// The identifier.
		/// </value>
		public override int Id
		{
			get
			{
				this.PrepareSystemData();
				ushort? id = this.systemProperties.Id;
				int? nullable3 = id.HasValue ? new int?(id.GetValueOrDefault()) : null;
				if (!nullable3.HasValue)
				{
					return 0;
				}
				return this.systemProperties.Id.Value;
			}
		}

		/// <summary>
		/// Gets the keywords.
		/// </summary>
		/// <value>
		/// The keywords.
		/// </value>
		public override long? Keywords
		{
			get
			{
				this.PrepareSystemData();
				ulong? keywords = this.systemProperties.Keywords;
				if (!keywords.HasValue)
				{
					return null;
				}
				return (long)(keywords.GetValueOrDefault());
			}
		}

		/// <summary>
		/// Gets the keywords display names.
		/// </summary>
		/// <value>
		/// The keywords display names.
		/// </value>
		public override IEnumerable<string> KeywordsDisplayNames
		{
			get
			{
				if (this.keywordsNames != null)
				{
					return this.keywordsNames;
				}
				lock (this.syncObject)
				{
					if (this.keywordsNames == null)
					{
						this.keywordsNames = this.cachedMetadataInformation.GetKeywordDisplayNames(this.ProviderName, this.handle);
					}
					return this.keywordsNames;
				}
			}
		}

		/// <summary>
		/// Gets the level.
		/// </summary>
		/// <value>
		/// The level.
		/// </value>
		public override byte? Level
		{
			get
			{
				this.PrepareSystemData();
				return this.systemProperties.Level;
			}
		}

		/// <summary>
		/// Gets the display name of the level.
		/// </summary>
		/// <value>
		/// The display name of the level.
		/// </value>
		public override string LevelDisplayName
		{
			get
			{
				if (this.levelNameReady)
				{
					return this.levelName;
				}
				lock (this.syncObject)
				{
					if (!this.levelNameReady)
					{
						this.levelNameReady = true;
						this.levelName = this.cachedMetadataInformation.GetLevelDisplayName(this.ProviderName, this.handle);
					}
					return this.levelName;
				}
			}
		}

		/// <summary>
		/// Gets the name of the log.
		/// </summary>
		/// <value>
		/// The name of the log.
		/// </value>
		public override string LogName
		{
			get
			{
				this.PrepareSystemData();
				return this.systemProperties.ChannelName;
			}
		}

		/// <summary>
		/// Gets the name of the machine.
		/// </summary>
		/// <value>
		/// The name of the machine.
		/// </value>
		public override string MachineName
		{
			get
			{
				this.PrepareSystemData();
				return this.systemProperties.ComputerName;
			}
		}

		/// <summary>
		/// Gets the matched query ids.
		/// </summary>
		/// <value>
		/// The matched query ids.
		/// </value>
		public IEnumerable<int> MatchedQueryIds
		{
			get
			{
				if (this.matchedQueryIds != null)
				{
					return this.matchedQueryIds;
				}
				lock (this.syncObject)
				{
					if (this.matchedQueryIds == null)
					{
						this.matchedQueryIds = (int[])NativeWrapper.EvtGetEventInfo(this.Handle, UnsafeNativeMethods.EvtEventPropertyId.EvtEventQueryIDs);
					}
					return this.matchedQueryIds;
				}
			}
		}

		/// <summary>
		/// Gets the opcode.
		/// </summary>
		/// <value>
		/// The opcode.
		/// </value>
		public override short? Opcode
		{
			get
			{
				this.PrepareSystemData();
				byte? opcode = this.systemProperties.Opcode;
				ushort? nullable3 = opcode.HasValue ? new ushort?(opcode.GetValueOrDefault()) : null;
				if (!nullable3.HasValue)
				{
					return null;
				}
				return new short?((short)nullable3.GetValueOrDefault());
			}
		}

		/// <summary>
		/// Gets the display name of the opcode.
		/// </summary>
		/// <value>
		/// The display name of the opcode.
		/// </value>
		public override string OpcodeDisplayName
		{
			get
			{
				lock (this.syncObject)
				{
					if (!this.opcodeNameReady)
					{
						this.opcodeNameReady = true;
						this.opcodeName = this.cachedMetadataInformation.GetOpcodeDisplayName(this.ProviderName, this.handle);
					}
					return this.opcodeName;
				}
			}
		}

		/// <summary>
		/// Gets the process identifier.
		/// </summary>
		/// <value>
		/// The process identifier.
		/// </value>
		public override int? ProcessId
		{
			get
			{
				this.PrepareSystemData();
				uint? processId = this.systemProperties.ProcessId;
				if (!processId.HasValue)
				{
					return null;
				}
				return (int)processId.GetValueOrDefault();
			}
		}

		/// <summary>
		/// Gets the properties.
		/// </summary>
		/// <value>
		/// The properties.
		/// </value>
		public override IList<EventProperty> Properties
		{
			get
			{
				this.session.SetupUserContext();
				IList<object> list = NativeWrapper.EvtRenderBufferWithContextUserOrValues(this.session.renderContextHandleUser, this.handle);
				List<EventProperty> list2 = new List<EventProperty>();
				foreach (object obj2 in list)
				{
					list2.Add(new EventProperty(obj2));
				}
				return list2;
			}
		}

		/// <summary>
		/// Gets the provider identifier.
		/// </summary>
		/// <value>
		/// The provider identifier.
		/// </value>
		public override Guid? ProviderId
		{
			get
			{
				this.PrepareSystemData();
				return this.systemProperties.ProviderId;
			}
		}

		/// <summary>
		/// Gets the name of the provider.
		/// </summary>
		/// <value>
		/// The name of the provider.
		/// </value>
		public override string ProviderName
		{
			get
			{
				this.PrepareSystemData();
				return this.systemProperties.ProviderName;
			}
		}

		/// <summary>
		/// Gets the qualifiers.
		/// </summary>
		/// <value>
		/// The qualifiers.
		/// </value>
		public override int? Qualifiers
		{
			get
			{
				this.PrepareSystemData();
				ushort? qualifiers = this.systemProperties.Qualifiers;
				uint? nullable3 = qualifiers.HasValue ? new uint?(qualifiers.GetValueOrDefault()) : null;
				if (!nullable3.HasValue)
				{
					return null;
				}
				return (int)(nullable3.GetValueOrDefault());
			}
		}

		/// <summary>
		/// Gets the record identifier.
		/// </summary>
		/// <value>
		/// The record identifier.
		/// </value>
		public override long? RecordId
		{
			get
			{
				this.PrepareSystemData();
				ulong? recordId = this.systemProperties.RecordId;
				if (!recordId.HasValue)
				{
					return null;
				}
				return (long)(recordId.GetValueOrDefault());
			}
		}

		/// <summary>
		/// Gets the related activity identifier.
		/// </summary>
		/// <value>
		/// The related activity identifier.
		/// </value>
		public override Guid? RelatedActivityId
		{
			get
			{
				this.PrepareSystemData();
				return this.systemProperties.RelatedActivityId;
			}
		}

		/// <summary>
		/// Gets the task.
		/// </summary>
		/// <value>
		/// The task.
		/// </value>
		public override int? Task
		{
			get
			{
				this.PrepareSystemData();
				ushort? task = this.systemProperties.Task;
				uint? nullable3 = task.HasValue ? new uint?(task.GetValueOrDefault()) : null;
				if (!nullable3.HasValue)
				{
					return null;
				}
				return (int)(nullable3.GetValueOrDefault());
			}
		}

		/// <summary>
		/// Gets the display name of the task.
		/// </summary>
		/// <value>
		/// The display name of the task.
		/// </value>
		public override string TaskDisplayName
		{
			get
			{
				if (this.taskNameReady)
				{
					return this.taskName;
				}
				lock (this.syncObject)
				{
					if (!this.taskNameReady)
					{
						this.taskNameReady = true;
						this.taskName = this.cachedMetadataInformation.GetTaskDisplayName(this.ProviderName, this.handle);
					}
					return this.taskName;
				}
			}
		}

		/// <summary>
		/// Gets the thread identifier.
		/// </summary>
		/// <value>
		/// The thread identifier.
		/// </value>
		public override int? ThreadId
		{
			get
			{
				this.PrepareSystemData();
				uint? threadId = this.systemProperties.ThreadId;
				if (!threadId.HasValue)
				{
					return null;
				}
				return (int)(threadId.GetValueOrDefault());
			}
		}

		/// <summary>
		/// Gets the time created.
		/// </summary>
		/// <value>
		/// The time created.
		/// </value>
		public override DateTime? TimeCreated
		{
			get
			{
				this.PrepareSystemData();
				return this.systemProperties.TimeCreated;
			}
		}

		/// <summary>
		/// Gets the user identifier.
		/// </summary>
		/// <value>
		/// The user identifier.
		/// </value>
		public override SecurityIdentifier UserId
		{
			get
			{
				this.PrepareSystemData();
				return this.systemProperties.UserId;
			}
		}

		/// <summary>
		/// Gets the version.
		/// </summary>
		/// <value>
		/// The version.
		/// </value>
		public override byte? Version
		{
			get
			{
				this.PrepareSystemData();
				return this.systemProperties.Version;
			}
		}

		internal EventLogHandle Handle
		{
			[SecurityTreatAsSafe]
			get
			{
				return this.handle;
			}
		}

		/// <summary>
		/// Formats the description.
		/// </summary>
		/// <returns></returns>
		public override string FormatDescription()
		{
			return this.cachedMetadataInformation.GetFormatDescription(this.ProviderName, this.handle);
		}

		/// <summary>
		/// Formats the description.
		/// </summary>
		/// <param name="values">The values.</param>
		/// <returns></returns>
		public override string FormatDescription(IEnumerable<object> values)
		{
			if (values == null)
			{
				return this.FormatDescription();
			}
			string[] array = new string[0];
			int index = 0;
			foreach (object obj2 in values)
			{
				if (array.Length == index)
				{
					Array.Resize<string>(ref array, index + 1);
				}
				array[index] = obj2.ToString();
				index++;
			}
			return this.cachedMetadataInformation.GetFormatDescription(this.ProviderName, this.handle, array);
		}

		/// <summary>
		/// Gets the property values.
		/// </summary>
		/// <param name="propertySelector">The property selector.</param>
		/// <returns></returns>
		public IList<object> GetPropertyValues(EventLogPropertySelector propertySelector)
		{
			if (propertySelector == null)
			{
				throw new ArgumentNullException("propertySelector");
			}
			return NativeWrapper.EvtRenderBufferWithContextUserOrValues(propertySelector.Handle, this.handle);
		}

		/// <summary>
		/// To the XML.
		/// </summary>
		/// <returns></returns>
		[SecurityTreatAsSafe, SecurityCritical]
		public override string ToXml()
		{
			EventLogPermissionHolder.GetEventLogPermission().Demand();
			StringBuilder buffer = new StringBuilder(0x7d0);
			NativeWrapper.EvtRender(EventLogHandle.Zero, this.handle, UnsafeNativeMethods.EvtRenderFlags.EvtRenderEventXml, buffer);
			return buffer.ToString();
		}

		[SecurityCritical]
		internal static EventLogHandle GetBookmarkHandleFromBookmark(EventBookmark bookmark)
		{
			if (bookmark == null)
			{
				return EventLogHandle.Zero;
			}
			return NativeWrapper.EvtCreateBookmark(bookmark.BookmarkText);
		}

		internal void PrepareSystemData()
		{
			if (!this.systemProperties.filled)
			{
				this.session.SetupSystemContext();
				lock (this.syncObject)
				{
					if (!this.systemProperties.filled)
					{
						NativeWrapper.EvtRenderBufferWithContextSystem(this.session.renderContextHandleSystem, this.handle, UnsafeNativeMethods.EvtRenderFlags.EvtRenderEventValues, this.systemProperties, 0x12);
						this.systemProperties.filled = true;
					}
				}
			}
		}

		/// <summary>
		/// Releases unmanaged and - optionally - managed resources.
		/// </summary>
		/// <param name="disposing"><c>true</c> to release both managed and unmanaged resources; <c>false</c> to release only unmanaged resources.</param>
		[SecurityTreatAsSafe, SecurityCritical]
		protected override void Dispose(bool disposing)
		{
			try
			{
				if (disposing)
				{
					EventLogPermissionHolder.GetEventLogPermission().Demand();
				}
				if ((this.handle != null) && !this.handle.IsInvalid)
				{
					this.handle.Dispose();
				}
			}
			finally
			{
			}
		}
	}

	/// <summary>
	/// 
	/// </summary>
	public class EventLogSession : IDisposable
	{
		internal EventLogHandle renderContextHandleSystem;

		internal EventLogHandle renderContextHandleUser;

		private static EventLogSession globalSession = new EventLogSession();

		// Fields
		private string domain;

		private EventLogHandle handle;
		private SessionAuthentication logOnType;
		private string server;
		private object syncObject;
		private string user;

		// Methods
		/// <summary>
		/// Initializes a new instance of the <see cref="EventLogSession"/> class.
		/// </summary>
		[SecurityCritical]
		public EventLogSession()
		{
			this.renderContextHandleSystem = EventLogHandle.Zero;
			this.renderContextHandleUser = EventLogHandle.Zero;
			this.handle = EventLogHandle.Zero;
			EventLogPermissionHolder.GetEventLogPermission().Demand();
			this.syncObject = new object();
		}

		/// <summary>
		/// Initializes a new instance of the <see cref="EventLogSession"/> class.
		/// </summary>
		/// <param name="server">The server.</param>
		public EventLogSession(string server)
			: this(server, null, null, null, SessionAuthentication.Default)
		{
		}

		/// <summary>
		/// Initializes a new instance of the <see cref="EventLogSession"/> class.
		/// </summary>
		/// <param name="server">The server.</param>
		/// <param name="domain">The domain.</param>
		/// <param name="user">The user.</param>
		/// <param name="password">The password.</param>
		/// <param name="logOnType">Type of the log on.</param>
		[SecurityCritical]
		public EventLogSession(string server, string domain, string user, SecureString password, SessionAuthentication logOnType)
		{
			this.renderContextHandleSystem = EventLogHandle.Zero;
			this.renderContextHandleUser = EventLogHandle.Zero;
			this.handle = EventLogHandle.Zero;
			EventLogPermissionHolder.GetEventLogPermission().Demand();
			if (server == null)
			{
				server = "localhost";
			}
			this.syncObject = new object();
			this.server = server;
			this.domain = domain;
			this.user = user;
			this.logOnType = logOnType;
			UnsafeNativeMethods.EvtRpcLogin login = new UnsafeNativeMethods.EvtRpcLogin();
			login.Server = this.server;
			login.User = this.user;
			login.Domain = this.domain;
			login.Flags = (int)this.logOnType;
			login.Password = CoTaskMemUnicodeSafeHandle.Zero;
			try
			{
				if (password != null)
				{
					login.Password.SetMemory(Marshal.SecureStringToCoTaskMemUnicode(password));
				}
				this.handle = NativeWrapper.EvtOpenSession(UnsafeNativeMethods.EvtLoginClass.EvtRpcLogin, ref login, 0, 0);
			}
			finally
			{
				login.Password.Close();
			}
		}

		// Properties
		/// <summary>
		/// Gets the global session.
		/// </summary>
		/// <value>
		/// The global session.
		/// </value>
		public static EventLogSession GlobalSession
		{
			get
			{
				return globalSession;
			}
		}

		internal EventLogHandle Handle
		{
			get
			{
				return this.handle;
			}
		}

		/// <summary>
		/// Cancels the current operations.
		/// </summary>
		public void CancelCurrentOperations()
		{
			NativeWrapper.EvtCancel(this.handle);
		}

		/// <summary>
		/// Clears the log.
		/// </summary>
		/// <param name="logName">Name of the log.</param>
		public void ClearLog(string logName)
		{
			this.ClearLog(logName, null);
		}

		/// <summary>
		/// Clears the log.
		/// </summary>
		/// <param name="logName">Name of the log.</param>
		/// <param name="backupPath">The backup path.</param>
		/// <exception cref="System.ArgumentNullException">logName</exception>
		public void ClearLog(string logName, string backupPath)
		{
			if (logName == null)
			{
				throw new ArgumentNullException("logName");
			}
			NativeWrapper.EvtClearLog(this.Handle, logName, backupPath, 0);
		}

		/// <summary>
		/// Performs application-defined tasks associated with freeing, releasing, or resetting unmanaged resources.
		/// </summary>
		public void Dispose()
		{
			this.Dispose(true);
			GC.SuppressFinalize(this);
		}

		/// <summary>
		/// Exports the log.
		/// </summary>
		/// <param name="path">The path.</param>
		/// <param name="pathType">Type of the path.</param>
		/// <param name="query">The query.</param>
		/// <param name="targetFilePath">The target file path.</param>
		public void ExportLog(string path, PathType pathType, string query, string targetFilePath)
		{
			this.ExportLog(path, pathType, query, targetFilePath, false);
		}

		/// <summary>
		/// Exports the log.
		/// </summary>
		/// <param name="path">The path.</param>
		/// <param name="pathType">Type of the path.</param>
		/// <param name="query">The query.</param>
		/// <param name="targetFilePath">The target file path.</param>
		/// <param name="tolerateQueryErrors">if set to <c>true</c> [tolerate query errors].</param>
		/// <exception cref="System.ArgumentNullException">
		/// path
		/// or
		/// targetFilePath
		/// </exception>
		/// <exception cref="System.ArgumentOutOfRangeException">pathType</exception>
		public void ExportLog(string path, PathType pathType, string query, string targetFilePath, bool tolerateQueryErrors)
		{
			UnsafeNativeMethods.EvtExportLogFlags evtExportLogChannelPath;
			if (path == null)
			{
				throw new ArgumentNullException("path");
			}
			if (targetFilePath == null)
			{
				throw new ArgumentNullException("targetFilePath");
			}
			switch (pathType)
			{
				case PathType.LogName:
					evtExportLogChannelPath = UnsafeNativeMethods.EvtExportLogFlags.EvtExportLogChannelPath;
					break;

				case PathType.FilePath:
					evtExportLogChannelPath = UnsafeNativeMethods.EvtExportLogFlags.EvtExportLogFilePath;
					break;

				default:
					throw new ArgumentOutOfRangeException("pathType");
			}
			if (!tolerateQueryErrors)
			{
				NativeWrapper.EvtExportLog(this.Handle, path, query, targetFilePath, (int)evtExportLogChannelPath);
			}
			else
			{
				NativeWrapper.EvtExportLog(this.Handle, path, query, targetFilePath, ((int)evtExportLogChannelPath) | 0x1000);
			}
		}

		/// <summary>
		/// Exports the log and messages.
		/// </summary>
		/// <param name="path">The path.</param>
		/// <param name="pathType">Type of the path.</param>
		/// <param name="query">The query.</param>
		/// <param name="targetFilePath">The target file path.</param>
		public void ExportLogAndMessages(string path, PathType pathType, string query, string targetFilePath)
		{
			this.ExportLogAndMessages(path, pathType, query, targetFilePath, false, CultureInfo.CurrentCulture);
		}

		/// <summary>
		/// Exports the log and messages.
		/// </summary>
		/// <param name="path">The path.</param>
		/// <param name="pathType">Type of the path.</param>
		/// <param name="query">The query.</param>
		/// <param name="targetFilePath">The target file path.</param>
		/// <param name="tolerateQueryErrors">if set to <c>true</c> [tolerate query errors].</param>
		/// <param name="targetCultureInfo">The target culture information.</param>
		public void ExportLogAndMessages(string path, PathType pathType, string query, string targetFilePath, bool tolerateQueryErrors, CultureInfo targetCultureInfo)
		{
			if (targetCultureInfo == null)
			{
				targetCultureInfo = CultureInfo.CurrentCulture;
			}
			this.ExportLog(path, pathType, query, targetFilePath, tolerateQueryErrors);
			NativeWrapper.EvtArchiveExportedLog(this.Handle, targetFilePath, targetCultureInfo.LCID, 0);
		}

		/// <summary>
		/// Gets the log information.
		/// </summary>
		/// <param name="logName">Name of the log.</param>
		/// <param name="pathType">Type of the path.</param>
		/// <returns></returns>
		/// <exception cref="System.ArgumentNullException">logName</exception>
		public EventLogInformation GetLogInformation(string logName, PathType pathType)
		{
			if (logName == null)
			{
				throw new ArgumentNullException("logName");
			}
			return new EventLogInformation(this, logName, pathType);
		}

		/// <summary>
		/// Gets the log names.
		/// </summary>
		/// <returns></returns>
		[SecurityCritical]
		public IEnumerable<string> GetLogNames()
		{
			EventLogPermissionHolder.GetEventLogPermission().Demand();
			List<string> list = new List<string>(100);
			using (EventLogHandle handle = NativeWrapper.EvtOpenChannelEnum(this.Handle, 0))
			{
				bool finish = false;
				do
				{
					string item = NativeWrapper.EvtNextChannelPath(handle, ref finish);
					if (!finish)
					{
						list.Add(item);
					}
				}
				while (!finish);
				return list;
			}
		}

		/// <summary>
		/// Gets the provider names.
		/// </summary>
		/// <returns></returns>
		[SecurityCritical]
		public IEnumerable<string> GetProviderNames()
		{
			EventLogPermissionHolder.GetEventLogPermission().Demand();
			List<string> list = new List<string>(100);
			using (EventLogHandle handle = NativeWrapper.EvtOpenProviderEnum(this.Handle, 0))
			{
				bool finish = false;
				do
				{
					string item = NativeWrapper.EvtNextPublisherId(handle, ref finish);
					if (!finish)
					{
						list.Add(item);
					}
				}
				while (!finish);
				return list;
			}
		}

		[SecurityTreatAsSafe, SecurityCritical]
		internal void SetupSystemContext()
		{
			EventLogPermissionHolder.GetEventLogPermission().Demand();
			if (this.renderContextHandleSystem.IsInvalid)
			{
				lock (this.syncObject)
				{
					if (this.renderContextHandleSystem.IsInvalid)
					{
						this.renderContextHandleSystem = NativeWrapper.EvtCreateRenderContext(0, null, UnsafeNativeMethods.EvtRenderContextFlags.EvtRenderContextSystem);
					}
				}
			}
		}

		[SecurityCritical, SecurityTreatAsSafe]
		internal void SetupUserContext()
		{
			EventLogPermissionHolder.GetEventLogPermission().Demand();
			lock (this.syncObject)
			{
				if (this.renderContextHandleUser.IsInvalid)
				{
					this.renderContextHandleUser = NativeWrapper.EvtCreateRenderContext(0, null, UnsafeNativeMethods.EvtRenderContextFlags.EvtRenderContextUser);
				}
			}
		}

		/// <summary>
		/// Releases unmanaged and - optionally - managed resources.
		/// </summary>
		/// <param name="disposing"><c>true</c> to release both managed and unmanaged resources; <c>false</c> to release only unmanaged resources.</param>
		/// <exception cref="System.InvalidOperationException"></exception>
		[SecurityTreatAsSafe, SecurityCritical]
		protected virtual void Dispose(bool disposing)
		{
			if (disposing)
			{
				if (this == globalSession)
				{
					throw new InvalidOperationException();
				}
				EventLogPermissionHolder.GetEventLogPermission().Demand();
			}
			if ((this.renderContextHandleSystem != null) && !this.renderContextHandleSystem.IsInvalid)
			{
				this.renderContextHandleSystem.Dispose();
			}
			if ((this.renderContextHandleUser != null) && !this.renderContextHandleUser.IsInvalid)
			{
				this.renderContextHandleUser.Dispose();
			}
			if ((this.handle != null) && !this.handle.IsInvalid)
			{
				this.handle.Dispose();
			}
		}
	}

	/// <summary>
	/// Contains the status code or error code for a specific event log. This status can be used to determine if the event log is available for an operation.
	/// </summary>
	[HostProtection(SecurityAction.LinkDemand, MayLeakOnAbort = true)]
	internal sealed class EventLogStatus
	{
		// Fields
		private string channelName;

		private int win32ErrorCode;

		// Methods
		internal EventLogStatus(string channelName, int win32ErrorCode)
		{
			this.channelName = channelName;
			this.win32ErrorCode = win32ErrorCode;
		}

		// Properties
		/// <summary>
		/// Gets the name of the log.
		/// </summary>
		/// <value>
		/// The name of the log.
		/// </value>
		public string LogName
		{
			get
			{
				return this.channelName;
			}
		}

		/// <summary>
		/// Gets the status code.
		/// </summary>
		/// <value>
		/// The status code.
		/// </value>
		public int StatusCode
		{
			get
			{
				return this.win32ErrorCode;
			}
		}
	}

	/// <summary>
	/// Contains the metadata (properties and settings) for an event that is defined in an event provider. 
	/// </summary>
	[HostProtection(SecurityAction.LinkDemand, MayLeakOnAbort = true)]
	public sealed class EventMetadata
	{
		// Fields
		private byte channelId;

		private string description;
		private long id;
		private long keywords;
		private byte level;
		private short opcode;
		private ProviderMetadata pmReference;
		private int task;
		private string template;
		private byte version;

		// Methods
		internal EventMetadata(uint id, byte version, byte channelId, byte level, byte opcode, short task, long keywords, string template, string description, ProviderMetadata pmReference)
		{
			this.id = id;
			this.version = version;
			this.channelId = channelId;
			this.level = level;
			this.opcode = opcode;
			this.task = task;
			this.keywords = keywords;
			this.template = template;
			this.description = description;
			this.pmReference = pmReference;
		}

		// Properties
		/// <summary>
		/// Gets the description.
		/// </summary>
		/// <value>
		/// The description.
		/// </value>
		public string Description
		{
			get
			{
				return this.description;
			}
		}

		/// <summary>
		/// Gets the identifier.
		/// </summary>
		/// <value>
		/// The identifier.
		/// </value>
		public long Id
		{
			get
			{
				return this.id;
			}
		}

		/// <summary>
		/// Gets the keywords.
		/// </summary>
		/// <value>
		/// The keywords.
		/// </value>
		public IEnumerable<EventKeyword> Keywords
		{
			get
			{
				List<EventKeyword> list = new List<EventKeyword>();
				ulong keywords = (ulong)this.keywords;
				ulong num2 = 9223372036854775808L;
				for (int i = 0; i < 0x40; i++)
				{
					if ((keywords & num2) > 0L)
					{
						list.Add(new EventKeyword((long)num2, this.pmReference));
					}
					num2 = num2 >> 1;
				}
				return list;
			}
		}

		/// <summary>
		/// Gets the level.
		/// </summary>
		/// <value>
		/// The level.
		/// </value>
		public EventLevel Level
		{
			get
			{
				return new EventLevel(this.level, this.pmReference);
			}
		}

		/// <summary>
		/// Gets the log link.
		/// </summary>
		/// <value>
		/// The log link.
		/// </value>
		public EventLogLink LogLink
		{
			get
			{
				return new EventLogLink(this.channelId, this.pmReference);
			}
		}

		/// <summary>
		/// Gets the opcode.
		/// </summary>
		/// <value>
		/// The opcode.
		/// </value>
		public EventOpcode Opcode
		{
			get
			{
				return new EventOpcode(this.opcode, this.pmReference);
			}
		}

		/// <summary>
		/// Gets the task.
		/// </summary>
		/// <value>
		/// The task.
		/// </value>
		public EventTask Task
		{
			get
			{
				return new EventTask(this.task, this.pmReference);
			}
		}

		/// <summary>
		/// Gets the template.
		/// </summary>
		/// <value>
		/// The template.
		/// </value>
		public string Template
		{
			get
			{
				return this.template;
			}
		}

		/// <summary>
		/// Gets the version.
		/// </summary>
		/// <value>
		/// The version.
		/// </value>
		public byte Version
		{
			get
			{
				return this.version;
			}
		}
	}

	/// <summary>
	/// 
	/// </summary>
	[HostProtection(SecurityAction.LinkDemand, MayLeakOnAbort = true)]
	public sealed class EventOpcode
	{
		// Fields
		private bool dataReady;

		private string displayName;
		private string name;
		private ProviderMetadata pmReference;
		private object syncObject;
		private int value;

		// Methods
		internal EventOpcode(int value, ProviderMetadata pmReference)
		{
			this.value = value;
			this.pmReference = pmReference;
			this.syncObject = new object();
		}

		internal EventOpcode(string name, int value, string displayName)
		{
			this.value = value;
			this.name = name;
			this.displayName = displayName;
			this.dataReady = true;
			this.syncObject = new object();
		}

		// Properties
		/// <summary>
		/// Gets the display name.
		/// </summary>
		/// <value>
		/// The display name.
		/// </value>
		public string DisplayName
		{
			get
			{
				this.PrepareData();
				return this.displayName;
			}
		}

		/// <summary>
		/// Gets the name.
		/// </summary>
		/// <value>
		/// The name.
		/// </value>
		public string Name
		{
			get
			{
				this.PrepareData();
				return this.name;
			}
		}

		/// <summary>
		/// Gets the value.
		/// </summary>
		/// <value>
		/// The value.
		/// </value>
		public int Value
		{
			get
			{
				return this.value;
			}
		}

		internal void PrepareData()
		{
			lock (this.syncObject)
			{
				if (!this.dataReady)
				{
					IEnumerable<EventOpcode> opcodes = this.pmReference.Opcodes;
					this.name = null;
					this.displayName = null;
					this.dataReady = true;
					foreach (EventOpcode opcode in opcodes)
					{
						if (opcode.Value == this.value)
						{
							this.name = opcode.Name;
							this.displayName = opcode.DisplayName;
							this.dataReady = true;
							break;
						}
					}
				}
			}
		}
	}

	/// <summary>
	/// Contains the value of an event property that is specified by the event provider when the event is published. 
	/// </summary>
	[HostProtection(SecurityAction.LinkDemand, MayLeakOnAbort = true)]
	public sealed class EventProperty
	{
		// Fields
		private object value;

		// Methods
		internal EventProperty(object value)
		{
			this.value = value;
		}

		// Properties
		/// <summary>
		/// Gets the value.
		/// </summary>
		/// <value>
		/// The value.
		/// </value>
		public object Value
		{
			get
			{
				return this.value;
			}
		}
	}

	/// <summary>
	/// Contains the properties of an event instance for an event that is received from an EventLogReader object. The event properties provide information about the event such as the name of the computer where the event was logged and the time that the event was created. 
	/// </summary>
	public abstract class EventRecord : IDisposable
	{
		/// <summary>
		/// Initializes a new instance of the <see cref="EventRecord"/> class.
		/// </summary>
		protected EventRecord()
		{
		}

		/// <summary>
		/// Releases unmanaged and - optionally - managed resources.
		/// </summary>
		public void Dispose()
		{
			this.Dispose(true);
			GC.SuppressFinalize(this);
		}

		/// <summary>
		/// Releases unmanaged and - optionally - managed resources.
		/// </summary>
		/// <param name="disposing"><c>true</c> to release both managed and unmanaged resources; <c>false</c> to release only unmanaged resources.</param>
		protected virtual void Dispose(bool disposing)
		{
		}

		/// <summary>
		/// Formats the description.
		/// </summary>
		/// <returns></returns>
		public abstract string FormatDescription();

		/// <summary>
		/// Formats the description.
		/// </summary>
		/// <param name="values">The values.</param>
		/// <returns></returns>
		public abstract string FormatDescription(IEnumerable<object> values);

		/// <summary>
		/// To the XML.
		/// </summary>
		/// <returns></returns>
		public abstract string ToXml();

		/// <summary>
		/// Gets the activity identifier.
		/// </summary>
		/// <value>
		/// The activity identifier.
		/// </value>
		public abstract Guid? ActivityId { get; }

		/// <summary>
		/// Gets the bookmark.
		/// </summary>
		/// <value>
		/// The bookmark.
		/// </value>
		public abstract EventBookmark Bookmark { get; }

		/// <summary>
		/// Gets the identifier.
		/// </summary>
		/// <value>
		/// The identifier.
		/// </value>
		public abstract int Id { get; }

		/// <summary>
		/// Gets the keywords.
		/// </summary>
		/// <value>
		/// The keywords.
		/// </value>
		public abstract long? Keywords { get; }

		/// <summary>
		/// Gets the keywords display names.
		/// </summary>
		/// <value>
		/// The keywords display names.
		/// </value>
		public abstract IEnumerable<string> KeywordsDisplayNames { get; }

		/// <summary>
		/// Gets the level.
		/// </summary>
		/// <value>
		/// The level.
		/// </value>
		public abstract byte? Level { get; }

		/// <summary>
		/// Gets the display name of the level.
		/// </summary>
		/// <value>
		/// The display name of the level.
		/// </value>
		public abstract string LevelDisplayName { get; }

		/// <summary>
		/// Gets the name of the log.
		/// </summary>
		/// <value>
		/// The name of the log.
		/// </value>
		public abstract string LogName { get; }

		/// <summary>
		/// Gets the name of the machine.
		/// </summary>
		/// <value>
		/// The name of the machine.
		/// </value>
		public abstract string MachineName { get; }

		/// <summary>
		/// Gets the opcode.
		/// </summary>
		/// <value>
		/// The opcode.
		/// </value>
		public abstract short? Opcode { get; }

		/// <summary>
		/// Gets the display name of the opcode.
		/// </summary>
		/// <value>
		/// The display name of the opcode.
		/// </value>
		public abstract string OpcodeDisplayName { get; }

		/// <summary>
		/// Gets the process identifier.
		/// </summary>
		/// <value>
		/// The process identifier.
		/// </value>
		public abstract int? ProcessId { get; }

		/// <summary>
		/// Gets the properties.
		/// </summary>
		/// <value>
		/// The properties.
		/// </value>
		public abstract IList<EventProperty> Properties { get; }

		/// <summary>
		/// Gets the provider identifier.
		/// </summary>
		/// <value>
		/// The provider identifier.
		/// </value>
		public abstract Guid? ProviderId { get; }

		/// <summary>
		/// Gets the name of the provider.
		/// </summary>
		/// <value>
		/// The name of the provider.
		/// </value>
		public abstract string ProviderName { get; }

		/// <summary>
		/// Gets the qualifiers.
		/// </summary>
		/// <value>
		/// The qualifiers.
		/// </value>
		public abstract int? Qualifiers { get; }

		/// <summary>
		/// Gets the record identifier.
		/// </summary>
		/// <value>
		/// The record identifier.
		/// </value>
		public abstract long? RecordId { get; }

		/// <summary>
		/// Gets the related activity identifier.
		/// </summary>
		/// <value>
		/// The related activity identifier.
		/// </value>
		public abstract Guid? RelatedActivityId { get; }

		/// <summary>
		/// Gets the task.
		/// </summary>
		/// <value>
		/// The task.
		/// </value>
		public abstract int? Task { get; }

		/// <summary>
		/// Gets the display name of the task.
		/// </summary>
		/// <value>
		/// The display name of the task.
		/// </value>
		public abstract string TaskDisplayName { get; }

		/// <summary>
		/// Gets the thread identifier.
		/// </summary>
		/// <value>
		/// The thread identifier.
		/// </value>
		public abstract int? ThreadId { get; }

		/// <summary>
		/// Gets the time created.
		/// </summary>
		/// <value>
		/// The time created.
		/// </value>
		public abstract DateTime? TimeCreated { get; }

		/// <summary>
		/// Gets the user identifier.
		/// </summary>
		/// <value>
		/// The user identifier.
		/// </value>
		public abstract SecurityIdentifier UserId { get; }

		/// <summary>
		/// Gets the version.
		/// </summary>
		/// <value>
		/// The version.
		/// </value>
		public abstract byte? Version { get; }
	}

	/// <summary>
	/// 
	/// </summary>
	[HostProtection(SecurityAction.LinkDemand, MayLeakOnAbort = true)]
	public sealed class EventTask
	{
		// Fields
		private bool dataReady;

		private string displayName;
		private Guid guid;
		private string name;
		private ProviderMetadata pmReference;
		private object syncObject;
		private int value;

		// Methods
		internal EventTask(int value, ProviderMetadata pmReference)
		{
			this.value = value;
			this.pmReference = pmReference;
			this.syncObject = new object();
		}

		internal EventTask(string name, int value, string displayName, Guid guid)
		{
			this.value = value;
			this.name = name;
			this.displayName = displayName;
			this.guid = guid;
			this.dataReady = true;
			this.syncObject = new object();
		}

		// Properties
		/// <summary>
		/// Gets the display name.
		/// </summary>
		/// <value>
		/// The display name.
		/// </value>
		public string DisplayName
		{
			get
			{
				this.PrepareData();
				return this.displayName;
			}
		}

		/// <summary>
		/// Gets the event unique identifier.
		/// </summary>
		/// <value>
		/// The event unique identifier.
		/// </value>
		public Guid EventGuid
		{
			get
			{
				this.PrepareData();
				return this.guid;
			}
		}

		/// <summary>
		/// Gets the name.
		/// </summary>
		/// <value>
		/// The name.
		/// </value>
		public string Name
		{
			get
			{
				this.PrepareData();
				return this.name;
			}
		}

		/// <summary>
		/// Gets the value.
		/// </summary>
		/// <value>
		/// The value.
		/// </value>
		public int Value
		{
			get
			{
				return this.value;
			}
		}

		internal void PrepareData()
		{
			lock (this.syncObject)
			{
				if (!this.dataReady)
				{
					IEnumerable<EventTask> tasks = this.pmReference.Tasks;
					this.name = null;
					this.displayName = null;
					this.guid = Guid.Empty;
					this.dataReady = true;
					foreach (EventTask task in tasks)
					{
						if (task.Value == this.value)
						{
							this.name = task.Name;
							this.displayName = task.DisplayName;
							this.guid = task.EventGuid;
							this.dataReady = true;
							break;
						}
					}
				}
			}
		}
	}

	/// <summary>
	/// 
	/// </summary>
	[HostProtection(SecurityAction.LinkDemand, MayLeakOnAbort = true)]
	public class ProviderMetadata : IDisposable
	{
		// Fields
		private IList<EventLogLink> channelReferences;

		private CultureInfo cultureInfo;
		private EventLogHandle defaultProviderHandle;
		private EventLogHandle handle;
		private IList<EventKeyword> keywords;
		private IList<EventLevel> levels;
		private string logFilePath;
		private IList<EventOpcode> opcodes;
		private string providerName;
		private EventLogSession session;
		private IList<EventKeyword> standardKeywords;
		private IList<EventLevel> standardLevels;
		private IList<EventOpcode> standardOpcodes;
		private IList<EventTask> standardTasks;
		private object syncObject;
		private IList<EventTask> tasks;

		// Methods
		/// <summary>
		/// Initializes a new instance of the <see cref="ProviderMetadata"/> class.
		/// </summary>
		/// <param name="providerName">Name of the provider.</param>
		public ProviderMetadata(string providerName)
			: this(providerName, null, null, null)
		{
		}

		/// <summary>
		/// Initializes a new instance of the <see cref="ProviderMetadata"/> class.
		/// </summary>
		/// <param name="providerName">Name of the provider.</param>
		/// <param name="session">The session.</param>
		/// <param name="targetCultureInfo">The target culture information.</param>
		public ProviderMetadata(string providerName, EventLogSession session, CultureInfo targetCultureInfo)
			: this(providerName, session, targetCultureInfo, null)
		{
		}

		[SecurityCritical, SecurityTreatAsSafe]
		internal ProviderMetadata(string providerName, EventLogSession session, CultureInfo targetCultureInfo, string logFilePath)
		{
			this.handle = EventLogHandle.Zero;
			this.defaultProviderHandle = EventLogHandle.Zero;
			EventLogPermissionHolder.GetEventLogPermission().Demand();
			if (targetCultureInfo == null)
			{
				targetCultureInfo = CultureInfo.CurrentCulture;
			}
			if (session == null)
			{
				session = EventLogSession.GlobalSession;
			}
			this.session = session;
			this.providerName = providerName;
			this.cultureInfo = targetCultureInfo;
			this.logFilePath = logFilePath;
			this.handle = NativeWrapper.EvtOpenProviderMetadata(this.session.Handle, this.providerName, this.logFilePath, this.cultureInfo.LCID, 0);
			this.syncObject = new object();
		}

		// Nested Types
		internal enum ObjectTypeName
		{
			Level,
			Opcode,
			Task,
			Keyword
		}

		// Properties
		/// <summary>
		/// Gets the display name.
		/// </summary>
		/// <value>
		/// The display name.
		/// </value>
		public string DisplayName
		{
			[SecurityCritical]
			get
			{
				uint providerMessageID = this.ProviderMessageID;
				if (providerMessageID == uint.MaxValue)
				{
					return null;
				}
				EventLogPermissionHolder.GetEventLogPermission().Demand();
				return NativeWrapper.EvtFormatMessage(this.handle, providerMessageID);
			}
		}

		/// <summary>
		/// Gets the events.
		/// </summary>
		/// <value>
		/// The events.
		/// </value>
		public IEnumerable<EventMetadata> Events
		{
			[SecurityCritical]
			get
			{
				EventLogPermissionHolder.GetEventLogPermission().Demand();
				List<EventMetadata> list = new List<EventMetadata>();
				EventLogHandle eventMetadataEnum = NativeWrapper.EvtOpenEventMetadataEnum(this.handle, 0);
				using (eventMetadataEnum)
				{
					EventLogHandle handle2;
				Label_0020:
					handle2 = handle2 = NativeWrapper.EvtNextEventMetadata(eventMetadataEnum, 0);
					if (handle2 != null)
					{
						using (handle2)
						{
							string str2;
							uint id = (uint)NativeWrapper.EvtGetEventMetadataProperty(handle2, UnsafeNativeMethods.EvtEventMetadataPropertyId.EventMetadataEventID);
							byte version = (byte)((uint)NativeWrapper.EvtGetEventMetadataProperty(handle2, UnsafeNativeMethods.EvtEventMetadataPropertyId.EventMetadataEventVersion));
							byte channelId = (byte)((uint)NativeWrapper.EvtGetEventMetadataProperty(handle2, UnsafeNativeMethods.EvtEventMetadataPropertyId.EventMetadataEventChannel));
							byte level = (byte)((uint)NativeWrapper.EvtGetEventMetadataProperty(handle2, UnsafeNativeMethods.EvtEventMetadataPropertyId.EventMetadataEventLevel));
							byte opcode = (byte)((uint)NativeWrapper.EvtGetEventMetadataProperty(handle2, UnsafeNativeMethods.EvtEventMetadataPropertyId.EventMetadataEventOpcode));
							short task = (short)((uint)NativeWrapper.EvtGetEventMetadataProperty(handle2, UnsafeNativeMethods.EvtEventMetadataPropertyId.EventMetadataEventTask));
							long keywords = (long)((ulong)NativeWrapper.EvtGetEventMetadataProperty(handle2, UnsafeNativeMethods.EvtEventMetadataPropertyId.EventMetadataEventKeyword));
							string template = (string)NativeWrapper.EvtGetEventMetadataProperty(handle2, UnsafeNativeMethods.EvtEventMetadataPropertyId.EventMetadataEventTemplate);
							int num8 = (int)((uint)NativeWrapper.EvtGetEventMetadataProperty(handle2, UnsafeNativeMethods.EvtEventMetadataPropertyId.EventMetadataEventMessageID));
							if (num8 == -1)
							{
								str2 = null;
							}
							else
							{
								str2 = NativeWrapper.EvtFormatMessage(this.handle, (uint)num8);
							}
							EventMetadata item = new EventMetadata(id, version, channelId, level, opcode, task, keywords, template, str2, this);
							list.Add(item);
							goto Label_0020;
						}
					}
					return list.AsReadOnly();
				}
			}
		}

		/// <summary>
		/// Gets the help link.
		/// </summary>
		/// <value>
		/// The help link.
		/// </value>
		public Uri HelpLink
		{
			get
			{
				string uriString = (string)NativeWrapper.EvtGetPublisherMetadataProperty(this.handle, UnsafeNativeMethods.EvtPublisherMetadataPropertyId.EvtPublisherMetadataHelpLink);
				if ((uriString != null) && (uriString.Length != 0))
				{
					return new Uri(uriString);
				}
				return null;
			}
		}

		/// <summary>
		/// Gets the identifier.
		/// </summary>
		/// <value>
		/// The identifier.
		/// </value>
		public Guid Id
		{
			get
			{
				return (Guid)NativeWrapper.EvtGetPublisherMetadataProperty(this.handle, UnsafeNativeMethods.EvtPublisherMetadataPropertyId.EvtPublisherMetadataPublisherGuid);
			}
		}

		/// <summary>
		/// Gets the keywords.
		/// </summary>
		/// <value>
		/// The keywords.
		/// </value>
		public IList<EventKeyword> Keywords
		{
			get
			{
				lock (this.syncObject)
				{
					if (this.keywords != null)
					{
						return this.keywords;
					}
					this.keywords = ((List<EventKeyword>)this.GetProviderListProperty(this.handle, UnsafeNativeMethods.EvtPublisherMetadataPropertyId.EvtPublisherMetadataKeywords)).AsReadOnly();
				}
				return this.keywords;
			}
		}

		/// <summary>
		/// Gets the levels.
		/// </summary>
		/// <value>
		/// The levels.
		/// </value>
		public IList<EventLevel> Levels
		{
			get
			{
				lock (this.syncObject)
				{
					if (this.levels != null)
					{
						return this.levels;
					}
					this.levels = ((List<EventLevel>)this.GetProviderListProperty(this.handle, UnsafeNativeMethods.EvtPublisherMetadataPropertyId.EvtPublisherMetadataLevels)).AsReadOnly();
				}
				return this.levels;
			}
		}

		/// <summary>
		/// Gets the log links.
		/// </summary>
		/// <value>
		/// The log links.
		/// </value>
		public IList<EventLogLink> LogLinks
		{
			[SecurityCritical]
			get
			{
				IList<EventLogLink> channelReferences;
				EventLogHandle zero = EventLogHandle.Zero;
				try
				{
					lock (this.syncObject)
					{
						if (this.channelReferences != null)
						{
							return this.channelReferences;
						}
						EventLogPermissionHolder.GetEventLogPermission().Demand();
						zero = NativeWrapper.EvtGetPublisherMetadataPropertyHandle(this.handle, UnsafeNativeMethods.EvtPublisherMetadataPropertyId.EvtPublisherMetadataChannelReferences);
						int capacity = NativeWrapper.EvtGetObjectArraySize(zero);
						List<EventLogLink> list = new List<EventLogLink>(capacity);
						for (int i = 0; i < capacity; i++)
						{
							bool flag;
							string str2;
							string strA = (string)NativeWrapper.EvtGetObjectArrayProperty(zero, i, 7);
							uint channelId = (uint)NativeWrapper.EvtGetObjectArrayProperty(zero, i, 9);
							uint num4 = (uint)NativeWrapper.EvtGetObjectArrayProperty(zero, i, 10);
							if (num4 == 1)
							{
								flag = true;
							}
							else
							{
								flag = false;
							}
							int num5 = (int)((uint)NativeWrapper.EvtGetObjectArrayProperty(zero, i, 11));
							if (num5 == -1)
							{
								str2 = null;
							}
							else
							{
								str2 = NativeWrapper.EvtFormatMessage(this.handle, (uint)num5);
							}
							if ((str2 == null) && flag)
							{
								if (string.Compare(strA, "Application", StringComparison.OrdinalIgnoreCase) == 0)
								{
									num5 = 0x100;
								}
								else if (string.Compare(strA, "System", StringComparison.OrdinalIgnoreCase) == 0)
								{
									num5 = 0x102;
								}
								else if (string.Compare(strA, "Security", StringComparison.OrdinalIgnoreCase) == 0)
								{
									num5 = 0x101;
								}
								else
								{
									num5 = -1;
								}
								if (num5 != -1)
								{
									if (this.defaultProviderHandle.IsInvalid)
									{
										this.defaultProviderHandle = NativeWrapper.EvtOpenProviderMetadata(this.session.Handle, null, null, this.cultureInfo.LCID, 0);
									}
									str2 = NativeWrapper.EvtFormatMessage(this.defaultProviderHandle, (uint)num5);
								}
							}
							list.Add(new EventLogLink(strA, flag, str2, channelId));
						}
						this.channelReferences = list.AsReadOnly();
					}
					channelReferences = this.channelReferences;
				}
				finally
				{
					zero.Close();
				}
				return channelReferences;
			}
		}

		/// <summary>
		/// Gets the message file path.
		/// </summary>
		/// <value>
		/// The message file path.
		/// </value>
		public string MessageFilePath
		{
			get
			{
				return (string)NativeWrapper.EvtGetPublisherMetadataProperty(this.handle, UnsafeNativeMethods.EvtPublisherMetadataPropertyId.EvtPublisherMetadataMessageFilePath);
			}
		}

		/// <summary>
		/// Gets the name.
		/// </summary>
		/// <value>
		/// The name.
		/// </value>
		public string Name
		{
			get
			{
				return this.providerName;
			}
		}

		/// <summary>
		/// Gets the opcodes.
		/// </summary>
		/// <value>
		/// The opcodes.
		/// </value>
		public IList<EventOpcode> Opcodes
		{
			get
			{
				lock (this.syncObject)
				{
					if (this.opcodes != null)
					{
						return this.opcodes;
					}
					this.opcodes = ((List<EventOpcode>)this.GetProviderListProperty(this.handle, UnsafeNativeMethods.EvtPublisherMetadataPropertyId.EvtPublisherMetadataOpcodes)).AsReadOnly();
				}
				return this.opcodes;
			}
		}

		/// <summary>
		/// Gets the parameter file path.
		/// </summary>
		/// <value>
		/// The parameter file path.
		/// </value>
		public string ParameterFilePath
		{
			get
			{
				return (string)NativeWrapper.EvtGetPublisherMetadataProperty(this.handle, UnsafeNativeMethods.EvtPublisherMetadataPropertyId.EvtPublisherMetadataParameterFilePath);
			}
		}

		/// <summary>
		/// Gets the resource file path.
		/// </summary>
		/// <value>
		/// The resource file path.
		/// </value>
		public string ResourceFilePath
		{
			get
			{
				return (string)NativeWrapper.EvtGetPublisherMetadataProperty(this.handle, UnsafeNativeMethods.EvtPublisherMetadataPropertyId.EvtPublisherMetadataResourceFilePath);
			}
		}

		/// <summary>
		/// Gets the tasks.
		/// </summary>
		/// <value>
		/// The tasks.
		/// </value>
		public IList<EventTask> Tasks
		{
			get
			{
				lock (this.syncObject)
				{
					if (this.tasks != null)
					{
						return this.tasks;
					}
					this.tasks = ((List<EventTask>)this.GetProviderListProperty(this.handle, UnsafeNativeMethods.EvtPublisherMetadataPropertyId.EvtPublisherMetadataTasks)).AsReadOnly();
				}
				return this.tasks;
			}
		}

		internal EventLogHandle Handle
		{
			get
			{
				return this.handle;
			}
		}

		private uint ProviderMessageID
		{
			get
			{
				return (uint)NativeWrapper.EvtGetPublisherMetadataProperty(this.handle, UnsafeNativeMethods.EvtPublisherMetadataPropertyId.EvtPublisherMetadataPublisherMessageID);
			}
		}

		/// <summary>
		/// Performs application-defined tasks associated with freeing, releasing, or resetting unmanaged resources.
		/// </summary>
		public void Dispose()
		{
			this.Dispose(true);
			GC.SuppressFinalize(this);
		}

		internal void CheckReleased()
		{
			lock (this.syncObject)
			{
				this.GetProviderListProperty(this.handle, UnsafeNativeMethods.EvtPublisherMetadataPropertyId.EvtPublisherMetadataTasks);
			}
		}

		internal string FindStandardKeywordDisplayName(string name, long value)
		{
			if (this.standardKeywords == null)
			{
				this.standardKeywords = (List<EventKeyword>)this.GetProviderListProperty(this.defaultProviderHandle, UnsafeNativeMethods.EvtPublisherMetadataPropertyId.EvtPublisherMetadataKeywords);
			}
			foreach (EventKeyword keyword in this.standardKeywords)
			{
				if ((keyword.Name == name) && (keyword.Value == value))
				{
					return keyword.DisplayName;
				}
			}
			return null;
		}

		internal string FindStandardLevelDisplayName(string name, uint value)
		{
			if (this.standardLevels == null)
			{
				this.standardLevels = (List<EventLevel>)this.GetProviderListProperty(this.defaultProviderHandle, UnsafeNativeMethods.EvtPublisherMetadataPropertyId.EvtPublisherMetadataLevels);
			}
			foreach (EventLevel level in this.standardLevels)
			{
				if ((level.Name == name) && (level.Value == value))
				{
					return level.DisplayName;
				}
			}
			return null;
		}

		internal string FindStandardOpcodeDisplayName(string name, uint value)
		{
			if (this.standardOpcodes == null)
			{
				this.standardOpcodes = (List<EventOpcode>)this.GetProviderListProperty(this.defaultProviderHandle, UnsafeNativeMethods.EvtPublisherMetadataPropertyId.EvtPublisherMetadataOpcodes);
			}
			foreach (EventOpcode opcode in this.standardOpcodes)
			{
				if ((opcode.Name == name) && (opcode.Value == value))
				{
					return opcode.DisplayName;
				}
			}
			return null;
		}

		internal string FindStandardTaskDisplayName(string name, uint value)
		{
			if (this.standardTasks == null)
			{
				this.standardTasks = (List<EventTask>)this.GetProviderListProperty(this.defaultProviderHandle, UnsafeNativeMethods.EvtPublisherMetadataPropertyId.EvtPublisherMetadataTasks);
			}
			foreach (EventTask task in this.standardTasks)
			{
				if ((task.Name == name) && (task.Value == value))
				{
					return task.DisplayName;
				}
			}
			return null;
		}

		[SecurityCritical, SecurityTreatAsSafe]
		internal object GetProviderListProperty(EventLogHandle providerHandle, UnsafeNativeMethods.EvtPublisherMetadataPropertyId metadataProperty)
		{
			object obj2;
			EventLogHandle zero = EventLogHandle.Zero;
			EventLogPermissionHolder.GetEventLogPermission().Demand();
			try
			{
				UnsafeNativeMethods.EvtPublisherMetadataPropertyId evtPublisherMetadataOpcodeName;
				UnsafeNativeMethods.EvtPublisherMetadataPropertyId evtPublisherMetadataOpcodeValue;
				UnsafeNativeMethods.EvtPublisherMetadataPropertyId evtPublisherMetadataOpcodeMessageID;
				ObjectTypeName opcode;
				List<EventLevel> list = null;
				List<EventOpcode> list2 = null;
				List<EventKeyword> list3 = null;
				List<EventTask> list4 = null;
				zero = NativeWrapper.EvtGetPublisherMetadataPropertyHandle(providerHandle, metadataProperty);
				int capacity = NativeWrapper.EvtGetObjectArraySize(zero);
				switch (metadataProperty)
				{
					case UnsafeNativeMethods.EvtPublisherMetadataPropertyId.EvtPublisherMetadataOpcodes:
						evtPublisherMetadataOpcodeName = UnsafeNativeMethods.EvtPublisherMetadataPropertyId.EvtPublisherMetadataOpcodeName;
						evtPublisherMetadataOpcodeValue = UnsafeNativeMethods.EvtPublisherMetadataPropertyId.EvtPublisherMetadataOpcodeValue;
						evtPublisherMetadataOpcodeMessageID = UnsafeNativeMethods.EvtPublisherMetadataPropertyId.EvtPublisherMetadataOpcodeMessageID;
						opcode = ObjectTypeName.Opcode;
						list2 = new List<EventOpcode>(capacity);
						break;

					case UnsafeNativeMethods.EvtPublisherMetadataPropertyId.EvtPublisherMetadataKeywords:
						evtPublisherMetadataOpcodeName = UnsafeNativeMethods.EvtPublisherMetadataPropertyId.EvtPublisherMetadataKeywordName;
						evtPublisherMetadataOpcodeValue = UnsafeNativeMethods.EvtPublisherMetadataPropertyId.EvtPublisherMetadataKeywordValue;
						evtPublisherMetadataOpcodeMessageID = UnsafeNativeMethods.EvtPublisherMetadataPropertyId.EvtPublisherMetadataKeywordMessageID;
						opcode = ObjectTypeName.Keyword;
						list3 = new List<EventKeyword>(capacity);
						break;

					case UnsafeNativeMethods.EvtPublisherMetadataPropertyId.EvtPublisherMetadataLevels:
						evtPublisherMetadataOpcodeName = UnsafeNativeMethods.EvtPublisherMetadataPropertyId.EvtPublisherMetadataLevelName;
						evtPublisherMetadataOpcodeValue = UnsafeNativeMethods.EvtPublisherMetadataPropertyId.EvtPublisherMetadataLevelValue;
						evtPublisherMetadataOpcodeMessageID = UnsafeNativeMethods.EvtPublisherMetadataPropertyId.EvtPublisherMetadataLevelMessageID;
						opcode = ObjectTypeName.Level;
						list = new List<EventLevel>(capacity);
						break;

					case UnsafeNativeMethods.EvtPublisherMetadataPropertyId.EvtPublisherMetadataTasks:
						evtPublisherMetadataOpcodeName = UnsafeNativeMethods.EvtPublisherMetadataPropertyId.EvtPublisherMetadataTaskName;
						evtPublisherMetadataOpcodeValue = UnsafeNativeMethods.EvtPublisherMetadataPropertyId.EvtPublisherMetadataTaskValue;
						evtPublisherMetadataOpcodeMessageID = UnsafeNativeMethods.EvtPublisherMetadataPropertyId.EvtPublisherMetadataTaskMessageID;
						opcode = ObjectTypeName.Task;
						list4 = new List<EventTask>(capacity);
						break;

					default:
						return null;
				}
				for (int i = 0; i < capacity; i++)
				{
					string name = (string)NativeWrapper.EvtGetObjectArrayProperty(zero, i, (int)evtPublisherMetadataOpcodeName);
					uint num3 = 0;
					long num4 = 0L;
					if (opcode != ObjectTypeName.Keyword)
					{
						num3 = (uint)NativeWrapper.EvtGetObjectArrayProperty(zero, i, (int)evtPublisherMetadataOpcodeValue);
					}
					else
					{
						num4 = (long)((ulong)NativeWrapper.EvtGetObjectArrayProperty(zero, i, (int)evtPublisherMetadataOpcodeValue));
					}
					int num5 = (int)((uint)NativeWrapper.EvtGetObjectArrayProperty(zero, i, (int)evtPublisherMetadataOpcodeMessageID));
					string displayName = null;
					if (num5 == -1)
					{
						if (providerHandle != this.defaultProviderHandle)
						{
							if (this.defaultProviderHandle.IsInvalid)
							{
								this.defaultProviderHandle = NativeWrapper.EvtOpenProviderMetadata(this.session.Handle, null, null, this.cultureInfo.LCID, 0);
							}
							switch (opcode)
							{
								case ObjectTypeName.Level:
									displayName = this.FindStandardLevelDisplayName(name, num3);
									goto Label_01BA;

								case ObjectTypeName.Opcode:
									displayName = this.FindStandardOpcodeDisplayName(name, num3 >> 0x10);
									goto Label_01BA;

								case ObjectTypeName.Task:
									displayName = this.FindStandardTaskDisplayName(name, num3);
									goto Label_01BA;

								case ObjectTypeName.Keyword:
									displayName = this.FindStandardKeywordDisplayName(name, num4);
									goto Label_01BA;
							}
							displayName = null;
						}
					}
					else
					{
						displayName = NativeWrapper.EvtFormatMessage(providerHandle, (uint)num5);
					}
				Label_01BA:
					switch (opcode)
					{
						case ObjectTypeName.Level:
							list.Add(new EventLevel(name, (int)num3, displayName));
							break;

						case ObjectTypeName.Opcode:
							list2.Add(new EventOpcode(name, (int)(num3 >> 0x10), displayName));
							break;

						case ObjectTypeName.Task:
							{
								Guid guid = (Guid)NativeWrapper.EvtGetObjectArrayProperty(zero, i, 0x12);
								list4.Add(new EventTask(name, (int)num3, displayName, guid));
								break;
							}
						case ObjectTypeName.Keyword:
							list3.Add(new EventKeyword(name, num4, displayName));
							break;

						default:
							return null;
					}
				}
				switch (opcode)
				{
					case ObjectTypeName.Level:
						return list;

					case ObjectTypeName.Opcode:
						return list2;

					case ObjectTypeName.Task:
						return list4;

					case ObjectTypeName.Keyword:
						return list3;
				}
				obj2 = null;
			}
			finally
			{
				zero.Close();
			}
			return obj2;
		}

		/// <summary>
		/// Releases unmanaged and - optionally - managed resources.
		/// </summary>
		/// <param name="disposing"><c>true</c> to release both managed and unmanaged resources; <c>false</c> to release only unmanaged resources.</param>
		[SecurityCritical, SecurityTreatAsSafe]
		protected virtual void Dispose(bool disposing)
		{
			if (disposing)
			{
				EventLogPermissionHolder.GetEventLogPermission().Demand();
			}
			if ((this.handle != null) && !this.handle.IsInvalid)
			{
				this.handle.Dispose();
			}
		}
	}

	[SecurityCritical(SecurityCriticalScope.Everything)]
	internal sealed class CoTaskMemSafeHandle : SafeHandle
	{
		// Methods
		internal CoTaskMemSafeHandle()
			: base(IntPtr.Zero, true)
		{
		}

		public static CoTaskMemSafeHandle Zero
		{
			get
			{
				return new CoTaskMemSafeHandle();
			}
		}

		// Properties
		public override bool IsInvalid
		{
			get
			{
				if (!base.IsClosed)
				{
					return (base.handle == IntPtr.Zero);
				}
				return true;
			}
		}

		internal IntPtr GetMemory()
		{
			return base.handle;
		}

		internal void SetMemory(IntPtr handle)
		{
			base.SetHandle(handle);
		}

		protected override bool ReleaseHandle()
		{
			Marshal.FreeCoTaskMem(base.handle);
			base.handle = IntPtr.Zero;
			return true;
		}
	}

	[SecurityCritical(SecurityCriticalScope.Everything)]
	internal sealed class CoTaskMemUnicodeSafeHandle : SafeHandle
	{
		// Methods
		internal CoTaskMemUnicodeSafeHandle()
			: base(IntPtr.Zero, true)
		{
		}

		internal CoTaskMemUnicodeSafeHandle(IntPtr handle, bool ownsHandle)
			: base(IntPtr.Zero, ownsHandle)
		{
			base.SetHandle(handle);
		}

		public static CoTaskMemUnicodeSafeHandle Zero
		{
			get
			{
				return new CoTaskMemUnicodeSafeHandle();
			}
		}

		// Properties
		public override bool IsInvalid
		{
			get
			{
				if (!base.IsClosed)
				{
					return (base.handle == IntPtr.Zero);
				}
				return true;
			}
		}

		internal IntPtr GetMemory()
		{
			return base.handle;
		}

		internal void SetMemory(IntPtr handle)
		{
			base.SetHandle(handle);
		}

		protected override bool ReleaseHandle()
		{
			Marshal.ZeroFreeCoTaskMemUnicode(base.handle);
			base.handle = IntPtr.Zero;
			return true;
		}
	}
	[SecurityTreatAsSafe, SecurityCritical(SecurityCriticalScope.Everything)]
	internal sealed class EventLogHandle : SafeHandle
	{
		internal EventLogHandle(IntPtr handle, bool ownsHandle)
			: base(IntPtr.Zero, ownsHandle)
		{
			base.SetHandle(handle);
		}

		// Methods
		private EventLogHandle()
			: base(IntPtr.Zero, true)
		{
		}

		public static EventLogHandle Zero
		{
			get
			{
				return new EventLogHandle();
			}
		}

		// Properties
		public override bool IsInvalid
		{
			get
			{
				if (!base.IsClosed)
				{
					return (base.handle == IntPtr.Zero);
				}
				return true;
			}
		}

		protected override bool ReleaseHandle()
		{
			NativeWrapper.EvtClose(base.handle);
			base.handle = IntPtr.Zero;
			return true;
		}
	}
	internal class EventLogPermissionHolder
	{
		// Methods
		public static EventLogPermission GetEventLogPermission()
		{
			EventLogPermission permission = new EventLogPermission();
			EventLogPermissionEntry entry = new EventLogPermissionEntry(EventLogPermissionAccess.Administer, ".");
			permission.PermissionEntries.Add(entry);
			return permission;
		}
	}

	internal class ProviderMetadataCachedInformation
	{
		// Fields
		private Dictionary<ProviderMetadataId, CacheItem> cache;

		private string logfile;
		private int maximumCacheSize;
		private EventLogSession session;

		// Methods
		public ProviderMetadataCachedInformation(EventLogSession session, string logfile, int maximumCacheSize)
		{
			this.session = session;
			this.logfile = logfile;
			this.cache = new Dictionary<ProviderMetadataId, CacheItem>();
			this.maximumCacheSize = maximumCacheSize;
		}

		[SecurityTreatAsSafe]
		public string GetFormatDescription(string ProviderName, EventLogHandle eventHandle)
		{
			string str;
			lock (this)
			{
				ProviderMetadataId key = new ProviderMetadataId(ProviderName, CultureInfo.CurrentCulture);
				try
				{
					str = NativeWrapper.EvtFormatMessageRenderName(this.GetProviderMetadata(key).Handle, eventHandle, UnsafeNativeMethods.EvtFormatMessageFlags.EvtFormatMessageEvent);
				}
				catch (EventLogNotFoundException)
				{
					str = null;
				}
			}
			return str;
		}

		public string GetFormatDescription(string ProviderName, EventLogHandle eventHandle, string[] values)
		{
			string str;
			lock (this)
			{
				ProviderMetadataId key = new ProviderMetadataId(ProviderName, CultureInfo.CurrentCulture);
				ProviderMetadata providerMetadata = this.GetProviderMetadata(key);
				try
				{
					str = NativeWrapper.EvtFormatMessageFormatDescription(providerMetadata.Handle, eventHandle, values);
				}
				catch (EventLogNotFoundException)
				{
					str = null;
				}
			}
			return str;
		}

		[SecurityTreatAsSafe]
		public IEnumerable<string> GetKeywordDisplayNames(string ProviderName, [SecurityTreatAsSafe] EventLogHandle eventHandle)
		{
			lock (this)
			{
				ProviderMetadataId key = new ProviderMetadataId(ProviderName, CultureInfo.CurrentCulture);
				return NativeWrapper.EvtFormatMessageRenderKeywords(this.GetProviderMetadata(key).Handle, eventHandle, UnsafeNativeMethods.EvtFormatMessageFlags.EvtFormatMessageKeyword);
			}
		}

		[SecurityTreatAsSafe]
		public string GetLevelDisplayName(string ProviderName, [SecurityTreatAsSafe] EventLogHandle eventHandle)
		{
			lock (this)
			{
				ProviderMetadataId key = new ProviderMetadataId(ProviderName, CultureInfo.CurrentCulture);
				return NativeWrapper.EvtFormatMessageRenderName(this.GetProviderMetadata(key).Handle, eventHandle, UnsafeNativeMethods.EvtFormatMessageFlags.EvtFormatMessageLevel);
			}
		}

		[SecurityTreatAsSafe]
		public string GetOpcodeDisplayName(string ProviderName, [SecurityTreatAsSafe] EventLogHandle eventHandle)
		{
			lock (this)
			{
				ProviderMetadataId key = new ProviderMetadataId(ProviderName, CultureInfo.CurrentCulture);
				return NativeWrapper.EvtFormatMessageRenderName(this.GetProviderMetadata(key).Handle, eventHandle, UnsafeNativeMethods.EvtFormatMessageFlags.EvtFormatMessageOpcode);
			}
		}

		[SecurityTreatAsSafe]
		public string GetTaskDisplayName(string ProviderName, [SecurityTreatAsSafe] EventLogHandle eventHandle)
		{
			lock (this)
			{
				ProviderMetadataId key = new ProviderMetadataId(ProviderName, CultureInfo.CurrentCulture);
				return NativeWrapper.EvtFormatMessageRenderName(this.GetProviderMetadata(key).Handle, eventHandle, UnsafeNativeMethods.EvtFormatMessageFlags.EvtFormatMessageTask);
			}
		}

		private static void UpdateCacheValueInfoForHit(CacheItem cacheItem)
		{
			cacheItem.TheTime = DateTime.Now;
		}

		private void AddCacheEntry(ProviderMetadataId key, ProviderMetadata pm)
		{
			if (this.IsCacheFull())
			{
				this.FlushOldestEntry();
			}
			CacheItem item = new CacheItem(pm);
			this.cache.Add(key, item);
		}

		private void DeleteCacheEntry(ProviderMetadataId key)
		{
			if (this.IsProviderinCache(key))
			{
				CacheItem item = this.cache[key];
				this.cache.Remove(key);
				item.ProviderMetadata.Dispose();
			}
		}

		private void FlushOldestEntry()
		{
			double totalMilliseconds = -10.0;
			DateTime now = DateTime.Now;
			ProviderMetadataId key = null;
			foreach (KeyValuePair<ProviderMetadataId, CacheItem> pair in this.cache)
			{
				TimeSpan span = now.Subtract(pair.Value.TheTime);
				if (span.TotalMilliseconds >= totalMilliseconds)
				{
					totalMilliseconds = span.TotalMilliseconds;
					key = pair.Key;
				}
			}
			if (key != null)
			{
				this.DeleteCacheEntry(key);
			}
		}

		private ProviderMetadata GetProviderMetadata(ProviderMetadataId key)
		{
			if (!this.IsProviderinCache(key))
			{
				ProviderMetadata metadata;
				try
				{
					metadata = new ProviderMetadata(key.ProviderName, this.session, key.TheCultureInfo, this.logfile);
				}
				catch (EventLogNotFoundException)
				{
					metadata = new ProviderMetadata(key.ProviderName, this.session, key.TheCultureInfo);
				}
				this.AddCacheEntry(key, metadata);
				return metadata;
			}
			CacheItem cacheItem = this.cache[key];
			ProviderMetadata providerMetadata = cacheItem.ProviderMetadata;
			try
			{
				providerMetadata.CheckReleased();
				UpdateCacheValueInfoForHit(cacheItem);
			}
			catch (EventLogException)
			{
				this.DeleteCacheEntry(key);
				try
				{
					providerMetadata = new ProviderMetadata(key.ProviderName, this.session, key.TheCultureInfo, this.logfile);
				}
				catch (EventLogNotFoundException)
				{
					providerMetadata = new ProviderMetadata(key.ProviderName, this.session, key.TheCultureInfo);
				}
				this.AddCacheEntry(key, providerMetadata);
			}
			return providerMetadata;
		}

		private bool IsCacheFull()
		{
			return (this.cache.Count == this.maximumCacheSize);
		}

		private bool IsProviderinCache(ProviderMetadataId key)
		{
			return this.cache.ContainsKey(key);
		}

		// Nested Types
		private class CacheItem
		{
			// Fields
			private ProviderMetadata pm;

			private DateTime theTime;

			// Methods
			public CacheItem(ProviderMetadata pm)
			{
				this.pm = pm;
				this.theTime = DateTime.Now;
			}

			// Properties
			public ProviderMetadata ProviderMetadata
			{
				get
				{
					return this.pm;
				}
			}

			public DateTime TheTime
			{
				get
				{
					return this.theTime;
				}
				set
				{
					this.theTime = value;
				}
			}
		}

		private class ProviderMetadataId
		{
			// Fields
			private CultureInfo cultureInfo;

			private string providerName;

			// Methods
			public ProviderMetadataId(string providerName, CultureInfo cultureInfo)
			{
				this.providerName = providerName;
				this.cultureInfo = cultureInfo;
			}

			// Properties
			public string ProviderName
			{
				get
				{
					return this.providerName;
				}
			}

			public CultureInfo TheCultureInfo
			{
				get
				{
					return this.cultureInfo;
				}
			}

			public override bool Equals(object obj)
			{
				ProviderMetadataCachedInformation.ProviderMetadataId id = obj as ProviderMetadataCachedInformation.ProviderMetadataId;
				if (id == null)
				{
					return false;
				}
				return (this.providerName.Equals(id.providerName) && (this.cultureInfo == id.cultureInfo));
			}

			public override int GetHashCode()
			{
				return (this.providerName.GetHashCode() ^ this.cultureInfo.GetHashCode());
			}
		}
	}
	internal class NativeWrapper
	{
		// Fields
		private static bool s_platformNotSupported = (Environment.OSVersion.Version.Major < 6);

		// Methods
		[SecurityCritical]
		public static DateTime ConvertFileTimeToDateTime(UnsafeNativeMethods.EvtVariant val)
		{
			if (val.Type != 0x11)
			{
				throw new EventLogInvalidDataException();
			}
			return DateTime.FromFileTime((long)val.FileTime);
		}

		[SecurityCritical]
		public static string ConvertToAnsiString(UnsafeNativeMethods.EvtVariant val)
		{
			if (val.Type != 2)
			{
				throw new EventLogInvalidDataException();
			}
			if (val.AnsiString == IntPtr.Zero)
			{
				return string.Empty;
			}
			return Marshal.PtrToStringAuto(val.AnsiString);
		}

		[SecurityCritical]
		public static byte[] ConvertToBinaryArray(UnsafeNativeMethods.EvtVariant val)
		{
			if (val.Type != 14)
			{
				throw new EventLogInvalidDataException();
			}
			if (val.Binary == IntPtr.Zero)
			{
				return new byte[0];
			}
			IntPtr binary = val.Binary;
			byte[] destination = new byte[val.Count];
			Marshal.Copy(binary, destination, 0, (int)val.Count);
			return destination;
		}

		[SecurityCritical]
		public static Guid ConvertToGuid(UnsafeNativeMethods.EvtVariant val)
		{
			if (val.Type != 15)
			{
				throw new EventLogInvalidDataException();
			}
			if (val.GuidReference == IntPtr.Zero)
			{
				return Guid.Empty;
			}
			return (Guid)Marshal.PtrToStructure(val.GuidReference, typeof(Guid));
		}

		[SecurityCritical]
		public static int[] ConvertToIntArray(UnsafeNativeMethods.EvtVariant val)
		{
			if (val.Type != 0x88)
			{
				throw new EventLogInvalidDataException();
			}
			if (val.Reference == IntPtr.Zero)
			{
				return new int[0];
			}
			IntPtr reference = val.Reference;
			int[] destination = new int[val.Count];
			Marshal.Copy(reference, destination, 0, (int)val.Count);
			return destination;
		}

		[SecurityCritical]
		public static object ConvertToObject(UnsafeNativeMethods.EvtVariant val, UnsafeNativeMethods.EvtVariantType desiredType)
		{
			if (val.Type == 0)
			{
				return null;
			}
			if (val.Type != ((long)desiredType))
			{
				throw new EventLogInvalidDataException();
			}
			return ConvertToObject(val);
		}

		[SecurityCritical]
		public static EventLogHandle ConvertToSafeHandle(UnsafeNativeMethods.EvtVariant val)
		{
			if (val.Type != 0x20)
			{
				throw new EventLogInvalidDataException();
			}
			if (val.Handle == IntPtr.Zero)
			{
				return EventLogHandle.Zero;
			}
			return new EventLogHandle(val.Handle, true);
		}

		[SecurityCritical]
		public static SecurityIdentifier ConvertToSid(UnsafeNativeMethods.EvtVariant val)
		{
			if (val.Type != 0x13)
			{
				throw new EventLogInvalidDataException();
			}
			if (val.SidVal == IntPtr.Zero)
			{
				return null;
			}
			return new SecurityIdentifier(val.SidVal);
		}

		[SecurityCritical]
		public static string ConvertToString(UnsafeNativeMethods.EvtVariant val)
		{
			if (val.Type != 1)
			{
				throw new EventLogInvalidDataException();
			}
			if (val.StringVal == IntPtr.Zero)
			{
				return string.Empty;
			}
			return Marshal.PtrToStringAuto(val.StringVal);
		}

		[SecurityCritical]
		public static string[] ConvertToStringArray(UnsafeNativeMethods.EvtVariant val)
		{
			if (val.Type != 0x81)
			{
				throw new EventLogInvalidDataException();
			}
			if (val.Reference == IntPtr.Zero)
			{
				return new string[0];
			}
			IntPtr reference = val.Reference;
			IntPtr[] destination = new IntPtr[val.Count];
			Marshal.Copy(reference, destination, 0, (int)val.Count);
			string[] strArray = new string[val.Count];
			for (int i = 0; i < val.Count; i++)
			{
				strArray[i] = Marshal.PtrToStringAuto(destination[i]);
			}
			return strArray;
		}

		[SecurityCritical, SecurityTreatAsSafe]
		public static void EvtArchiveExportedLog(EventLogHandle session, string logFilePath, int locale, int flags)
		{
			if (s_platformNotSupported)
			{
				throw new PlatformNotSupportedException();
			}
			EventLogPermissionHolder.GetEventLogPermission().Demand();
			bool flag = UnsafeNativeMethods.EvtArchiveExportedLog(session, logFilePath, locale, flags);
			int errorCode = Marshal.GetLastWin32Error();
			if (!flag)
			{
				EventLogException.Throw(errorCode);
			}
		}

		[SecurityTreatAsSafe, SecurityCritical]
		public static void EvtCancel(EventLogHandle handle)
		{
			EventLogPermissionHolder.GetEventLogPermission().Demand();
			if (!UnsafeNativeMethods.EvtCancel(handle))
			{
				EventLogException.Throw(Marshal.GetLastWin32Error());
			}
		}

		[SecurityTreatAsSafe, SecurityCritical]
		public static void EvtClearLog(EventLogHandle session, string channelPath, string targetFilePath, int flags)
		{
			if (s_platformNotSupported)
			{
				throw new PlatformNotSupportedException();
			}
			EventLogPermissionHolder.GetEventLogPermission().Demand();
			bool flag = UnsafeNativeMethods.EvtClearLog(session, channelPath, targetFilePath, flags);
			int errorCode = Marshal.GetLastWin32Error();
			if (!flag)
			{
				EventLogException.Throw(errorCode);
			}
		}

		[SecurityCritical]
		public static void EvtClose(IntPtr handle)
		{
			UnsafeNativeMethods.EvtClose(handle);
		}

		[SecurityCritical]
		public static EventLogHandle EvtCreateBookmark(string bookmarkXml)
		{
			if (s_platformNotSupported)
			{
				throw new PlatformNotSupportedException();
			}
			EventLogHandle handle = UnsafeNativeMethods.EvtCreateBookmark(bookmarkXml);
			int errorCode = Marshal.GetLastWin32Error();
			if (handle.IsInvalid)
			{
				EventLogException.Throw(errorCode);
			}
			return handle;
		}

		[SecurityCritical]
		public static EventLogHandle EvtCreateRenderContext(int valuePathsCount, string[] valuePaths, UnsafeNativeMethods.EvtRenderContextFlags flags)
		{
			if (s_platformNotSupported)
			{
				throw new PlatformNotSupportedException();
			}
			EventLogHandle handle = UnsafeNativeMethods.EvtCreateRenderContext(valuePathsCount, valuePaths, flags);
			int errorCode = Marshal.GetLastWin32Error();
			if (handle.IsInvalid)
			{
				EventLogException.Throw(errorCode);
			}
			return handle;
		}

		[SecurityCritical, SecurityTreatAsSafe]
		public static void EvtExportLog(EventLogHandle session, string channelPath, string query, string targetFilePath, int flags)
		{
			if (s_platformNotSupported)
			{
				throw new PlatformNotSupportedException();
			}
			EventLogPermissionHolder.GetEventLogPermission().Demand();
			bool flag = UnsafeNativeMethods.EvtExportLog(session, channelPath, query, targetFilePath, flags);
			int errorCode = Marshal.GetLastWin32Error();
			if (!flag)
			{
				EventLogException.Throw(errorCode);
			}
		}

		[SecurityCritical]
		public static string EvtFormatMessage(EventLogHandle handle, uint msgId)
		{
			int num;
			if (s_platformNotSupported)
			{
				throw new PlatformNotSupportedException();
			}
			StringBuilder buffer = new StringBuilder(null);
			bool flag = UnsafeNativeMethods.EvtFormatMessage(handle, EventLogHandle.Zero, msgId, 0, null, UnsafeNativeMethods.EvtFormatMessageFlags.EvtFormatMessageId, 0, buffer, out num);
			int errorCode = Marshal.GetLastWin32Error();
			if (!flag && (errorCode != 0x3ab5))
			{
				if (errorCode == 0x3ab3)
				{
					return null;
				}
				if (errorCode != 0x7a)
				{
					EventLogException.Throw(errorCode);
				}
			}
			buffer.EnsureCapacity(num);
			flag = UnsafeNativeMethods.EvtFormatMessage(handle, EventLogHandle.Zero, msgId, 0, null, UnsafeNativeMethods.EvtFormatMessageFlags.EvtFormatMessageId, num, buffer, out num);
			errorCode = Marshal.GetLastWin32Error();
			if (!flag && (errorCode != 0x3ab5))
			{
				if (errorCode == 0x3ab3)
				{
					return null;
				}
				if (errorCode == 0x3ab5)
				{
					return null;
				}
				EventLogException.Throw(errorCode);
			}
			return buffer.ToString();
		}

		[SecurityTreatAsSafe, SecurityCritical]
		public static string EvtFormatMessageFormatDescription(EventLogHandle handle, EventLogHandle eventHandle, string[] values)
		{
			int num;
			if (s_platformNotSupported)
			{
				throw new PlatformNotSupportedException();
			}
			EventLogPermissionHolder.GetEventLogPermission().Demand();
			UnsafeNativeMethods.EvtStringVariant[] variantArray = new UnsafeNativeMethods.EvtStringVariant[values.Length];
			for (int i = 0; i < values.Length; i++)
			{
				variantArray[i].Type = 1;
				variantArray[i].StringVal = values[i];
			}
			StringBuilder buffer = new StringBuilder(null);
			bool flag = UnsafeNativeMethods.EvtFormatMessage(handle, eventHandle, uint.MaxValue, values.Length, variantArray, UnsafeNativeMethods.EvtFormatMessageFlags.EvtFormatMessageEvent, 0, buffer, out num);
			int errorCode = Marshal.GetLastWin32Error();
			if (!flag && (errorCode != 0x3ab5))
			{
				switch (errorCode)
				{
					case 0x3ab9:
					case 0x3afc:
					case 0x3ab3:
					case 0x3ab4:
					case 0x717:
						return null;
				}
				if (errorCode != 0x7a)
				{
					EventLogException.Throw(errorCode);
				}
			}
			buffer.EnsureCapacity(num);
			flag = UnsafeNativeMethods.EvtFormatMessage(handle, eventHandle, uint.MaxValue, values.Length, variantArray, UnsafeNativeMethods.EvtFormatMessageFlags.EvtFormatMessageEvent, num, buffer, out num);
			errorCode = Marshal.GetLastWin32Error();
			if (!flag && (errorCode != 0x3ab5))
			{
				if (errorCode == 0x3ab3)
				{
					return null;
				}
				EventLogException.Throw(errorCode);
			}
			return buffer.ToString();
		}

		[SecurityCritical, SecurityTreatAsSafe]
		public static IEnumerable<string> EvtFormatMessageRenderKeywords(EventLogHandle pmHandle, EventLogHandle eventHandle, UnsafeNativeMethods.EvtFormatMessageFlags flag)
		{
			IEnumerable<string> enumerable;
			EventLogPermissionHolder.GetEventLogPermission().Demand();
			IntPtr zero = IntPtr.Zero;
			try
			{
				int num;
				List<string> list = new List<string>();
				bool flag2 = UnsafeNativeMethods.EvtFormatMessageBuffer(pmHandle, eventHandle, 0, 0, IntPtr.Zero, flag, 0, IntPtr.Zero, out num);
				int errorCode = Marshal.GetLastWin32Error();
				if (!flag2)
				{
					switch (errorCode)
					{
						case 0x3ab9:
						case 0x3afc:
						case 0x3ab3:
						case 0x3ab4:
						case 0x717:
							return list.AsReadOnly();
					}
					if (errorCode != 0x7a)
					{
						EventLogException.Throw(errorCode);
					}
				}
				zero = Marshal.AllocHGlobal((int)(num * 2));
				flag2 = UnsafeNativeMethods.EvtFormatMessageBuffer(pmHandle, eventHandle, 0, 0, IntPtr.Zero, flag, num, zero, out num);
				errorCode = Marshal.GetLastWin32Error();
				if (!flag2)
				{
					switch (errorCode)
					{
						case 0x3ab9:
						case 0x3afc:
							return list;

						case 0x3ab3:
						case 0x3ab4:
							return list;

						case 0x717:
							return list;
					}
					EventLogException.Throw(errorCode);
				}
				IntPtr ptr = zero;
				while (true)
				{
					string str = Marshal.PtrToStringAuto(ptr);
					if (string.IsNullOrEmpty(str))
					{
						break;
					}
					list.Add(str);
					ptr = new IntPtr((((long)ptr) + (str.Length * 2)) + 2L);
				}
				enumerable = list.AsReadOnly();
			}
			finally
			{
				if (zero != IntPtr.Zero)
				{
					Marshal.FreeHGlobal(zero);
				}
			}
			return enumerable;
		}

		[SecurityCritical, SecurityTreatAsSafe]
		public static string EvtFormatMessageRenderName(EventLogHandle pmHandle, EventLogHandle eventHandle, UnsafeNativeMethods.EvtFormatMessageFlags flag)
		{
			int num;
			EventLogPermissionHolder.GetEventLogPermission().Demand();
			StringBuilder buffer = new StringBuilder(null);
			bool flag2 = UnsafeNativeMethods.EvtFormatMessage(pmHandle, eventHandle, 0, 0, null, flag, 0, buffer, out num);
			int errorCode = Marshal.GetLastWin32Error();
			if (!flag2 && (errorCode != 0x3ab5))
			{
				switch (errorCode)
				{
					case 0x3ab9:
					case 0x3afc:
					case 0x3ab3:
					case 0x3ab4:
					case 0x717:
						return null;
				}
				if (errorCode != 0x7a)
				{
					EventLogException.Throw(errorCode);
				}
			}
			buffer.EnsureCapacity(num);
			flag2 = UnsafeNativeMethods.EvtFormatMessage(pmHandle, eventHandle, 0, 0, null, flag, num, buffer, out num);
			errorCode = Marshal.GetLastWin32Error();
			if (!flag2 && (errorCode != 0x3ab5))
			{
				switch (errorCode)
				{
					case 0x3ab9:
					case 0x3afc:
					case 0x3ab3:
					case 0x3ab4:
					case 0x717:
						return null;
				}
				EventLogException.Throw(errorCode);
			}
			return buffer.ToString();
		}

		[SecurityTreatAsSafe, SecurityCritical]
		public static object EvtGetChannelConfigProperty(EventLogHandle handle, UnsafeNativeMethods.EvtChannelConfigPropertyId enumType)
		{
			object obj2;
			IntPtr zero = IntPtr.Zero;
			EventLogPermissionHolder.GetEventLogPermission().Demand();
			try
			{
				int num;
				bool flag = UnsafeNativeMethods.EvtGetChannelConfigProperty(handle, enumType, 0, 0, IntPtr.Zero, out num);
				int errorCode = Marshal.GetLastWin32Error();
				if (!flag && (errorCode != 0x7a))
				{
					EventLogException.Throw(errorCode);
				}
				zero = Marshal.AllocHGlobal(num);
				flag = UnsafeNativeMethods.EvtGetChannelConfigProperty(handle, enumType, 0, num, zero, out num);
				errorCode = Marshal.GetLastWin32Error();
				if (!flag)
				{
					EventLogException.Throw(errorCode);
				}
				UnsafeNativeMethods.EvtVariant val = (UnsafeNativeMethods.EvtVariant)Marshal.PtrToStructure(zero, typeof(UnsafeNativeMethods.EvtVariant));
				obj2 = ConvertToObject(val);
			}
			finally
			{
				if (zero != IntPtr.Zero)
				{
					Marshal.FreeHGlobal(zero);
				}
			}
			return obj2;
		}

		[SecurityCritical, SecurityTreatAsSafe]
		public static object EvtGetEventInfo(EventLogHandle handle, UnsafeNativeMethods.EvtEventPropertyId enumType)
		{
			object obj2;
			IntPtr zero = IntPtr.Zero;
			EventLogPermissionHolder.GetEventLogPermission().Demand();
			try
			{
				int num;
				bool flag = UnsafeNativeMethods.EvtGetEventInfo(handle, enumType, 0, IntPtr.Zero, out num);
				int errorCode = Marshal.GetLastWin32Error();
				if ((!flag && (errorCode != 0)) && (errorCode != 0x7a))
				{
					EventLogException.Throw(errorCode);
				}
				zero = Marshal.AllocHGlobal(num);
				flag = UnsafeNativeMethods.EvtGetEventInfo(handle, enumType, num, zero, out num);
				errorCode = Marshal.GetLastWin32Error();
				if (!flag)
				{
					EventLogException.Throw(errorCode);
				}
				UnsafeNativeMethods.EvtVariant val = (UnsafeNativeMethods.EvtVariant)Marshal.PtrToStructure(zero, typeof(UnsafeNativeMethods.EvtVariant));
				obj2 = ConvertToObject(val);
			}
			finally
			{
				if (zero != IntPtr.Zero)
				{
					Marshal.FreeHGlobal(zero);
				}
			}
			return obj2;
		}

		[SecurityCritical]
		public static object EvtGetEventMetadataProperty(EventLogHandle handle, UnsafeNativeMethods.EvtEventMetadataPropertyId enumType)
		{
			object obj2;
			IntPtr zero = IntPtr.Zero;
			try
			{
				int num;
				bool flag = UnsafeNativeMethods.EvtGetEventMetadataProperty(handle, enumType, 0, 0, IntPtr.Zero, out num);
				int errorCode = Marshal.GetLastWin32Error();
				if (!flag && (errorCode != 0x7a))
				{
					EventLogException.Throw(errorCode);
				}
				zero = Marshal.AllocHGlobal(num);
				flag = UnsafeNativeMethods.EvtGetEventMetadataProperty(handle, enumType, 0, num, zero, out num);
				errorCode = Marshal.GetLastWin32Error();
				if (!flag)
				{
					EventLogException.Throw(errorCode);
				}
				UnsafeNativeMethods.EvtVariant val = (UnsafeNativeMethods.EvtVariant)Marshal.PtrToStructure(zero, typeof(UnsafeNativeMethods.EvtVariant));
				obj2 = ConvertToObject(val);
			}
			finally
			{
				if (zero != IntPtr.Zero)
				{
					Marshal.FreeHGlobal(zero);
				}
			}
			return obj2;
		}

		[SecurityCritical]
		public static object EvtGetLogInfo(EventLogHandle handle, UnsafeNativeMethods.EvtLogPropertyId enumType)
		{
			object obj2;
			IntPtr zero = IntPtr.Zero;
			try
			{
				int num;
				bool flag = UnsafeNativeMethods.EvtGetLogInfo(handle, enumType, 0, IntPtr.Zero, out num);
				int errorCode = Marshal.GetLastWin32Error();
				if (!flag && (errorCode != 0x7a))
				{
					EventLogException.Throw(errorCode);
				}
				zero = Marshal.AllocHGlobal(num);
				flag = UnsafeNativeMethods.EvtGetLogInfo(handle, enumType, num, zero, out num);
				errorCode = Marshal.GetLastWin32Error();
				if (!flag)
				{
					EventLogException.Throw(errorCode);
				}
				UnsafeNativeMethods.EvtVariant val = (UnsafeNativeMethods.EvtVariant)Marshal.PtrToStructure(zero, typeof(UnsafeNativeMethods.EvtVariant));
				obj2 = ConvertToObject(val);
			}
			finally
			{
				if (zero != IntPtr.Zero)
				{
					Marshal.FreeHGlobal(zero);
				}
			}
			return obj2;
		}

		[SecurityCritical]
		public static object EvtGetObjectArrayProperty(EventLogHandle objArrayHandle, int index, int thePropertyId)
		{
			object obj2;
			IntPtr zero = IntPtr.Zero;
			try
			{
				int num;
				bool flag = UnsafeNativeMethods.EvtGetObjectArrayProperty(objArrayHandle, thePropertyId, index, 0, 0, IntPtr.Zero, out num);
				int errorCode = Marshal.GetLastWin32Error();
				if (!flag && (errorCode != 0x7a))
				{
					EventLogException.Throw(errorCode);
				}
				zero = Marshal.AllocHGlobal(num);
				flag = UnsafeNativeMethods.EvtGetObjectArrayProperty(objArrayHandle, thePropertyId, index, 0, num, zero, out num);
				errorCode = Marshal.GetLastWin32Error();
				if (!flag)
				{
					EventLogException.Throw(errorCode);
				}
				UnsafeNativeMethods.EvtVariant val = (UnsafeNativeMethods.EvtVariant)Marshal.PtrToStructure(zero, typeof(UnsafeNativeMethods.EvtVariant));
				obj2 = ConvertToObject(val);
			}
			finally
			{
				if (zero != IntPtr.Zero)
				{
					Marshal.FreeHGlobal(zero);
				}
			}
			return obj2;
		}

		[SecurityCritical]
		public static int EvtGetObjectArraySize(EventLogHandle objectArray)
		{
			int num;
			bool flag = UnsafeNativeMethods.EvtGetObjectArraySize(objectArray, out num);
			int errorCode = Marshal.GetLastWin32Error();
			if (!flag)
			{
				EventLogException.Throw(errorCode);
			}
			return num;
		}

		[SecurityCritical, SecurityTreatAsSafe]
		public static object EvtGetPublisherMetadataProperty(EventLogHandle pmHandle, UnsafeNativeMethods.EvtPublisherMetadataPropertyId thePropertyId)
		{
			object obj2;
			IntPtr zero = IntPtr.Zero;
			EventLogPermissionHolder.GetEventLogPermission().Demand();
			try
			{
				int num;
				bool flag = UnsafeNativeMethods.EvtGetPublisherMetadataProperty(pmHandle, thePropertyId, 0, 0, IntPtr.Zero, out num);
				int errorCode = Marshal.GetLastWin32Error();
				if (!flag && (errorCode != 0x7a))
				{
					EventLogException.Throw(errorCode);
				}
				zero = Marshal.AllocHGlobal(num);
				flag = UnsafeNativeMethods.EvtGetPublisherMetadataProperty(pmHandle, thePropertyId, 0, num, zero, out num);
				errorCode = Marshal.GetLastWin32Error();
				if (!flag)
				{
					EventLogException.Throw(errorCode);
				}
				UnsafeNativeMethods.EvtVariant val = (UnsafeNativeMethods.EvtVariant)Marshal.PtrToStructure(zero, typeof(UnsafeNativeMethods.EvtVariant));
				obj2 = ConvertToObject(val);
			}
			finally
			{
				if (zero != IntPtr.Zero)
				{
					Marshal.FreeHGlobal(zero);
				}
			}
			return obj2;
		}

		[SecurityCritical]
		public static object EvtGetQueryInfo(EventLogHandle handle, UnsafeNativeMethods.EvtQueryPropertyId enumType)
		{
			object obj2;
			IntPtr zero = IntPtr.Zero;
			int bufferRequired = 0;
			try
			{
				bool flag = UnsafeNativeMethods.EvtGetQueryInfo(handle, enumType, 0, IntPtr.Zero, ref bufferRequired);
				int errorCode = Marshal.GetLastWin32Error();
				if (!flag && (errorCode != 0x7a))
				{
					EventLogException.Throw(errorCode);
				}
				zero = Marshal.AllocHGlobal(bufferRequired);
				flag = UnsafeNativeMethods.EvtGetQueryInfo(handle, enumType, bufferRequired, zero, ref bufferRequired);
				errorCode = Marshal.GetLastWin32Error();
				if (!flag)
				{
					EventLogException.Throw(errorCode);
				}
				UnsafeNativeMethods.EvtVariant val = (UnsafeNativeMethods.EvtVariant)Marshal.PtrToStructure(zero, typeof(UnsafeNativeMethods.EvtVariant));
				obj2 = ConvertToObject(val);
			}
			finally
			{
				if (zero != IntPtr.Zero)
				{
					Marshal.FreeHGlobal(zero);
				}
			}
			return obj2;
		}

		[SecurityCritical]
		public static bool EvtNext(EventLogHandle queryHandle, int eventSize, IntPtr[] events, int timeout, int flags, ref int returned)
		{
			bool flag = UnsafeNativeMethods.EvtNext(queryHandle, eventSize, events, timeout, flags, ref returned);
			int errorCode = Marshal.GetLastWin32Error();
			if (!flag && (errorCode != 0x103))
			{
				EventLogException.Throw(errorCode);
			}
			return (errorCode == 0);
		}

		[SecurityCritical]
		public static string EvtNextChannelPath(EventLogHandle handle, ref bool finish)
		{
			int num;
			StringBuilder channelPathBuffer = new StringBuilder(null);
			bool flag = UnsafeNativeMethods.EvtNextChannelPath(handle, 0, channelPathBuffer, out num);
			int errorCode = Marshal.GetLastWin32Error();
			if (!flag)
			{
				if (errorCode == 0x103)
				{
					finish = true;
					return null;
				}
				if (errorCode != 0x7a)
				{
					EventLogException.Throw(errorCode);
				}
			}
			channelPathBuffer.EnsureCapacity(num);
			flag = UnsafeNativeMethods.EvtNextChannelPath(handle, num, channelPathBuffer, out num);
			errorCode = Marshal.GetLastWin32Error();
			if (!flag)
			{
				EventLogException.Throw(errorCode);
			}
			return channelPathBuffer.ToString();
		}

		[SecurityCritical]
		public static EventLogHandle EvtNextEventMetadata(EventLogHandle eventMetadataEnum, int flags)
		{
			EventLogHandle handle = UnsafeNativeMethods.EvtNextEventMetadata(eventMetadataEnum, flags);
			int errorCode = Marshal.GetLastWin32Error();
			if (!handle.IsInvalid)
			{
				return handle;
			}
			if (errorCode != 0x103)
			{
				EventLogException.Throw(errorCode);
			}
			return null;
		}

		[SecurityCritical]
		public static string EvtNextPublisherId(EventLogHandle handle, ref bool finish)
		{
			int num;
			StringBuilder publisherIdBuffer = new StringBuilder(null);
			bool flag = UnsafeNativeMethods.EvtNextPublisherId(handle, 0, publisherIdBuffer, out num);
			int errorCode = Marshal.GetLastWin32Error();
			if (!flag)
			{
				if (errorCode == 0x103)
				{
					finish = true;
					return null;
				}
				if (errorCode != 0x7a)
				{
					EventLogException.Throw(errorCode);
				}
			}
			publisherIdBuffer.EnsureCapacity(num);
			flag = UnsafeNativeMethods.EvtNextPublisherId(handle, num, publisherIdBuffer, out num);
			errorCode = Marshal.GetLastWin32Error();
			if (!flag)
			{
				EventLogException.Throw(errorCode);
			}
			return publisherIdBuffer.ToString();
		}

		[SecurityCritical]
		public static EventLogHandle EvtOpenChannelConfig(EventLogHandle session, string channelPath, int flags)
		{
			if (s_platformNotSupported)
			{
				throw new PlatformNotSupportedException();
			}
			EventLogHandle handle = UnsafeNativeMethods.EvtOpenChannelConfig(session, channelPath, flags);
			int errorCode = Marshal.GetLastWin32Error();
			if (handle.IsInvalid)
			{
				EventLogException.Throw(errorCode);
			}
			return handle;
		}

		[SecurityCritical]
		public static EventLogHandle EvtOpenChannelEnum(EventLogHandle session, int flags)
		{
			if (s_platformNotSupported)
			{
				throw new PlatformNotSupportedException();
			}
			EventLogHandle handle = UnsafeNativeMethods.EvtOpenChannelEnum(session, flags);
			int errorCode = Marshal.GetLastWin32Error();
			if (handle.IsInvalid)
			{
				EventLogException.Throw(errorCode);
			}
			return handle;
		}

		[SecurityCritical]
		public static EventLogHandle EvtOpenEventMetadataEnum(EventLogHandle ProviderMetadata, int flags)
		{
			EventLogHandle handle = UnsafeNativeMethods.EvtOpenEventMetadataEnum(ProviderMetadata, flags);
			int errorCode = Marshal.GetLastWin32Error();
			if (handle.IsInvalid)
			{
				EventLogException.Throw(errorCode);
			}
			return handle;
		}

		[SecurityCritical]
		public static EventLogHandle EvtOpenLog(EventLogHandle session, string path, PathType flags)
		{
			if (s_platformNotSupported)
			{
				throw new PlatformNotSupportedException();
			}
			EventLogHandle handle = UnsafeNativeMethods.EvtOpenLog(session, path, flags);
			int errorCode = Marshal.GetLastWin32Error();
			if (handle.IsInvalid)
			{
				EventLogException.Throw(errorCode);
			}
			return handle;
		}

		[SecurityCritical]
		public static EventLogHandle EvtOpenProviderEnum(EventLogHandle session, int flags)
		{
			if (s_platformNotSupported)
			{
				throw new PlatformNotSupportedException();
			}
			EventLogHandle handle = UnsafeNativeMethods.EvtOpenPublisherEnum(session, flags);
			int errorCode = Marshal.GetLastWin32Error();
			if (handle.IsInvalid)
			{
				EventLogException.Throw(errorCode);
			}
			return handle;
		}

		[SecurityCritical]
		public static EventLogHandle EvtOpenProviderMetadata(EventLogHandle session, string ProviderId, string logFilePath, int locale, int flags)
		{
			if (s_platformNotSupported)
			{
				throw new PlatformNotSupportedException();
			}
			EventLogHandle handle = UnsafeNativeMethods.EvtOpenPublisherMetadata(session, ProviderId, logFilePath, 0, flags);
			int errorCode = Marshal.GetLastWin32Error();
			if (handle.IsInvalid)
			{
				EventLogException.Throw(errorCode);
			}
			return handle;
		}

		[SecurityCritical]
		public static EventLogHandle EvtOpenSession(UnsafeNativeMethods.EvtLoginClass loginClass, ref UnsafeNativeMethods.EvtRpcLogin login, int timeout, int flags)
		{
			if (s_platformNotSupported)
			{
				throw new PlatformNotSupportedException();
			}
			EventLogHandle handle = UnsafeNativeMethods.EvtOpenSession(loginClass, ref login, timeout, flags);
			int errorCode = Marshal.GetLastWin32Error();
			if (handle.IsInvalid)
			{
				EventLogException.Throw(errorCode);
			}
			return handle;
		}

		[SecurityCritical]
		public static EventLogHandle EvtQuery(EventLogHandle session, string path, string query, int flags)
		{
			if (s_platformNotSupported)
			{
				throw new PlatformNotSupportedException();
			}
			EventLogHandle handle = UnsafeNativeMethods.EvtQuery(session, path, query, flags);
			int errorCode = Marshal.GetLastWin32Error();
			if (handle.IsInvalid)
			{
				EventLogException.Throw(errorCode);
			}
			return handle;
		}

		[SecurityCritical]
		public static void EvtRender(EventLogHandle context, EventLogHandle eventHandle, UnsafeNativeMethods.EvtRenderFlags flags, StringBuilder buffer)
		{
			int num;
			int num2;
			if (s_platformNotSupported)
			{
				throw new PlatformNotSupportedException();
			}
			bool flag = UnsafeNativeMethods.EvtRender(context, eventHandle, flags, buffer.Capacity, buffer, out num, out num2);
			int errorCode = Marshal.GetLastWin32Error();
			if (!flag)
			{
				if (errorCode == 0x7a)
				{
					buffer.Capacity = num;
					flag = UnsafeNativeMethods.EvtRender(context, eventHandle, flags, buffer.Capacity, buffer, out num, out num2);
					errorCode = Marshal.GetLastWin32Error();
				}
				if (!flag)
				{
					EventLogException.Throw(errorCode);
				}
			}
		}

		[SecurityCritical]
		public static string EvtRenderBookmark(EventLogHandle eventHandle)
		{
			string str;
			IntPtr zero = IntPtr.Zero;
			UnsafeNativeMethods.EvtRenderFlags evtRenderBookmark = UnsafeNativeMethods.EvtRenderFlags.EvtRenderBookmark;
			try
			{
				int num;
				int num2;
				bool flag = UnsafeNativeMethods.EvtRender(EventLogHandle.Zero, eventHandle, evtRenderBookmark, 0, IntPtr.Zero, out num, out num2);
				int errorCode = Marshal.GetLastWin32Error();
				if (!flag && (errorCode != 0x7a))
				{
					EventLogException.Throw(errorCode);
				}
				zero = Marshal.AllocHGlobal(num);
				flag = UnsafeNativeMethods.EvtRender(EventLogHandle.Zero, eventHandle, evtRenderBookmark, num, zero, out num, out num2);
				errorCode = Marshal.GetLastWin32Error();
				if (!flag)
				{
					EventLogException.Throw(errorCode);
				}
				str = Marshal.PtrToStringAuto(zero);
			}
			finally
			{
				if (zero != IntPtr.Zero)
				{
					Marshal.FreeHGlobal(zero);
				}
			}
			return str;
		}

		[SecurityCritical, SecurityTreatAsSafe]
		public static void EvtRenderBufferWithContextSystem(EventLogHandle contextHandle, EventLogHandle eventHandle, UnsafeNativeMethods.EvtRenderFlags flag, SystemProperties systemProperties, int SYSTEM_PROPERTY_COUNT)
		{
			IntPtr zero = IntPtr.Zero;
			IntPtr ptr = IntPtr.Zero;
			EventLogPermissionHolder.GetEventLogPermission().Demand();
			try
			{
				int num;
				int num2;
				if (!UnsafeNativeMethods.EvtRender(contextHandle, eventHandle, flag, 0, IntPtr.Zero, out num, out num2))
				{
					int num3 = Marshal.GetLastWin32Error();
					if (num3 != 0x7a)
					{
						EventLogException.Throw(num3);
					}
				}
				zero = Marshal.AllocHGlobal(num);
				bool flag2 = UnsafeNativeMethods.EvtRender(contextHandle, eventHandle, flag, num, zero, out num, out num2);
				int errorCode = Marshal.GetLastWin32Error();
				if (!flag2)
				{
					EventLogException.Throw(errorCode);
				}
				if (num2 != SYSTEM_PROPERTY_COUNT)
				{
					throw new InvalidOperationException("We do not have " + SYSTEM_PROPERTY_COUNT + " variants given for the  UnsafeNativeMethods.EvtRenderFlags.EvtRenderEventValues flag. (System Properties)");
				}
				ptr = zero;
				for (int i = 0; i < num2; i++)
				{
					UnsafeNativeMethods.EvtVariant val = (UnsafeNativeMethods.EvtVariant)Marshal.PtrToStructure(ptr, typeof(UnsafeNativeMethods.EvtVariant));
					switch (i)
					{
						case 0:
							systemProperties.ProviderName = (string)ConvertToObject(val, UnsafeNativeMethods.EvtVariantType.EvtVarTypeString);
							break;

						case 1:
							systemProperties.ProviderId = (Guid?)ConvertToObject(val, UnsafeNativeMethods.EvtVariantType.EvtVarTypeGuid);
							break;

						case 2:
							systemProperties.Id = (ushort?)ConvertToObject(val, UnsafeNativeMethods.EvtVariantType.EvtVarTypeUInt16);
							break;

						case 3:
							systemProperties.Qualifiers = (ushort?)ConvertToObject(val, UnsafeNativeMethods.EvtVariantType.EvtVarTypeUInt16);
							break;

						case 4:
							systemProperties.Level = (byte?)ConvertToObject(val, UnsafeNativeMethods.EvtVariantType.EvtVarTypeByte);
							break;

						case 5:
							systemProperties.Task = (ushort?)ConvertToObject(val, UnsafeNativeMethods.EvtVariantType.EvtVarTypeUInt16);
							break;

						case 6:
							systemProperties.Opcode = (byte?)ConvertToObject(val, UnsafeNativeMethods.EvtVariantType.EvtVarTypeByte);
							break;

						case 7:
							systemProperties.Keywords = (ulong?)ConvertToObject(val, UnsafeNativeMethods.EvtVariantType.EvtVarTypeHexInt64);
							break;

						case 8:
							systemProperties.TimeCreated = (DateTime?)ConvertToObject(val, UnsafeNativeMethods.EvtVariantType.EvtVarTypeFileTime);
							break;

						case 9:
							systemProperties.RecordId = (ulong?)ConvertToObject(val, UnsafeNativeMethods.EvtVariantType.EvtVarTypeUInt64);
							break;

						case 10:
							systemProperties.ActivityId = (Guid?)ConvertToObject(val, UnsafeNativeMethods.EvtVariantType.EvtVarTypeGuid);
							break;

						case 11:
							systemProperties.RelatedActivityId = (Guid?)ConvertToObject(val, UnsafeNativeMethods.EvtVariantType.EvtVarTypeGuid);
							break;

						case 12:
							systemProperties.ProcessId = (uint?)ConvertToObject(val, UnsafeNativeMethods.EvtVariantType.EvtVarTypeUInt32);
							break;

						case 13:
							systemProperties.ThreadId = (uint?)ConvertToObject(val, UnsafeNativeMethods.EvtVariantType.EvtVarTypeUInt32);
							break;

						case 14:
							systemProperties.ChannelName = (string)ConvertToObject(val, UnsafeNativeMethods.EvtVariantType.EvtVarTypeString);
							break;

						case 15:
							systemProperties.ComputerName = (string)ConvertToObject(val, UnsafeNativeMethods.EvtVariantType.EvtVarTypeString);
							break;

						case 0x10:
							systemProperties.UserId = (SecurityIdentifier)ConvertToObject(val, UnsafeNativeMethods.EvtVariantType.EvtVarTypeSid);
							break;

						case 0x11:
							systemProperties.Version = (byte?)ConvertToObject(val, UnsafeNativeMethods.EvtVariantType.EvtVarTypeByte);
							break;
					}
					ptr = new IntPtr(((long)ptr) + Marshal.SizeOf(val));
				}
			}
			finally
			{
				if (zero != IntPtr.Zero)
				{
					Marshal.FreeHGlobal(zero);
				}
			}
		}

		[SecurityCritical, SecurityTreatAsSafe]
		public static IList<object> EvtRenderBufferWithContextUserOrValues(EventLogHandle contextHandle, EventLogHandle eventHandle)
		{
			IList<object> list2;
			IntPtr zero = IntPtr.Zero;
			IntPtr ptr = IntPtr.Zero;
			UnsafeNativeMethods.EvtRenderFlags evtRenderEventValues = UnsafeNativeMethods.EvtRenderFlags.EvtRenderEventValues;
			EventLogPermissionHolder.GetEventLogPermission().Demand();
			try
			{
				int num;
				int num2;
				if (!UnsafeNativeMethods.EvtRender(contextHandle, eventHandle, evtRenderEventValues, 0, IntPtr.Zero, out num, out num2))
				{
					int num3 = Marshal.GetLastWin32Error();
					if (num3 != 0x7a)
					{
						EventLogException.Throw(num3);
					}
				}
				zero = Marshal.AllocHGlobal(num);
				bool flag = UnsafeNativeMethods.EvtRender(contextHandle, eventHandle, evtRenderEventValues, num, zero, out num, out num2);
				int errorCode = Marshal.GetLastWin32Error();
				if (!flag)
				{
					EventLogException.Throw(errorCode);
				}
				List<object> list = new List<object>(num2);
				if (num2 > 0)
				{
					ptr = zero;
					for (int i = 0; i < num2; i++)
					{
						UnsafeNativeMethods.EvtVariant val = (UnsafeNativeMethods.EvtVariant)Marshal.PtrToStructure(ptr, typeof(UnsafeNativeMethods.EvtVariant));
						list.Add(ConvertToObject(val));
						ptr = new IntPtr(((long)ptr) + Marshal.SizeOf(val));
					}
				}
				list2 = list;
			}
			finally
			{
				if (zero != IntPtr.Zero)
				{
					Marshal.FreeHGlobal(zero);
				}
			}
			return list2;
		}

		[SecurityCritical, SecurityTreatAsSafe]
		public static void EvtSaveChannelConfig(EventLogHandle channelConfig, int flags)
		{
			EventLogPermissionHolder.GetEventLogPermission().Demand();
			bool flag = UnsafeNativeMethods.EvtSaveChannelConfig(channelConfig, flags);
			int errorCode = Marshal.GetLastWin32Error();
			if (!flag)
			{
				EventLogException.Throw(errorCode);
			}
		}

		[SecurityCritical]
		public static void EvtSeek(EventLogHandle resultSet, long position, EventLogHandle bookmark, int timeout, UnsafeNativeMethods.EvtSeekFlags flags)
		{
			bool flag = UnsafeNativeMethods.EvtSeek(resultSet, position, bookmark, timeout, flags);
			int errorCode = Marshal.GetLastWin32Error();
			if (!flag)
			{
				EventLogException.Throw(errorCode);
			}
		}

		[SecurityTreatAsSafe, SecurityCritical]
		public static void EvtSetChannelConfigProperty(EventLogHandle handle, UnsafeNativeMethods.EvtChannelConfigPropertyId enumType, object val)
		{
			EventLogPermissionHolder.GetEventLogPermission().Demand();
			UnsafeNativeMethods.EvtVariant propertyValue = new UnsafeNativeMethods.EvtVariant();
			CoTaskMemSafeHandle handle2 = new CoTaskMemSafeHandle();
			using (handle2)
			{
				bool flag;
				if (val == null)
				{
					goto Label_017B;
				}
				switch (enumType)
				{
					case UnsafeNativeMethods.EvtChannelConfigPropertyId.EvtChannelConfigEnabled:
						propertyValue.Type = 13;
						if (!((bool)val))
						{
							break;
						}
						propertyValue.Bool = 1;
						goto Label_0183;

					case UnsafeNativeMethods.EvtChannelConfigPropertyId.EvtChannelConfigAccess:
						propertyValue.Type = 1;
						handle2.SetMemory(Marshal.StringToCoTaskMemAuto((string)val));
						propertyValue.StringVal = handle2.GetMemory();
						goto Label_0183;

					case UnsafeNativeMethods.EvtChannelConfigPropertyId.EvtChannelLoggingConfigRetention:
						propertyValue.Type = 13;
						if (!((bool)val))
						{
							goto Label_0146;
						}
						propertyValue.Bool = 1;
						goto Label_0183;

					case UnsafeNativeMethods.EvtChannelConfigPropertyId.EvtChannelLoggingConfigAutoBackup:
						propertyValue.Type = 13;
						if (!((bool)val))
						{
							goto Label_016B;
						}
						propertyValue.Bool = 1;
						goto Label_0183;

					case UnsafeNativeMethods.EvtChannelConfigPropertyId.EvtChannelLoggingConfigMaxSize:
						propertyValue.Type = 10;
						propertyValue.ULong = (ulong)((long)val);
						goto Label_0183;

					case UnsafeNativeMethods.EvtChannelConfigPropertyId.EvtChannelLoggingConfigLogFilePath:
						propertyValue.Type = 1;
						handle2.SetMemory(Marshal.StringToCoTaskMemAuto((string)val));
						propertyValue.StringVal = handle2.GetMemory();
						goto Label_0183;

					case UnsafeNativeMethods.EvtChannelConfigPropertyId.EvtChannelPublishingConfigLevel:
						propertyValue.Type = 8;
						propertyValue.UInteger = (uint)((int)val);
						goto Label_0183;

					case UnsafeNativeMethods.EvtChannelConfigPropertyId.EvtChannelPublishingConfigKeywords:
						propertyValue.Type = 10;
						propertyValue.ULong = (ulong)((long)val);
						goto Label_0183;

					default:
						throw new InvalidOperationException();
				}
				propertyValue.Bool = 0;
				goto Label_0183;
			Label_0146:
				propertyValue.Bool = 0;
				goto Label_0183;
			Label_016B:
				propertyValue.Bool = 0;
				goto Label_0183;
			Label_017B:
				propertyValue.Type = 0;
			Label_0183:
				flag = UnsafeNativeMethods.EvtSetChannelConfigProperty(handle, enumType, 0, ref propertyValue);
				int errorCode = Marshal.GetLastWin32Error();
				if (!flag)
				{
					EventLogException.Throw(errorCode);
				}
			}
		}

		[SecurityCritical]
		public static EventLogHandle EvtSubscribe(EventLogHandle session, SafeWaitHandle signalEvent, string path, string query, EventLogHandle bookmark, IntPtr context, IntPtr callback, int flags)
		{
			if (s_platformNotSupported)
			{
				throw new PlatformNotSupportedException();
			}
			EventLogHandle handle = UnsafeNativeMethods.EvtSubscribe(session, signalEvent, path, query, bookmark, context, callback, flags);
			int errorCode = Marshal.GetLastWin32Error();
			if (handle.IsInvalid)
			{
				EventLogException.Throw(errorCode);
			}
			return handle;
		}

		[SecurityCritical]
		public static void EvtUpdateBookmark(EventLogHandle bookmark, EventLogHandle eventHandle)
		{
			bool flag = UnsafeNativeMethods.EvtUpdateBookmark(bookmark, eventHandle);
			int errorCode = Marshal.GetLastWin32Error();
			if (!flag)
			{
				EventLogException.Throw(errorCode);
			}
		}

		[SecurityCritical]
		internal static EventLogHandle EvtGetPublisherMetadataPropertyHandle(EventLogHandle pmHandle, UnsafeNativeMethods.EvtPublisherMetadataPropertyId thePropertyId)
		{
			EventLogHandle handle;
			IntPtr zero = IntPtr.Zero;
			try
			{
				int num;
				bool flag = UnsafeNativeMethods.EvtGetPublisherMetadataProperty(pmHandle, thePropertyId, 0, 0, IntPtr.Zero, out num);
				int errorCode = Marshal.GetLastWin32Error();
				if (!flag && (errorCode != 0x7a))
				{
					EventLogException.Throw(errorCode);
				}
				zero = Marshal.AllocHGlobal(num);
				flag = UnsafeNativeMethods.EvtGetPublisherMetadataProperty(pmHandle, thePropertyId, 0, num, zero, out num);
				errorCode = Marshal.GetLastWin32Error();
				if (!flag)
				{
					EventLogException.Throw(errorCode);
				}
				UnsafeNativeMethods.EvtVariant val = (UnsafeNativeMethods.EvtVariant)Marshal.PtrToStructure(zero, typeof(UnsafeNativeMethods.EvtVariant));
				handle = ConvertToSafeHandle(val);
			}
			finally
			{
				if (zero != IntPtr.Zero)
				{
					Marshal.FreeHGlobal(zero);
				}
			}
			return handle;
		}

		[SecurityCritical]
		private static object ConvertToObject(UnsafeNativeMethods.EvtVariant val)
		{
			switch (val.Type)
			{
				case 0:
					return null;

				case 1:
					return ConvertToString(val);

				case 2:
					return ConvertToAnsiString(val);

				case 3:
					return val.SByte;

				case 4:
					return val.UInt8;

				case 5:
					return val.SByte;

				case 6:
					return val.UShort;

				case 7:
					return val.Integer;

				case 8:
					return val.UInteger;

				case 9:
					return val.Long;

				case 10:
					return val.ULong;

				case 12:
					return val.Double;

				case 13:
					if (val.Bool == 0)
					{
						return false;
					}
					return true;

				case 14:
					return ConvertToBinaryArray(val);

				case 15:
					return ConvertToGuid(val);

				case 0x11:
					return ConvertFileTimeToDateTime(val);

				case 0x13:
					return ConvertToSid(val);

				case 20:
					return val.Integer;

				case 0x15:
					return val.ULong;

				case 0x20:
					return ConvertToSafeHandle(val);

				case 0x81:
					return ConvertToStringArray(val);

				case 0x88:
					return ConvertToIntArray(val);
			}
			throw new EventLogInvalidDataException();
		}
		// Nested Types
		[HostProtection(SecurityAction.LinkDemand, MayLeakOnAbort = true)]
		internal class SystemProperties
		{
			// Fields
			public Guid? ActivityId = null;
			public string ChannelName;
			public string ComputerName;
			public bool filled;
			public ushort? Id = null;
			public ulong? Keywords = null;
			public byte? Level = null;
			public byte? Opcode = null;
			public uint? ProcessId = null;
			public Guid? ProviderId = null;
			public string ProviderName;
			public ushort? Qualifiers = null;
			public ulong? RecordId = null;
			public Guid? RelatedActivityId = null;
			public ushort? Task = null;
			public uint? ThreadId = null;
			public DateTime? TimeCreated = null;
			public SecurityIdentifier UserId;
			public byte? Version = null;
		}
	}

	internal static class UnsafeNativeMethods
	{
		internal enum EvtChannelConfigPropertyId
		{
			EvtChannelConfigEnabled,
			EvtChannelConfigIsolation,
			EvtChannelConfigType,
			EvtChannelConfigOwningPublisher,
			EvtChannelConfigClassicEventlog,
			EvtChannelConfigAccess,
			EvtChannelLoggingConfigRetention,
			EvtChannelLoggingConfigAutoBackup,
			EvtChannelLoggingConfigMaxSize,
			EvtChannelLoggingConfigLogFilePath,
			EvtChannelPublishingConfigLevel,
			EvtChannelPublishingConfigKeywords,
			EvtChannelPublishingConfigControlGuid,
			EvtChannelPublishingConfigBufferSize,
			EvtChannelPublishingConfigMinBuffers,
			EvtChannelPublishingConfigMaxBuffers,
			EvtChannelPublishingConfigLatency,
			EvtChannelPublishingConfigClockType,
			EvtChannelPublishingConfigSidType,
			EvtChannelPublisherList,
			EvtChannelConfigPropertyIdEND
		}

		internal enum EvtEventMetadataPropertyId
		{
			EventMetadataEventID,
			EventMetadataEventVersion,
			EventMetadataEventChannel,
			EventMetadataEventLevel,
			EventMetadataEventOpcode,
			EventMetadataEventTask,
			EventMetadataEventKeyword,
			EventMetadataEventMessageID,
			EventMetadataEventTemplate
		}

		internal enum EvtEventPropertyId
		{
			EvtEventQueryIDs,
			EvtEventPath
		}

		internal enum EvtExportLogFlags
		{
			EvtExportLogChannelPath = 1,
			EvtExportLogFilePath = 2,
			EvtExportLogTolerateQueryErrors = 0x1000
		}

		internal enum EvtFormatMessageFlags
		{
			EvtFormatMessageChannel = 6,
			EvtFormatMessageEvent = 1,
			EvtFormatMessageId = 8,
			EvtFormatMessageKeyword = 5,
			EvtFormatMessageLevel = 2,
			EvtFormatMessageOpcode = 4,
			EvtFormatMessageProvider = 7,
			EvtFormatMessageTask = 3,
			EvtFormatMessageXml = 9
		}

		internal enum EvtLoginClass
		{
			EvtRpcLogin = 1
		}

		internal enum EvtLogPropertyId
		{
			EvtLogCreationTime,
			EvtLogLastAccessTime,
			EvtLogLastWriteTime,
			EvtLogFileSize,
			EvtLogAttributes,
			EvtLogNumberOfLogRecords,
			EvtLogOldestRecordNumber,
			EvtLogFull
		}

		internal enum EvtPublisherMetadataPropertyId
		{
			EvtPublisherMetadataPublisherGuid,
			EvtPublisherMetadataResourceFilePath,
			EvtPublisherMetadataParameterFilePath,
			EvtPublisherMetadataMessageFilePath,
			EvtPublisherMetadataHelpLink,
			EvtPublisherMetadataPublisherMessageID,
			EvtPublisherMetadataChannelReferences,
			EvtPublisherMetadataChannelReferencePath,
			EvtPublisherMetadataChannelReferenceIndex,
			EvtPublisherMetadataChannelReferenceID,
			EvtPublisherMetadataChannelReferenceFlags,
			EvtPublisherMetadataChannelReferenceMessageID,
			EvtPublisherMetadataLevels,
			EvtPublisherMetadataLevelName,
			EvtPublisherMetadataLevelValue,
			EvtPublisherMetadataLevelMessageID,
			EvtPublisherMetadataTasks,
			EvtPublisherMetadataTaskName,
			EvtPublisherMetadataTaskEventGuid,
			EvtPublisherMetadataTaskValue,
			EvtPublisherMetadataTaskMessageID,
			EvtPublisherMetadataOpcodes,
			EvtPublisherMetadataOpcodeName,
			EvtPublisherMetadataOpcodeValue,
			EvtPublisherMetadataOpcodeMessageID,
			EvtPublisherMetadataKeywords,
			EvtPublisherMetadataKeywordName,
			EvtPublisherMetadataKeywordValue,
			EvtPublisherMetadataKeywordMessageID
		}

		internal enum EvtQueryPropertyId
		{
			EvtQueryNames,
			EvtQueryStatuses
		}

		internal enum EvtRenderContextFlags
		{
			EvtRenderContextValues,
			EvtRenderContextSystem,
			EvtRenderContextUser
		}

		internal enum EvtRenderFlags
		{
			EvtRenderEventValues,
			EvtRenderEventXml,
			EvtRenderBookmark
		}

		[Flags]
		internal enum EvtSeekFlags
		{
			EvtSeekOriginMask = 7,
			EvtSeekRelativeToBookmark = 4,
			EvtSeekRelativeToCurrent = 3,
			EvtSeekRelativeToFirst = 1,
			EvtSeekRelativeToLast = 2,
			EvtSeekStrict = 0x10000
		}

		internal enum EvtVariantType
		{
			EvtVarTypeAnsiString = 2,
			EvtVarTypeBinary = 14,
			EvtVarTypeBoolean = 13,
			EvtVarTypeByte = 4,
			EvtVarTypeDouble = 12,
			EvtVarTypeEvtHandle = 0x20,
			EvtVarTypeEvtXml = 0x23,
			EvtVarTypeFileTime = 0x11,
			EvtVarTypeGuid = 15,
			EvtVarTypeHexInt32 = 20,
			EvtVarTypeHexInt64 = 0x15,
			EvtVarTypeInt16 = 5,
			EvtVarTypeInt32 = 7,
			EvtVarTypeInt64 = 9,
			EvtVarTypeNull = 0,
			EvtVarTypeSByte = 3,
			EvtVarTypeSid = 0x13,
			EvtVarTypeSingle = 11,
			EvtVarTypeSizeT = 0x10,
			EvtVarTypeString = 1,
			EvtVarTypeStringArray = 0x81,
			EvtVarTypeSysTime = 0x12,
			EvtVarTypeUInt16 = 6,
			EvtVarTypeUInt32 = 8,
			EvtVarTypeUInt32Array = 0x88,
			EvtVarTypeUInt64 = 10
		}

		[return: MarshalAs(UnmanagedType.Bool)]
		[DllImport("wevtapi.dll", CharSet = CharSet.Auto, SetLastError = true)]
		internal static extern bool EvtArchiveExportedLog(EventLogHandle session, [MarshalAs(UnmanagedType.LPWStr)] string logFilePath, int locale, int flags);

		[return: MarshalAs(UnmanagedType.Bool)]
		[DllImport("wevtapi.dll", SetLastError = true)]
		internal static extern bool EvtCancel(EventLogHandle handle);

		[return: MarshalAs(UnmanagedType.Bool)]
		[DllImport("wevtapi.dll", CharSet = CharSet.Auto, SetLastError = true)]
		internal static extern bool EvtClearLog(EventLogHandle session, [MarshalAs(UnmanagedType.LPWStr)] string channelPath, [MarshalAs(UnmanagedType.LPWStr)] string targetFilePath, int flags);

		[return: MarshalAs(UnmanagedType.Bool)]
		[ReliabilityContract(Consistency.WillNotCorruptState, Cer.Success), DllImport("wevtapi.dll")]
		internal static extern bool EvtClose(IntPtr handle);

		[DllImport("wevtapi.dll", CharSet = CharSet.Auto, SetLastError = true)]
		internal static extern EventLogHandle EvtCreateBookmark([MarshalAs(UnmanagedType.LPWStr)] string bookmarkXml);

		[DllImport("wevtapi.dll", CharSet = CharSet.Auto, SetLastError = true)]
		internal static extern EventLogHandle EvtCreateRenderContext(int valuePathsCount, [MarshalAs(UnmanagedType.LPArray, ArraySubType = UnmanagedType.LPWStr)] string[] valuePaths, [MarshalAs(UnmanagedType.I4)] EvtRenderContextFlags flags);
		[return: MarshalAs(UnmanagedType.Bool)]
		[DllImport("wevtapi.dll", CharSet = CharSet.Auto, SetLastError = true)]
		internal static extern bool EvtExportLog(EventLogHandle session, [MarshalAs(UnmanagedType.LPWStr)] string channelPath, [MarshalAs(UnmanagedType.LPWStr)] string query, [MarshalAs(UnmanagedType.LPWStr)] string targetFilePath, int flags);

		[return: MarshalAs(UnmanagedType.Bool)]
		[DllImport("wevtapi.dll", CharSet = CharSet.Auto, SetLastError = true)]
		internal static extern bool EvtFormatMessage(EventLogHandle publisherMetadataHandle, EventLogHandle eventHandle, uint messageId, int valueCount, EvtStringVariant[] values, [MarshalAs(UnmanagedType.I4)] EvtFormatMessageFlags flags, int bufferSize, [Out, MarshalAs(UnmanagedType.LPWStr)] StringBuilder buffer, out int bufferUsed);

		[return: MarshalAs(UnmanagedType.Bool)]
		[DllImport("wevtapi.dll", EntryPoint = "EvtFormatMessage", CharSet = CharSet.Auto, SetLastError = true)]
		internal static extern bool EvtFormatMessageBuffer(EventLogHandle publisherMetadataHandle, EventLogHandle eventHandle, uint messageId, int valueCount, IntPtr values, [MarshalAs(UnmanagedType.I4)] EvtFormatMessageFlags flags, int bufferSize, IntPtr buffer, out int bufferUsed);

		[return: MarshalAs(UnmanagedType.Bool)]
		[DllImport("wevtapi.dll", CharSet = CharSet.Auto, SetLastError = true)]
		internal static extern bool EvtGetChannelConfigProperty(EventLogHandle channelConfig, [MarshalAs(UnmanagedType.I4)] EvtChannelConfigPropertyId propertyId, int flags, int propertyValueBufferSize, IntPtr propertyValueBuffer, out int propertyValueBufferUsed);

		[return: MarshalAs(UnmanagedType.Bool)]
		[DllImport("wevtapi.dll", CharSet = CharSet.Auto, SetLastError = true)]
		internal static extern bool EvtGetEventInfo(EventLogHandle eventHandle, [MarshalAs(UnmanagedType.I4)] EvtEventPropertyId propertyId, int bufferSize, IntPtr bufferPtr, out int bufferUsed);

		[return: MarshalAs(UnmanagedType.Bool)]
		[DllImport("wevtapi.dll", CharSet = CharSet.Auto, SetLastError = true)]
		internal static extern bool EvtGetEventMetadataProperty(EventLogHandle eventMetadata, [MarshalAs(UnmanagedType.I4)] EvtEventMetadataPropertyId propertyId, int flags, int eventMetadataPropertyBufferSize, IntPtr eventMetadataPropertyBuffer, out int eventMetadataPropertyBufferUsed);

		[return: MarshalAs(UnmanagedType.Bool)]
		[DllImport("wevtapi.dll", CharSet = CharSet.Auto, SetLastError = true)]
		internal static extern bool EvtGetLogInfo(EventLogHandle log, [MarshalAs(UnmanagedType.I4)] EvtLogPropertyId propertyId, int propertyValueBufferSize, IntPtr propertyValueBuffer, out int propertyValueBufferUsed);

		[return: MarshalAs(UnmanagedType.Bool)]
		[DllImport("wevtapi.dll", CharSet = CharSet.Auto, SetLastError = true)]
		internal static extern bool EvtGetObjectArrayProperty(EventLogHandle objectArray, int propertyId, int arrayIndex, int flags, int propertyValueBufferSize, IntPtr propertyValueBuffer, out int propertyValueBufferUsed);

		[return: MarshalAs(UnmanagedType.Bool)]
		[DllImport("wevtapi.dll", CharSet = CharSet.Auto, SetLastError = true)]
		internal static extern bool EvtGetObjectArraySize(EventLogHandle objectArray, out int objectArraySize);

		[return: MarshalAs(UnmanagedType.Bool)]
		[DllImport("wevtapi.dll", CharSet = CharSet.Auto, SetLastError = true)]
		internal static extern bool EvtGetPublisherMetadataProperty(EventLogHandle publisherMetadataHandle, [MarshalAs(UnmanagedType.I4)] EvtPublisherMetadataPropertyId propertyId, int flags, int publisherMetadataPropertyBufferSize, IntPtr publisherMetadataPropertyBuffer, out int publisherMetadataPropertyBufferUsed);

		[return: MarshalAs(UnmanagedType.Bool)]
		[DllImport("wevtapi.dll", CharSet = CharSet.Auto, SetLastError = true)]
		internal static extern bool EvtGetQueryInfo(EventLogHandle queryHandle, [MarshalAs(UnmanagedType.I4)] EvtQueryPropertyId propertyId, int bufferSize, IntPtr buffer, ref int bufferRequired);

		[return: MarshalAs(UnmanagedType.Bool)]
		[DllImport("wevtapi.dll", SetLastError = true)]
		internal static extern bool EvtNext(EventLogHandle queryHandle, int eventSize, [MarshalAs(UnmanagedType.LPArray)] IntPtr[] events, int timeout, int flags, ref int returned);

		[return: MarshalAs(UnmanagedType.Bool)]
		[DllImport("wevtapi.dll", CharSet = CharSet.Auto, SetLastError = true)]
		internal static extern bool EvtNextChannelPath(EventLogHandle channelEnum, int channelPathBufferSize, [Out, MarshalAs(UnmanagedType.LPWStr)] StringBuilder channelPathBuffer, out int channelPathBufferUsed);

		[DllImport("wevtapi.dll", CharSet = CharSet.Auto, SetLastError = true)]
		internal static extern EventLogHandle EvtNextEventMetadata(EventLogHandle eventMetadataEnum, int flags);

		[return: MarshalAs(UnmanagedType.Bool)]
		[DllImport("wevtapi.dll", CharSet = CharSet.Auto, SetLastError = true)]
		internal static extern bool EvtNextPublisherId(EventLogHandle publisherEnum, int publisherIdBufferSize, [Out, MarshalAs(UnmanagedType.LPWStr)] StringBuilder publisherIdBuffer, out int publisherIdBufferUsed);

		[DllImport("wevtapi.dll", CharSet = CharSet.Auto, SetLastError = true)]
		internal static extern EventLogHandle EvtOpenChannelConfig(EventLogHandle session, [MarshalAs(UnmanagedType.LPWStr)] string channelPath, int flags);

		[DllImport("wevtapi.dll", CharSet = CharSet.Auto, SetLastError = true)]
		internal static extern EventLogHandle EvtOpenChannelEnum(EventLogHandle session, int flags);

		[DllImport("wevtapi.dll", CharSet = CharSet.Auto, SetLastError = true)]
		internal static extern EventLogHandle EvtOpenEventMetadataEnum(EventLogHandle publisherMetadata, int flags);

		[DllImport("wevtapi.dll", CharSet = CharSet.Auto, SetLastError = true)]
		internal static extern EventLogHandle EvtOpenLog(EventLogHandle session, [MarshalAs(UnmanagedType.LPWStr)] string path, [MarshalAs(UnmanagedType.I4)] PathType flags);

		[DllImport("wevtapi.dll", CharSet = CharSet.Auto, SetLastError = true)]
		internal static extern EventLogHandle EvtOpenPublisherEnum(EventLogHandle session, int flags);

		[DllImport("wevtapi.dll", CharSet = CharSet.Auto, SetLastError = true)]
		internal static extern EventLogHandle EvtOpenPublisherMetadata(EventLogHandle session, [MarshalAs(UnmanagedType.LPWStr)] string publisherId, [MarshalAs(UnmanagedType.LPWStr)] string logFilePath, int locale, int flags);

		[DllImport("wevtapi.dll", CharSet = CharSet.Auto, SetLastError = true)]
		internal static extern EventLogHandle EvtOpenSession([MarshalAs(UnmanagedType.I4)] EvtLoginClass loginClass, ref EvtRpcLogin login, int timeout, int flags);

		[DllImport("wevtapi.dll", SetLastError = true)]
		internal static extern EventLogHandle EvtQuery(EventLogHandle session, [MarshalAs(UnmanagedType.LPWStr)] string path, [MarshalAs(UnmanagedType.LPWStr)] string query, int flags);

		[return: MarshalAs(UnmanagedType.Bool)]
		[DllImport("wevtapi.dll", SetLastError = true)]
		internal static extern bool EvtRender(EventLogHandle context, EventLogHandle eventHandle, EvtRenderFlags flags, int buffSize, IntPtr buffer, out int buffUsed, out int propCount);

		[return: MarshalAs(UnmanagedType.Bool)]
		[DllImport("wevtapi.dll", SetLastError = true)]
		internal static extern bool EvtRender(EventLogHandle context, EventLogHandle eventHandle, EvtRenderFlags flags, int buffSize, [Out, MarshalAs(UnmanagedType.LPWStr)] StringBuilder buffer, out int buffUsed, out int propCount);

		[return: MarshalAs(UnmanagedType.Bool)]
		[DllImport("wevtapi.dll", CharSet = CharSet.Auto, SetLastError = true)]
		internal static extern bool EvtSaveChannelConfig(EventLogHandle channelConfig, int flags);

		[return: MarshalAs(UnmanagedType.Bool)]
		[DllImport("wevtapi.dll", CharSet = CharSet.Auto, SetLastError = true)]
		internal static extern bool EvtSeek(EventLogHandle resultSet, long position, EventLogHandle bookmark, int timeout, [MarshalAs(UnmanagedType.I4)] EvtSeekFlags flags);

		[return: MarshalAs(UnmanagedType.Bool)]
		[DllImport("wevtapi.dll", CharSet = CharSet.Auto, SetLastError = true)]
		internal static extern bool EvtSetChannelConfigProperty(EventLogHandle channelConfig, [MarshalAs(UnmanagedType.I4)] EvtChannelConfigPropertyId propertyId, int flags, ref EvtVariant propertyValue);

		[DllImport("wevtapi.dll", SetLastError = true)]
		internal static extern EventLogHandle EvtSubscribe(EventLogHandle session, SafeWaitHandle signalEvent, [MarshalAs(UnmanagedType.LPWStr)] string path, [MarshalAs(UnmanagedType.LPWStr)] string query, EventLogHandle bookmark, IntPtr context, IntPtr callback, int flags);

		[return: MarshalAs(UnmanagedType.Bool)]
		[DllImport("wevtapi.dll", CharSet = CharSet.Auto, SetLastError = true)]
		internal static extern bool EvtUpdateBookmark(EventLogHandle bookmark, EventLogHandle eventHandle);
		[StructLayout(LayoutKind.Sequential, CharSet = CharSet.Auto)]
		internal struct EvtRpcLogin
		{
			[MarshalAs(UnmanagedType.LPWStr)]
			public string Server;

			[MarshalAs(UnmanagedType.LPWStr)]
			public string User;

			[MarshalAs(UnmanagedType.LPWStr)]
			public string Domain;

			public CoTaskMemUnicodeSafeHandle Password;
			public int Flags;
		}

		[StructLayout(LayoutKind.Explicit, CharSet = CharSet.Auto)]
		internal struct EvtStringVariant
		{
			// Fields
			[FieldOffset(8)]
			public uint Count;

			[MarshalAs(UnmanagedType.LPWStr), FieldOffset(0)]
			public string StringVal;

			[FieldOffset(12)]
			public uint Type;
		}
		[StructLayout(LayoutKind.Explicit, CharSet = CharSet.Auto)]
		internal struct EvtVariant
		{
			// Fields
			[FieldOffset(0)]
			public IntPtr AnsiString;

			[FieldOffset(0)]
			public IntPtr Binary;

			[FieldOffset(0)]
			public uint Bool;

			[FieldOffset(0)]
			public byte ByteVal;

			[FieldOffset(8)]
			public uint Count;

			[FieldOffset(0)]
			public double Double;

			[FieldOffset(0)]
			public ulong FileTime;

			[FieldOffset(0)]
			public IntPtr GuidReference;

			[FieldOffset(0)]
			public IntPtr Handle;

			[FieldOffset(0)]
			public int Integer;

			[FieldOffset(0)]
			public long Long;

			[FieldOffset(0)]
			public IntPtr Reference;

			[FieldOffset(0)]
			public byte SByte;

			[FieldOffset(0)]
			public short Short;

			[FieldOffset(0)]
			public IntPtr SidVal;

			[FieldOffset(0)]
			public IntPtr StringVal;

			[FieldOffset(0)]
			public IntPtr SystemTime;

			[FieldOffset(12)]
			public uint Type;

			[FieldOffset(0)]
			public byte UInt8;

			[FieldOffset(0)]
			public uint UInteger;

			[FieldOffset(0)]
			public ulong ULong;

			[FieldOffset(0)]
			public ushort UShort;
		}
	}
}
#endif