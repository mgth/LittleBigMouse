using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Text;
using System.Xml.Serialization;

namespace Microsoft.Win32.TaskScheduler
{
	#region Enumerations

	/// <summary>Values for days of the week (Monday, Tuesday, etc.)</summary>
	[Flags]
	public enum DaysOfTheWeek : short
	{
		/// <summary>Sunday</summary>
		Sunday = 0x1,
		/// <summary>Monday</summary>
		Monday = 0x2,
		/// <summary>Tuesday</summary>
		Tuesday = 0x4,
		/// <summary>Wednesday</summary>
		Wednesday = 0x8,
		/// <summary>Thursday</summary>
		Thursday = 0x10,
		/// <summary>Friday</summary>
		Friday = 0x20,
		/// <summary>Saturday</summary>
		Saturday = 0x40,
		/// <summary>All days</summary>
		AllDays = 0x7F
	}

	/// <summary>Values for months of the year (January, February, etc.)</summary>
	[Flags]
	public enum MonthsOfTheYear : short
	{
		/// <summary>January</summary>
		January = 0x1,
		/// <summary>February</summary>
		February = 0x2,
		/// <summary>March</summary>
		March = 0x4,
		/// <summary>April</summary>
		April = 0x8,
		/// <summary>May</summary>
		May = 0x10,
		/// <summary>June</summary>
		June = 0x20,
		/// <summary>July</summary>
		July = 0x40,
		/// <summary>August</summary>
		August = 0x80,
		/// <summary>September</summary>
		September = 0x100,
		/// <summary>October</summary>
		October = 0x200,
		/// <summary>November</summary>
		November = 0x400,
		/// <summary>December</summary>
		December = 0x800,
		/// <summary>All months</summary>
		AllMonths = 0xFFF
	}

	/// <summary>Defines the type of triggers that can be used by tasks.</summary>
	public enum TaskTriggerType
	{
		/// <summary>Triggers the task when a specific event occurs. Version 1.2 only.</summary>
		Event = 0,
		/// <summary>Triggers the task at a specific time of day.</summary>
		Time = 1,
		/// <summary>Triggers the task on a daily schedule.</summary>
		Daily = 2,
		/// <summary>Triggers the task on a weekly schedule.</summary>
		Weekly = 3,
		/// <summary>Triggers the task on a monthly schedule.</summary>
		Monthly = 4,
		/// <summary>Triggers the task on a monthly day-of-week schedule.</summary>
		MonthlyDOW = 5,
		/// <summary>Triggers the task when the computer goes into an idle state.</summary>
		Idle = 6,
		/// <summary>Triggers the task when the task is registered. Version 1.2 only.</summary>
		Registration = 7,
		/// <summary>Triggers the task when the computer boots.</summary>
		Boot = 8,
		/// <summary>Triggers the task when a specific user logs on.</summary>
		Logon = 9,
		/// <summary>Triggers the task when a specific user session state changes. Version 1.2 only.</summary>
		SessionStateChange = 11,
		/// <summary>Triggers the custom trigger. Version 1.3 only.</summary>
		Custom = 12
	}

	/// <summary>Values for week of month (first, second, ..., last)</summary>
	[Flags]
	public enum WhichWeek : short
	{
		/// <summary>First week of the month</summary>
		FirstWeek = 1,
		/// <summary>Second week of the month</summary>
		SecondWeek = 2,
		/// <summary>Third week of the month</summary>
		ThirdWeek = 4,
		/// <summary>Fourth week of the month</summary>
		FourthWeek = 8,
		/// <summary>Last week of the month</summary>
		LastWeek = 0x10,
		/// <summary>Every week of the month</summary>
		AllWeeks = 0x1F
	}

	#endregion Enumerations

	#region Interfaces

	/// <summary>
	/// Interface that categorizes the trigger as a calendar trigger.
	/// </summary>
	public interface ICalendarTrigger { }

	/// <summary>
	/// Interface for triggers that support a delay.
	/// </summary>
	public interface ITriggerDelay
	{
		/// <summary>
		/// Gets or sets a value that indicates the amount of time before the task is started.
		/// </summary>
		/// <value>The delay duration.</value>
		TimeSpan Delay { get; set; }
	}

	/// <summary>
	/// Interface for triggers that support a user identifier.
	/// </summary>
	public interface ITriggerUserId
	{
		/// <summary>
		/// Gets or sets the user for the <see cref="Trigger"/>.
		/// </summary>
		string UserId { get; set; }
	}

	#endregion Interfaces

	/// <summary>
	/// Abstract base class which provides the common properties that are inherited by all trigger classes. A trigger can be created using the <see cref="TriggerCollection.Add"/> or the <see cref="TriggerCollection.AddNew"/> method.
	/// </summary>
	public abstract partial class Trigger : IDisposable, ICloneable, IEquatable<Trigger>
	{
		internal const string V2BoundaryDateFormat = "yyyy'-'MM'-'dd'T'HH':'mm':'ss'.'FFFK";
		internal static readonly System.Globalization.CultureInfo DefaultDateCulture = System.Globalization.CultureInfo.CreateSpecificCulture("en-US");

		internal TaskTriggerType ttype;
		internal V1Interop.ITaskTrigger v1Trigger = null;
		internal V1Interop.TaskTrigger v1TriggerData;
		internal V2Interop.ITrigger v2Trigger = null;

		/// <summary>In testing and may change. Do not use until officially introduced into library.</summary>
		protected Dictionary<string, object> unboundValues;

		private RepetitionPattern repititionPattern = null;

		internal Trigger(V1Interop.ITaskTrigger trigger, V1Interop.TaskTriggerType type)
		{
			v1Trigger = trigger;
			v1TriggerData = trigger.GetTrigger();
			v1TriggerData.Type = type;
			ttype = ConvertFromV1TriggerType(type);
		}

		internal Trigger(V1Interop.ITaskTrigger trigger, V1Interop.TaskTrigger data)
		{
			v1Trigger = trigger;
			v1TriggerData = data;
			ttype = ConvertFromV1TriggerType(data.Type);
		}

		internal Trigger(V2Interop.ITrigger iTrigger)
		{
			v2Trigger = iTrigger;
			this.ttype = iTrigger.Type;
			if (string.IsNullOrEmpty(v2Trigger.StartBoundary))
				this.StartBoundary = DateTime.SpecifyKind(DateTime.Now, DateTimeKind.Unspecified);
		}

		internal Trigger(TaskTriggerType triggerType)
		{
			this.ttype = triggerType;
			unboundValues = new Dictionary<string, object>();

			v1TriggerData.TriggerSize = (ushort)Marshal.SizeOf(typeof(V1Interop.TaskTrigger));
			try { v1TriggerData.Type = ConvertToV1TriggerType(this.ttype); }
			catch { }

			this.StartBoundary = DateTime.SpecifyKind(DateTime.Now, DateTimeKind.Unspecified);
		}

		/// <summary>
		/// Gets or sets a Boolean value that indicates whether the trigger is enabled.
		/// </summary>
		public bool Enabled
		{
			get
			{
				if (v2Trigger != null)
					return v2Trigger.Enabled;
				return !((v1TriggerData.Flags & V1Interop.TaskTriggerFlags.Disabled) == V1Interop.TaskTriggerFlags.Disabled);
			}
			set
			{
				if (v2Trigger != null)
					v2Trigger.Enabled = value;
				else
				{
					if (!value)
						v1TriggerData.Flags |= V1Interop.TaskTriggerFlags.Disabled;
					else
						v1TriggerData.Flags &= ~V1Interop.TaskTriggerFlags.Disabled;
					if (v1Trigger != null)
						SetV1TriggerData();
					else
						unboundValues["Enabled"] = value;
				}
			}
		}

		/// <summary>
		/// Gets or sets the date and time when the trigger is deactivated. The trigger cannot start the task after it is deactivated.
		/// </summary>
		/// <remarks>
		/// <para>Version 1 (1.1 on all systems prior to Vista) of the native library only allows for the Day, Month and Year values of the <see cref="DateTime"/> structure.</para>
		/// <para>Version 2 (1.2 or higher) of the native library only allows for both date and time and all <see cref="DateTime.Kind"/> values. However, the user interface and <see cref="Trigger.ToString()"/> methods
		/// will always show the time translated to local time. The library makes every attempt to maintain the Kind value. When using the UI elements provided in the TaskSchedulerEditor
		/// library, the "Synchronize across time zones" checkbox will be checked if the Kind is Local or Utc. If the Kind is Unspecified and the user selects the checkbox, the Kind will
		/// be changed to Utc and the time adjusted from the value displayed as the local time.
		/// </para>
		/// </remarks>
		[DefaultValue(typeof(DateTime), "9999-12-31T23:59:59.9999999")]
		public DateTime EndBoundary
		{
			get
			{
				if (v2Trigger != null)
					return string.IsNullOrEmpty(v2Trigger.EndBoundary) ? DateTime.MaxValue : DateTime.Parse(v2Trigger.EndBoundary, DefaultDateCulture);
				return (unboundValues!=null && unboundValues.ContainsKey("EndBoundary")) ? (DateTime)unboundValues["EndBoundary"] : v1TriggerData.EndDate;
			}
			set
			{
				if (v2Trigger != null)
					v2Trigger.EndBoundary = value == DateTime.MaxValue ? null : value.ToString(V2BoundaryDateFormat, DefaultDateCulture);
				else
				{
					v1TriggerData.EndDate = value;
					if (v1Trigger != null)
						SetV1TriggerData();
					else
						unboundValues["EndBoundary"] = value;
				}
			}
		}

		/// <summary>
		/// Gets or sets the maximum amount of time that the task launched by this trigger is allowed to run. Not available with Task Scheduler 1.0.
		/// </summary>
		/// <exception cref="NotV1SupportedException">Not supported under Task Scheduler 1.0.</exception>
		[DefaultValue(typeof(TimeSpan), "00:00:00")]
		[XmlIgnore]
		public TimeSpan ExecutionTimeLimit
		{
			get
			{
				if (v2Trigger != null)
					return Task.StringToTimeSpan(v2Trigger.ExecutionTimeLimit);
				if (v1Trigger != null)
					throw new NotV1SupportedException();
				return ((unboundValues!=null && unboundValues.ContainsKey("ExecutionTimeLimit")) ? (TimeSpan)unboundValues["ExecutionTimeLimit"] : TimeSpan.Zero);
			}
			set
			{
				if (v2Trigger != null)
					v2Trigger.ExecutionTimeLimit = Task.TimeSpanToString(value);
				else if (v1Trigger != null)
					throw new NotV1SupportedException();
				else
					unboundValues["ExecutionTimeLimit"] = value;
			}
		}

		/// <summary>
		/// Gets or sets the identifier for the trigger. Cannot set with Task Scheduler 1.0.
		/// </summary>
		/// <exception cref="NotV1SupportedException">Not supported under Task Scheduler 1.0.</exception>
		[DefaultValue(null)]
		[XmlIgnore]
		public string Id
		{
			get
			{
				if (v2Trigger != null)
					return v2Trigger.Id;
				if (v1Trigger != null)
					throw new NotV1SupportedException();
				return (unboundValues.ContainsKey("Id") ? (string)unboundValues["Id"] : null);
			}
			set
			{
				if (v2Trigger != null)
					v2Trigger.Id = value;
				else if (v1Trigger != null)
					throw new NotV1SupportedException();
				else
					unboundValues["Id"] = value;
			}
		}

		/// <summary>
		/// Gets a <see cref="RepetitionPattern"/> instance that indicates how often the task is run and how long the repetition pattern is repeated after the task is started.
		/// </summary>
		public RepetitionPattern Repetition
		{
			get
			{
				if (repititionPattern == null)
					repititionPattern = new RepetitionPattern(this);
				return repititionPattern;
			}
		}

		/// <summary>
		/// Gets or sets the date and time when the trigger is activated.
		/// </summary>
		/// <remarks>
		/// <para>Version 1 (1.1 on all systems prior to Vista) of the native library only allows for <see cref="DateTime"/> values where the <see cref="DateTime.Kind"/> is unspecified.
		/// If the DateTime value Kind is <see cref="DateTimeKind.Local"/> then it will be used as is. If the DateTime value Kind is <see cref="DateTimeKind.Utc"/> then it will be 
		/// converted to the local time and then used.
		/// </para>
		/// <para>Version 2 (1.2 or higher) of the native library only allows for all <see cref="DateTime.Kind"/> values. However, the user interface and <see cref="Trigger.ToString()"/> methods
		/// will always show the time translated to local time. The library makes every attempt to maintain the Kind value. When using the UI elements provided in the TaskSchedulerEditor
		/// library, the "Synchronize across time zones" checkbox will be checked if the Kind is Local or Utc. If the Kind is Unspecified and the user selects the checkbox, the Kind will
		/// be changed to Utc and the time adjusted from the value displayed as the local time.
		/// </para>
		/// <para>Under Version 2, when converting the string used in the native library for this value (ITrigger.Startboundary) this library will behave as follows:
		/// <list type="bullet">
		///	<item><description>YYYY-MM-DDTHH:MM:SS format uses DateTimeKind.Unspecified and the time specified.</description></item>
		///	<item><description>YYYY-MM-DDTHH:MM:SSZ format uses DateTimeKind.Utc and the time specified as the GMT time.</description></item>
		///	<item><description>YYYY-MM-DDTHH:MM:SS±HH:MM format uses DateTimeKind.Local and the time specified in that time zone.</description></item>
		/// </list>
		/// </para>
		/// </remarks>
		public DateTime StartBoundary
		{
			get
			{
				if (v2Trigger != null)
				{
					if (string.IsNullOrEmpty(v2Trigger.StartBoundary))
					{
						return DateTime.MinValue;
					}
					else
					{
						DateTime ret = DateTime.Parse(v2Trigger.StartBoundary, DefaultDateCulture);
						if (v2Trigger.StartBoundary.EndsWith("Z"))
							ret = ret.ToUniversalTime();
						return ret;
					}
				}
				return (unboundValues!=null && unboundValues.ContainsKey("StartBoundary")) ? (DateTime)unboundValues["StartBoundary"] : v1TriggerData.BeginDate;
			}
			set
			{
				if (v2Trigger != null)
					v2Trigger.StartBoundary = value == DateTime.MinValue ? null : value.ToString(V2BoundaryDateFormat, DefaultDateCulture);
				else
				{
					v1TriggerData.BeginDate = value;
					if (v1Trigger != null)
						SetV1TriggerData();
					else
						unboundValues["StartBoundary"] = value;
				}
			}
		}

		/// <summary>
		/// Gets the type of the trigger.
		/// </summary>
		/// <value>The <see cref="TaskTriggerType"/> of the trigger.</value>
		[XmlIgnore]
		public TaskTriggerType TriggerType
		{
			get { return ttype; }
		}

		/// <summary>In testing and may change. Do not use until officially introduced into library.</summary>
		internal virtual bool Bound
		{
			get
			{
				if (v1Trigger != null)
					return v1Trigger.GetTrigger().Equals(v1TriggerData);
				return (v2Trigger != null);
			}
		}

		/// <summary>
		/// Creates a new <see cref="Trigger"/> that is an unbound copy of this instance.
		/// </summary>
		/// <returns>
		/// A new <see cref="Trigger"/> that is an unbound copy of this instance.
		/// </returns>
		public virtual object Clone()
		{
			Trigger ret = CreateTrigger(this.TriggerType);
			ret.CopyProperties(this);
			return ret;
		}

		/// <summary>
		/// Copies the properties from another <see cref="Trigger"/> the current instance. This will not copy any properties associated with any derived triggers except those supporting the <see cref="ITriggerDelay"/> interface.
		/// </summary>
		/// <param name="sourceTrigger">The source <see cref="Trigger"/>.</param>
		public virtual void CopyProperties(Trigger sourceTrigger)
		{
			this.Enabled = sourceTrigger.Enabled;
			this.EndBoundary = sourceTrigger.EndBoundary;
			try { this.ExecutionTimeLimit = sourceTrigger.ExecutionTimeLimit; }
			catch { }
			this.Repetition.Duration = sourceTrigger.Repetition.Duration;
			this.Repetition.Interval = sourceTrigger.Repetition.Interval;
			this.Repetition.StopAtDurationEnd = sourceTrigger.Repetition.StopAtDurationEnd;
			this.StartBoundary = sourceTrigger.StartBoundary;
			if (sourceTrigger is ITriggerDelay && this is ITriggerDelay)
				try { ((ITriggerDelay)this).Delay = ((ITriggerDelay)sourceTrigger).Delay; }
				catch { }
			if (sourceTrigger is ITriggerUserId && this is ITriggerUserId)
				try { ((ITriggerUserId)this).UserId = ((ITriggerUserId)sourceTrigger).UserId; }
				catch { }
		}

		/// <summary>
		/// Releases all resources used by this class.
		/// </summary>
		public virtual void Dispose()
		{
			if (v2Trigger != null)
				Marshal.ReleaseComObject(v2Trigger);
			if (v1Trigger != null)
				Marshal.ReleaseComObject(v1Trigger);
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
			if (obj is Trigger)
				return this.Equals((Trigger)obj);
			return base.Equals(obj);
		}

		/// <summary>
		/// Indicates whether the current object is equal to another object of the same type.
		/// </summary>
		/// <param name="other">An object to compare with this object.</param>
		/// <returns>
		/// true if the current object is equal to the <paramref name="other" /> parameter; otherwise, false.
		/// </returns>
		public virtual bool Equals(Trigger other)
		{
			bool ret = this.TriggerType == other.TriggerType && this.Enabled == other.Enabled && this.EndBoundary == other.EndBoundary &&
				this.ExecutionTimeLimit == other.ExecutionTimeLimit && this.Id == other.Id && this.Repetition.Equals(other.Repetition) &&
				this.StartBoundary == other.StartBoundary;
			if (other is ITriggerDelay && this is ITriggerDelay)
				try { ret = ret && ((ITriggerDelay)this).Delay == ((ITriggerDelay)other).Delay; }
				catch { }
			if (other is ITriggerUserId && this is ITriggerUserId)
				try { ret = ret && ((ITriggerUserId)this).UserId == ((ITriggerUserId)other).UserId; }
				catch { }
			return ret;
		}

		/// <summary>
		/// Returns a hash code for this instance.
		/// </summary>
		/// <returns>
		/// A hash code for this instance, suitable for use in hashing algorithms and data structures like a hash table. 
		/// </returns>
		public override int GetHashCode()
		{
			return new { A = this.TriggerType, B = this.Enabled, C = this.EndBoundary, D = this.ExecutionTimeLimit, E = this.Id,
						 F = this.Repetition, G = this.StartBoundary,
						 H = this is ITriggerDelay ? ((ITriggerDelay)this).Delay : TimeSpan.Zero,
						 I = this is ITriggerUserId ? ((ITriggerUserId)this).UserId : null }.GetHashCode();
		}

		/// <summary>
		/// Sets the repetition.
		/// </summary>
		/// <param name="interval">The amount of time between each restart of the task. The maximum time allowed is 31 days, and the minimum time allowed is 1 minute.</param>
		/// <param name="duration">The duration of how long the pattern is repeated. The minimum time allowed is one minute. If <c>TimeSpan.Zero</c> is specified, the pattern is repeated indefinitely.</param>
		/// <param name="stopAtDurationEnd">if set to <c>true</c> the running instance of the task is stopped at the end of repetition pattern duration.</param>
		public void SetRepetition(TimeSpan interval, TimeSpan duration, bool stopAtDurationEnd = true)
		{
			this.Repetition.Duration = duration;
			this.Repetition.Interval = interval;
			this.Repetition.StopAtDurationEnd = stopAtDurationEnd;
		}

		/// <summary>
		/// Returns a string representing this trigger.
		/// </summary>
		/// <returns>String value of trigger.</returns>
		public override string ToString()
		{
			if (v1Trigger != null)
				return v1Trigger.GetTriggerString();
			return V2GetTriggerString() + V2BaseTriggerString();
		}

		/// <summary>
		/// Returns a <see cref="System.String"/> that represents this trigger in a specific language.
		/// </summary>
		/// <param name="culture">The language of the resulting string.</param>
		/// <returns>String value of trigger.</returns>
		public virtual string ToString(System.Globalization.CultureInfo culture)
		{
			using (new CultureSwitcher(culture))
				return this.ToString();
		}

		internal static TaskTriggerType ConvertFromV1TriggerType(V1Interop.TaskTriggerType v1Type)
		{
			int v2tt = (int)v1Type + 1;
			if (v2tt > 6) v2tt++;
			return (TaskTriggerType)v2tt;
		}

		internal static V1Interop.TaskTriggerType ConvertToV1TriggerType(TaskTriggerType type)
		{
			if (type == TaskTriggerType.Registration || type == TaskTriggerType.Event || type == TaskTriggerType.SessionStateChange)
				throw new NotV1SupportedException();
			int v1tt = (int)type - 1;
			if (v1tt >= 7) v1tt--;
			return (V1Interop.TaskTriggerType)v1tt;
		}

		internal static Trigger CreateTrigger(V1Interop.ITaskTrigger trigger)
		{
			return CreateTrigger(trigger, trigger.GetTrigger().Type);
		}

		internal static Trigger CreateTrigger(V1Interop.ITaskTrigger trigger, V1Interop.TaskTriggerType triggerType)
		{
			Trigger t = null;
			switch (triggerType)
			{
				case Microsoft.Win32.TaskScheduler.V1Interop.TaskTriggerType.RunOnce:
					t = new TimeTrigger(trigger);
					break;
				case Microsoft.Win32.TaskScheduler.V1Interop.TaskTriggerType.RunDaily:
					t = new DailyTrigger(trigger);
					break;
				case Microsoft.Win32.TaskScheduler.V1Interop.TaskTriggerType.RunWeekly:
					t = new WeeklyTrigger(trigger);
					break;
				case Microsoft.Win32.TaskScheduler.V1Interop.TaskTriggerType.RunMonthly:
					t = new MonthlyTrigger(trigger);
					break;
				case Microsoft.Win32.TaskScheduler.V1Interop.TaskTriggerType.RunMonthlyDOW:
					t = new MonthlyDOWTrigger(trigger);
					break;
				case Microsoft.Win32.TaskScheduler.V1Interop.TaskTriggerType.OnIdle:
					t = new IdleTrigger(trigger);
					break;
				case Microsoft.Win32.TaskScheduler.V1Interop.TaskTriggerType.OnSystemStart:
					t = new BootTrigger(trigger);
					break;
				case Microsoft.Win32.TaskScheduler.V1Interop.TaskTriggerType.OnLogon:
					t = new LogonTrigger(trigger);
					break;
				default:
					break;
			}
			return t;
		}

		internal static Trigger CreateTrigger(V2Interop.ITrigger iTrigger, V2Interop.ITaskDefinition iDef = null)
		{
			switch (iTrigger.Type)
			{
				case TaskTriggerType.Boot:
					return new BootTrigger((V2Interop.IBootTrigger)iTrigger);
				case TaskTriggerType.Daily:
					return new DailyTrigger((V2Interop.IDailyTrigger)iTrigger);
				case TaskTriggerType.Event:
					return new EventTrigger((V2Interop.IEventTrigger)iTrigger);
				case TaskTriggerType.Idle:
					return new IdleTrigger((V2Interop.IIdleTrigger)iTrigger);
				case TaskTriggerType.Logon:
					return new LogonTrigger((V2Interop.ILogonTrigger)iTrigger);
				case TaskTriggerType.Monthly:
					return new MonthlyTrigger((V2Interop.IMonthlyTrigger)iTrigger);
				case TaskTriggerType.MonthlyDOW:
					return new MonthlyDOWTrigger((V2Interop.IMonthlyDOWTrigger)iTrigger);
				case TaskTriggerType.Registration:
					return new RegistrationTrigger((V2Interop.IRegistrationTrigger)iTrigger);
				case TaskTriggerType.SessionStateChange:
					return new SessionStateChangeTrigger((V2Interop.ISessionStateChangeTrigger)iTrigger);
				case TaskTriggerType.Time:
					return new TimeTrigger((V2Interop.ITimeTrigger)iTrigger);
				case TaskTriggerType.Weekly:
					return new WeeklyTrigger((V2Interop.IWeeklyTrigger)iTrigger);
				case TaskTriggerType.Custom:
					CustomTrigger ct = new CustomTrigger(iTrigger);
					if (iDef != null)
						ct.UpdateFromXml(iDef.XmlText);
					return ct;
				default:
					break;
			}
			return null;
		}

		/// <summary>
		/// Creates the specified trigger.
		/// </summary>
		/// <param name="triggerType">Type of the trigger to instantiate.</param>
		/// <returns><see cref="Trigger"/> of specified type.</returns>
		public static Trigger CreateTrigger(TaskTriggerType triggerType)
		{
			switch (triggerType)
			{
				case TaskTriggerType.Boot:
					return new BootTrigger();
				case TaskTriggerType.Daily:
					return new DailyTrigger();
				case TaskTriggerType.Event:
					return new EventTrigger();
				case TaskTriggerType.Idle:
					return new IdleTrigger();
				case TaskTriggerType.Logon:
					return new LogonTrigger();
				case TaskTriggerType.Monthly:
					return new MonthlyTrigger();
				case TaskTriggerType.MonthlyDOW:
					return new MonthlyDOWTrigger();
				case TaskTriggerType.Registration:
					return new RegistrationTrigger();
				case TaskTriggerType.SessionStateChange:
					return new SessionStateChangeTrigger();
				case TaskTriggerType.Time:
					return new TimeTrigger();
				case TaskTriggerType.Weekly:
					return new WeeklyTrigger();
				default:
					break;
			}
			return null;
		}

		internal virtual void Bind(V1Interop.ITask iTask)
		{
			if (v1Trigger == null)
			{
				ushort idx;
				v1Trigger = iTask.CreateTrigger(out idx);
			}
			SetV1TriggerData();
		}

		internal virtual void Bind(V2Interop.ITaskDefinition iTaskDef)
		{
			V2Interop.ITriggerCollection iTriggers = iTaskDef.Triggers;
			v2Trigger = iTriggers.Create(ttype);
			Marshal.ReleaseComObject(iTriggers);
			foreach (string key in unboundValues.Keys)
			{
				try
				{
					object o = unboundValues[key];
					CheckBindValue(key, ref o);
					v2Trigger.GetType().InvokeMember(key, System.Reflection.BindingFlags.SetProperty, null, v2Trigger, new object[] { o });
				}
				catch (System.Reflection.TargetInvocationException tie) { throw tie.InnerException; }
				catch { }
			}
			unboundValues.Clear();
			unboundValues = null;

			this.repititionPattern = new RepetitionPattern(this);
			this.repititionPattern.Bind();
		}

		/// <summary>
		/// Checks the bind value for any conversion.
		/// </summary>
		/// <param name="key">The key (property) name.</param>
		/// <param name="o">The value.</param>
		internal virtual void CheckBindValue(string key, ref object o)
		{
			if (o is TimeSpan)
				o = Task.TimeSpanToString((TimeSpan)o);
			if (o is DateTime)
			{
				if ((key == "EndBoundary" && ((DateTime)o) == DateTime.MaxValue) || (key == "StartBoundary" && ((DateTime)o) == DateTime.MinValue))
					o = null;
				else
					o = ((DateTime)o).ToString(V2BoundaryDateFormat, DefaultDateCulture);
			}
		}

		/// <summary>Assigns the unbound TriggerData structure to the V1 trigger instance.</summary>
		internal void SetV1TriggerData()
		{
			if (v1TriggerData.MinutesInterval != 0 && v1TriggerData.MinutesInterval >= v1TriggerData.MinutesDuration)
				throw new ArgumentException("Trigger.Repetition.Interval must be less than Trigger.Repetition.Duration under Task Scheduler 1.0.");
			if (v1Trigger != null)
				v1Trigger.SetTrigger(ref v1TriggerData);
			System.Diagnostics.Debug.WriteLine(v1TriggerData);
		}

		/// <summary>
		/// Gets the non-localized trigger string for V2 triggers.
		/// </summary>
		/// <returns>String describing the trigger.</returns>
		protected virtual string V2GetTriggerString()
		{
			return string.Empty;
		}

		internal static DateTime AdjustToLocal(DateTime dt)
		{
			return dt.Kind == DateTimeKind.Utc ? dt.ToLocalTime() : dt;
		}

		private string V2BaseTriggerString()
		{
			StringBuilder ret = new StringBuilder();
			if (this.Repetition.Interval != TimeSpan.Zero)
			{
				string sduration = string.Empty;
				if (this.Repetition.Duration == TimeSpan.Zero)
					sduration = Properties.Resources.TriggerDuration0;
				else
					sduration = string.Format(Properties.Resources.TriggerDurationNot0, GetBestTimeSpanString(this.Repetition.Duration));
				ret.AppendFormat(Properties.Resources.TriggerRepetition, GetBestTimeSpanString(this.Repetition.Interval), sduration);
			}
			if (this.EndBoundary != DateTime.MaxValue)
				ret.AppendFormat(Properties.Resources.TriggerEndBoundary, AdjustToLocal(this.EndBoundary));
			if (ret.Length > 0)
				ret.Insert(0, Properties.Resources.HyphenSeparator);
			return ret.ToString();
		}

		private static Type timeSpan2Type = null;
		private static bool? foundTimeSpan2 = null;

		/// <summary>
		/// Gets the best time span string.
		/// </summary>
		/// <param name="span">The <see cref="TimeSpan"/> to display.</param>
		/// <returns>Either the full string representation created by TimeSpan2 or the default TimeSpan representation.</returns>
		private static string GetBestTimeSpanString(TimeSpan span)
		{
			// See if the TimeSpan2 assembly is accessible
			if (!foundTimeSpan2.HasValue)
			{
				try
				{
					foundTimeSpan2 = false;
					timeSpan2Type = System.Reflection.ReflectionHelper.LoadType("System.TimeSpan2", "TimeSpan2.dll");
					if (timeSpan2Type != null)
						foundTimeSpan2 = true;
				}
				catch { }
			}

			// If the TimeSpan2 assembly is available, try to call the ToString("f") method and return the result.
			if (foundTimeSpan2 == true && timeSpan2Type != null)
			{
				try
				{
					return System.Reflection.ReflectionHelper.InvokeMethod<string>(timeSpan2Type, new object[] { span }, "ToString", "f");
				}
				catch { }
			}

			return span.ToString();
		}
	}

	/// <summary>
	/// Represents a trigger that starts a task when the system is booted.
	/// </summary>
	public sealed class BootTrigger : Trigger, ITriggerDelay
	{
		/// <summary>
		/// Creates an unbound instance of a <see cref="BootTrigger"/>.
		/// </summary>
		public BootTrigger()
			: base(TaskTriggerType.Boot)
		{
		}

		internal BootTrigger(V1Interop.ITaskTrigger iTrigger)
			: base(iTrigger, V1Interop.TaskTriggerType.OnSystemStart)
		{
		}

		internal BootTrigger(V2Interop.ITrigger iTrigger)
			: base(iTrigger)
		{
		}

		/// <summary>
		/// Gets or sets a value that indicates the amount of time between when the system is booted and when the task is started.
		/// </summary>
		/// <exception cref="NotV1SupportedException">Not supported under Task Scheduler 1.0.</exception>
		[DefaultValue(typeof(TimeSpan), "00:00:00")]
		[XmlIgnore]
		public TimeSpan Delay
		{
			get
			{
				if (v2Trigger != null)
					return Task.StringToTimeSpan(((V2Interop.IBootTrigger)v2Trigger).Delay);
				if (v1Trigger != null)
					throw new NotV1SupportedException();
				return (unboundValues.ContainsKey("Delay") ? (TimeSpan)unboundValues["Delay"] : TimeSpan.Zero);
			}
			set
			{
				if (v2Trigger != null)
					((V2Interop.IBootTrigger)v2Trigger).Delay = Task.TimeSpanToString(value);
				else if (v1Trigger != null)
					throw new NotV1SupportedException();
				else
					unboundValues["Delay"] = value;
			}
		}

		/// <summary>
		/// Gets the non-localized trigger string for V2 triggers.
		/// </summary>
		/// <returns>String describing the trigger.</returns>
		protected override string V2GetTriggerString()
		{
			return Properties.Resources.TriggerBoot1;
		}
	}

	/// <summary>
	/// Represents a custom trigger. This class is based on undocumented features and may change.
	/// </summary>
	public sealed class CustomTrigger : Trigger, ITriggerDelay
	{
		private TimeSpan delay = TimeSpan.MinValue;
		private string name = string.Empty;
		private NamedValueCollection nvc = new NamedValueCollection();

		internal CustomTrigger(V2Interop.ITrigger iTrigger)
			: base(iTrigger)
		{
		}

		/// <summary>
		/// Gets the non-localized trigger string for V2 triggers.
		/// </summary>
		/// <returns>String describing the trigger.</returns>
		protected override string V2GetTriggerString()
		{
			return TaskScheduler.Properties.Resources.TriggerCustom1;
		}

		/// <summary>
		/// Updates custom properties from XML provided by definition.
		/// </summary>
		/// <param name="xml">The XML from the TaskDefinition.</param>
		internal void UpdateFromXml(string xml)
		{
			nvc.Clear();
			try
			{
				var xmlDoc = new System.Xml.XmlDocument();
				xmlDoc.LoadXml(xml);
				var nsmgr = new System.Xml.XmlNamespaceManager(xmlDoc.NameTable);
				nsmgr.AddNamespace("n", "http://schemas.microsoft.com/windows/2004/02/mit/task");
				System.Xml.XmlNode elem = xmlDoc.DocumentElement.SelectSingleNode("n:Triggers/*[@id='" + base.Id + "']", nsmgr);
				if (elem == null)
				{
					var nodes = xmlDoc.GetElementsByTagName("WnfStateChangeTrigger");
					if (nodes != null && nodes.Count == 1)
						elem = nodes[0];
				}
				if (elem != null)
				{
					name = elem.LocalName;
					foreach (System.Xml.XmlNode node in elem.ChildNodes)
					{
						switch (node.LocalName)
						{
							case "Delay":
								delay = Task.StringToTimeSpan(node.InnerText);
								break;
							case "StartBoundary":
							case "Enabled":
							case "EndBoundary":
							case "ExecutionTimeLimit":
								break;
							default:
								nvc.Add(node.LocalName, node.InnerText);
								break;
						}
					}
				}

			}
			catch { }
		}

		/// <summary>
		/// Gets the properties from the Xml definition if possible.
		/// </summary>
		[XmlArray, XmlArrayItem("Property")]
		public NamedValueCollection Properties
		{
			get { return nvc; }
		}

		/// <summary>
		/// Gets the name of the custom trigger type.
		/// </summary>
		/// <value>
		/// The name of the XML element representing this custom trigger.
		/// </value>
		public string Name
		{
			get { return name; }
		}

		/// <summary>
		/// Gets a value that indicates the amount of time between the trigger events and when the task is started.
		/// </summary>
		/// <exception cref="System.NotImplementedException">This value cannot be set.</exception>
		public TimeSpan Delay { get { return delay; } set { throw new NotImplementedException(); } }

		/// <summary>
		/// Clones this instance.
		/// </summary>
		/// <returns>This method will always throw an exception.</returns>
		/// <exception cref="System.InvalidOperationException">CustomTrigger cannot be cloned due to OS restrictions.</exception>
		public override object Clone()
		{
			throw new InvalidOperationException("CustomTrigger cannot be cloned due to OS restrictions.");
		}
	}

	internal static class CalendarTrigger
	{
		internal delegate void CalendarXmlReader(System.Xml.XmlReader reader);
		internal delegate void CalendarXmlWriter(System.Xml.XmlWriter writer);

		internal static Trigger GetTriggerFromXml(System.Xml.XmlReader reader)
		{
			Trigger t = null;
			string xml = reader.ReadOuterXml();
			var match = System.Text.RegularExpressions.Regex.Match(xml, @"\<(?<T>ScheduleBy.+)\>");
			if (match.Success && match.Groups.Count == 2)
			{
				switch (match.Groups[1].Value)
				{
					case "ScheduleByDay":
						t = new DailyTrigger();
						break;
					case "ScheduleByWeek":
						t = new WeeklyTrigger();
						break;
					case "ScheduleByMonth":
						t = new MonthlyTrigger();
						break;
					case "ScheduleByMonthDayOfWeek":
						t = new MonthlyDOWTrigger();
						break;
					default:
						break;
				}

				if (t != null)
				{
					using (System.IO.StringReader ms = new System.IO.StringReader(xml))
					{
						using (System.Xml.XmlReader iReader = System.Xml.XmlReader.Create(ms))
						{
							((IXmlSerializable)t).ReadXml(iReader);
						}
					}
				}
			}
			return t;
		}

		internal static void ReadXml(System.Xml.XmlReader reader, Trigger t, CalendarXmlReader calReaderProc)
		{
			reader.ReadStartElement("CalendarTrigger", TaskDefinition.tns);
			while (reader.MoveToContent() == System.Xml.XmlNodeType.Element)
			{
				switch (reader.LocalName)
				{
					case "Enabled":
						t.Enabled = reader.ReadElementContentAsBoolean();
						break;
					case "EndBoundary":
						t.EndBoundary = reader.ReadElementContentAsDateTime();
						break;
					case "RandomDelay":
						((ITriggerDelay)t).Delay = Task.StringToTimeSpan(reader.ReadElementContentAsString());
						break;
					case "StartBoundary":
						t.StartBoundary = reader.ReadElementContentAsDateTime();
						break;
					case "Repetition":
						XmlSerializationHelper.ReadObject(reader, t.Repetition);
						break;
					case "ScheduleByDay":
					case "ScheduleByWeek":
					case "ScheduleByMonth":
					case "ScheduleByMonthDayOfWeek":
						calReaderProc(reader);
						break;
					default:
						reader.Skip();
						break;
				}
			}
			reader.ReadEndElement();
		}

		public static void WriteXml(System.Xml.XmlWriter writer, Trigger t, CalendarXmlWriter calWriterProc)
		{
			if (!t.Enabled)
				writer.WriteElementString("Enabled", System.Xml.XmlConvert.ToString(t.Enabled));
			if (t.EndBoundary != DateTime.MaxValue)
				writer.WriteElementString("EndBoundary", System.Xml.XmlConvert.ToString(t.EndBoundary, System.Xml.XmlDateTimeSerializationMode.RoundtripKind));
			XmlSerializationHelper.WriteObject(writer, t.Repetition);
			writer.WriteElementString("StartBoundary", System.Xml.XmlConvert.ToString(t.StartBoundary, System.Xml.XmlDateTimeSerializationMode.RoundtripKind));
			calWriterProc(writer);
		}
	}

	/// <summary>
	/// Represents a trigger that starts a task based on a daily schedule. For example, the task starts at a specific time every day, every other day, every third day, and so on.
	/// </summary>
	[XmlRoot("CalendarTrigger", Namespace = TaskDefinition.tns, IsNullable = false)]
	public sealed class DailyTrigger : Trigger, ICalendarTrigger, ITriggerDelay, IXmlSerializable
	{
		/// <summary>
		/// Creates an unbound instance of a <see cref="DailyTrigger"/>.
		/// </summary>
		/// <param name="daysInterval">Interval between the days in the schedule.</param>
		public DailyTrigger(short daysInterval = 1)
			: base(TaskTriggerType.Daily)
		{
			this.DaysInterval = daysInterval;
		}

		internal DailyTrigger(V1Interop.ITaskTrigger iTrigger)
			: base(iTrigger, V1Interop.TaskTriggerType.RunDaily)
		{
			if (v1TriggerData.Data.daily.DaysInterval == 0)
				v1TriggerData.Data.daily.DaysInterval = 1;
		}

		internal DailyTrigger(V2Interop.ITrigger iTrigger)
			: base(iTrigger)
		{
		}

		/// <summary>
		/// Sets or retrieves the interval between the days in the schedule.
		/// </summary>
		[DefaultValue(1)]
		public short DaysInterval
		{
			get
			{
				if (v2Trigger != null)
					return ((V2Interop.IDailyTrigger)v2Trigger).DaysInterval;
				return (short)v1TriggerData.Data.daily.DaysInterval;
			}
			set
			{
				if (v2Trigger != null)
					((V2Interop.IDailyTrigger)v2Trigger).DaysInterval = value;
				else
				{
					v1TriggerData.Data.daily.DaysInterval = (ushort)value;
					if (v1Trigger != null)
						SetV1TriggerData();
					else
						unboundValues["DaysInterval"] = value;
				}
			}
		}

		/// <summary>
		/// Gets or sets a delay time that is randomly added to the start time of the trigger.
		/// </summary>
		/// <exception cref="NotV1SupportedException">Not supported under Task Scheduler 1.0.</exception>
		[DefaultValue(typeof(TimeSpan), "00:00:00")]
		[XmlIgnore]
		public TimeSpan RandomDelay
		{
			get
			{
				if (v2Trigger != null)
					return Task.StringToTimeSpan(((V2Interop.IDailyTrigger)v2Trigger).RandomDelay);
				if (v1Trigger != null)
					throw new NotV1SupportedException();
				return (unboundValues.ContainsKey("RandomDelay") ? (TimeSpan)unboundValues["RandomDelay"] : TimeSpan.Zero);
			}
			set
			{
				if (v2Trigger != null)
					((V2Interop.IDailyTrigger)v2Trigger).RandomDelay = Task.TimeSpanToString(value);
				else if (v1Trigger != null)
					throw new NotV1SupportedException();
				else
					unboundValues["RandomDelay"] = value;
			}
		}

		/// <summary>
		/// Gets or sets a value that indicates the amount of time before the task is started.
		/// </summary>
		/// <value>The delay duration.</value>
		TimeSpan ITriggerDelay.Delay
		{
			get { return this.RandomDelay; }
			set { this.RandomDelay = value; }
		}

		/// <summary>
		/// Copies the properties from another <see cref="Trigger"/> the current instance. This will not copy any properties associated with any derived triggers except those supporting the <see cref="ITriggerDelay"/> interface.
		/// </summary>
		/// <param name="sourceTrigger">The source <see cref="Trigger"/>.</param>
		public override void CopyProperties(Trigger sourceTrigger)
		{
			base.CopyProperties(sourceTrigger);
			if (sourceTrigger.GetType() == this.GetType())
			{
				this.DaysInterval = ((DailyTrigger)sourceTrigger).DaysInterval;
			}
		}

		/// <summary>
		/// Indicates whether the current object is equal to another object of the same type.
		/// </summary>
		/// <param name="other">An object to compare with this object.</param>
		/// <returns>
		/// true if the current object is equal to the <paramref name="other" /> parameter; otherwise, false.
		/// </returns>
		public override bool Equals(Trigger other)
		{
			return base.Equals(other) && this.DaysInterval == ((DailyTrigger)other).DaysInterval;
		}

		/// <summary>
		/// Gets the non-localized trigger string for V2 triggers.
		/// </summary>
		/// <returns>String describing the trigger.</returns>
		protected override string V2GetTriggerString()
		{
			if (this.DaysInterval == 1)
				return string.Format(Properties.Resources.TriggerDaily1, AdjustToLocal(this.StartBoundary));
			return string.Format(Properties.Resources.TriggerDaily2, AdjustToLocal(this.StartBoundary), this.DaysInterval);
		}

		System.Xml.Schema.XmlSchema IXmlSerializable.GetSchema()
		{
			return null;
		}

		void ReadMyXml(System.Xml.XmlReader reader)
		{
			reader.ReadStartElement("ScheduleByDay");
			if (reader.MoveToContent() == System.Xml.XmlNodeType.Element && reader.LocalName == "DaysInterval")
				this.DaysInterval = (short)reader.ReadElementContentAs(typeof(short), null);
			reader.Read();
			reader.ReadEndElement();
		}

		void IXmlSerializable.ReadXml(System.Xml.XmlReader reader)
		{
			CalendarTrigger.ReadXml(reader, this, this.ReadMyXml);
		}

		void WriteMyXml(System.Xml.XmlWriter writer)
		{
			if (this.DaysInterval != 1)
			{
				writer.WriteStartElement("ScheduleByDay");
				writer.WriteElementString("DaysInterval", this.DaysInterval.ToString());
				writer.WriteEndElement();
			}
		}

		void IXmlSerializable.WriteXml(System.Xml.XmlWriter writer)
		{
			CalendarTrigger.WriteXml(writer, this, this.WriteMyXml);
		}
	}

	/// <summary>
	/// Represents a trigger that starts a task when a system event occurs. Not available on Task Scheduler 1.0.
	/// </summary>
	[XmlType(IncludeInSchema = false)]
	public sealed class EventTrigger : Trigger, ITriggerDelay
	{
		private NamedValueCollection nvc = null;

		/// <summary>
		/// Creates an unbound instance of a <see cref="EventTrigger"/>.
		/// </summary>
		public EventTrigger()
			: base(TaskTriggerType.Event)
		{
		}

		/// <summary>
		/// Initializes an unbound instance of the <see cref="EventTrigger"/> class and sets a basic event.
		/// </summary>
		/// <param name="log">The event's log.</param>
		/// <param name="source">The event's source. Can be <c>null</c>.</param>
		/// <param name="eventId">The event's id. Can be <c>null</c>.</param>
		public EventTrigger(string log, string source, int? eventId) : this() { SetBasic(log, source, eventId); }

		internal EventTrigger(V2Interop.ITrigger iTrigger)
			: base(iTrigger)
		{
		}

		/// <summary>
		/// Gets or sets a value that indicates the amount of time between when the system is booted and when the task is started.
		/// </summary>
		[DefaultValue(typeof(TimeSpan), "00:00:00")]
		public TimeSpan Delay
		{
			get
			{
				if (v2Trigger != null)
					return Task.StringToTimeSpan(((V2Interop.IEventTrigger)v2Trigger).Delay);
				return (unboundValues.ContainsKey("Delay") ? (TimeSpan)unboundValues["Delay"] : TimeSpan.Zero);
			}
			set
			{
				if (v2Trigger != null)
					((V2Interop.IEventTrigger)v2Trigger).Delay = Task.TimeSpanToString(value);
				else
					unboundValues["Delay"] = value;
			}
		}

		/// <summary>
		/// Gets or sets the XPath query string that identifies the event that fires the trigger.
		/// </summary>
		[DefaultValue((string)null)]
		public string Subscription
		{
			get
			{
				if (v2Trigger != null)
					return ((V2Interop.IEventTrigger)v2Trigger).Subscription;
				return (unboundValues.ContainsKey("Subscription") ? (string)unboundValues["Subscription"] : null);
			}
			set
			{
				if (v2Trigger != null)
					((V2Interop.IEventTrigger)v2Trigger).Subscription = value;
				else
					unboundValues["Subscription"] = value;
			}
		}

		/// <summary>
		/// Gets a collection of named XPath queries. Each query in the collection is applied to the last matching event XML returned from the subscription query specified in the Subscription property. The name of the query can be used as a variable in the message of a <see cref="ShowMessageAction"/> action.
		/// </summary>
		[XmlArray]
		[XmlArrayItem("Value", typeof(NameValuePair))]
		public NamedValueCollection ValueQueries
		{
			get
			{
				if (nvc == null)
				{
					if (v2Trigger == null)
						nvc = new NamedValueCollection();
					else
						nvc = new NamedValueCollection(((V2Interop.IEventTrigger)v2Trigger).ValueQueries);
				}
				return nvc;
			}
		}

		/// <summary>
		/// Copies the properties from another <see cref="Trigger"/> the current instance. This will not copy any properties associated with any derived triggers except those supporting the <see cref="ITriggerDelay"/> interface.
		/// </summary>
		/// <param name="sourceTrigger">The source <see cref="Trigger"/>.</param>
		public override void CopyProperties(Trigger sourceTrigger)
		{
			base.CopyProperties(sourceTrigger);
			if (sourceTrigger.GetType() == this.GetType())
			{
				this.Subscription = ((EventTrigger)sourceTrigger).Subscription;
				((EventTrigger)sourceTrigger).ValueQueries.CopyTo(this.ValueQueries);
			}
		}

		/// <summary>
		/// Indicates whether the current object is equal to another object of the same type.
		/// </summary>
		/// <param name="other">An object to compare with this object.</param>
		/// <returns>
		/// true if the current object is equal to the <paramref name="other" /> parameter; otherwise, false.
		/// </returns>
		public override bool Equals(Trigger other)
		{
			return base.Equals(other) && this.Subscription == ((EventTrigger)other).Subscription;
		}

		/// <summary>
		/// Builds an event log XML query string based on the input parameters.
		/// </summary>
		/// <param name="log">The event's log.</param>
		/// <param name="source">The event's source. Can be <c>null</c>.</param>
		/// <param name="eventId">The event's id. Can be <c>null</c>.</param>
		/// <returns>XML query string.</returns>
		/// <exception cref="System.ArgumentNullException">log</exception>
		public static string BuildQuery(string log, string source, int? eventId)
		{
			StringBuilder sb = new StringBuilder();
			if (string.IsNullOrEmpty(log))
				throw new ArgumentNullException("log");
			sb.AppendFormat("<QueryList><Query Id=\"0\" Path=\"{0}\"><Select Path=\"{0}\">*", log);
			bool hasSource = !string.IsNullOrEmpty(source), hasId = eventId.HasValue;
			if (hasSource || hasId)
			{
				sb.Append("[System[");
				if (hasSource)
					sb.AppendFormat("Provider[@Name='{0}']", source);
				if (hasSource && hasId)
					sb.Append(" and ");
				if (hasId)
					sb.AppendFormat("EventID={0}", eventId.Value);
				sb.Append("]]");
			}
			sb.Append("</Select></Query></QueryList>");
			return sb.ToString();
		}

		/// <summary>
		/// Gets basic event information.
		/// </summary>
		/// <param name="log">The event's log.</param>
		/// <param name="source">The event's source. Can be <c>null</c>.</param>
		/// <param name="eventId">The event's id. Can be <c>null</c>.</param>
		/// <returns><c>true</c> if subscription represents a basic event, <c>false</c> if not.</returns>
		public bool GetBasic(out string log, out string source, out int? eventId)
		{
			log = source = null;
			eventId = null;
			if (!string.IsNullOrEmpty(this.Subscription))
			{
				using (System.IO.MemoryStream str = new System.IO.MemoryStream(System.Text.Encoding.UTF8.GetBytes(this.Subscription)))
				{
					using (System.Xml.XmlTextReader rdr = new System.Xml.XmlTextReader(str))
					{
						rdr.WhitespaceHandling = System.Xml.WhitespaceHandling.None;
						try
						{
							rdr.MoveToContent();
							rdr.ReadStartElement("QueryList");
							if (rdr.Name == "Query" && rdr.MoveToAttribute("Path"))
							{
								string path = rdr.Value;
								if (rdr.MoveToElement() && rdr.ReadToDescendant("Select") && path.Equals(rdr["Path"], StringComparison.InvariantCultureIgnoreCase))
								{
									string content = rdr.ReadString();
									System.Text.RegularExpressions.Match m = System.Text.RegularExpressions.Regex.Match(content,
										@"\*(?:\[System\[(?:Provider\[\@Name='(?<s>[^']+)'\])?(?:\s+and\s+)?(?:EventID=(?<e>\d+))?\]\])",
										System.Text.RegularExpressions.RegexOptions.IgnoreCase |
										System.Text.RegularExpressions.RegexOptions.Compiled |
										System.Text.RegularExpressions.RegexOptions.Singleline |
										System.Text.RegularExpressions.RegexOptions.IgnorePatternWhitespace);
									if (m.Success)
									{
										log = path;
										if (m.Groups["s"].Success)
											source = m.Groups["s"].Value;
										if (m.Groups["e"].Success)
											eventId = Convert.ToInt32(m.Groups["e"].Value);
										return true;
									}
								}
							}
						}
						catch { }
					}
				}
			}
			return false;
		}

		/// <summary>
		/// Sets the subscription for a basic event. This will replace the contents of the <see cref="Subscription"/> property and clear all entries in the <see cref="ValueQueries"/> property.
		/// </summary>
		/// <param name="log">The event's log.</param>
		/// <param name="source">The event's source. Can be <c>null</c>.</param>
		/// <param name="eventId">The event's id. Can be <c>null</c>.</param>
		public void SetBasic(string log, string source, int? eventId)
		{
			this.ValueQueries.Clear();
			this.Subscription = BuildQuery(log, source, eventId);
		}

		internal override void Bind(Microsoft.Win32.TaskScheduler.V2Interop.ITaskDefinition iTaskDef)
		{
			base.Bind(iTaskDef);
			if (nvc != null)
				nvc.Bind(((V2Interop.IEventTrigger)v2Trigger).ValueQueries);
		}

		/// <summary>
		/// Gets the non-localized trigger string for V2 triggers.
		/// </summary>
		/// <returns>String describing the trigger.</returns>
		protected override string V2GetTriggerString()
		{
			string log, source; int? id;
			if (this.GetBasic(out log, out source, out id))
			{
				StringBuilder sb = new StringBuilder();
				sb.AppendFormat(Properties.Resources.TriggerEventBasic1, log);
				if (!string.IsNullOrEmpty(source))
					sb.AppendFormat(Properties.Resources.TriggerEventBasic2, source);
				if (id.HasValue)
					sb.AppendFormat(Properties.Resources.TriggerEventBasic3, id.Value);
				return sb.ToString();
			}
			return Properties.Resources.TriggerEvent1;
		}
	}

	/// <summary>
	/// Represents a trigger that starts a task when the computer goes into an idle state. For information about idle conditions, see Task Idle Conditions.
	/// </summary>
	public sealed class IdleTrigger : Trigger
	{
		/// <summary>
		/// Creates an unbound instance of a <see cref="IdleTrigger"/>.
		/// </summary>
		public IdleTrigger()
			: base(TaskTriggerType.Idle)
		{
		}

		internal IdleTrigger(V1Interop.ITaskTrigger iTrigger)
			: base(iTrigger, V1Interop.TaskTriggerType.OnIdle)
		{
		}

		internal IdleTrigger(V2Interop.ITrigger iTrigger)
			: base(iTrigger)
		{
		}

		/// <summary>
		/// Gets the non-localized trigger string for V2 triggers.
		/// </summary>
		/// <returns>String describing the trigger.</returns>
		protected override string V2GetTriggerString()
		{
			return Properties.Resources.TriggerIdle1;
		}
	}

	/// <summary>
	/// Represents a trigger that starts a task when a user logs on. When the Task Scheduler service starts, all logged-on users are enumerated and any tasks registered with logon triggers that match the logged on user are run. Not available on Task Scheduler 1.0.
	/// </summary>
	public sealed class LogonTrigger : Trigger, ITriggerDelay, ITriggerUserId
	{
		/// <summary>
		/// Creates an unbound instance of a <see cref="LogonTrigger"/>.
		/// </summary>
		public LogonTrigger()
			: base(TaskTriggerType.Logon)
		{
		}

		internal LogonTrigger(V1Interop.ITaskTrigger iTrigger)
			: base(iTrigger, V1Interop.TaskTriggerType.OnLogon)
		{
		}

		internal LogonTrigger(V2Interop.ITrigger iTrigger)
			: base(iTrigger)
		{
		}

		/// <summary>
		/// Gets or sets a value that indicates the amount of time between when the system is booted and when the task is started.
		/// </summary>
		/// <exception cref="NotV1SupportedException">Not supported under Task Scheduler 1.0.</exception>
		[DefaultValue(typeof(TimeSpan), "00:00:00")]
		[XmlIgnore]
		public TimeSpan Delay
		{
			get
			{
				if (v2Trigger != null)
					return Task.StringToTimeSpan(((V2Interop.ILogonTrigger)v2Trigger).Delay);
				if (v1Trigger != null)
					throw new NotV1SupportedException();
				return (unboundValues.ContainsKey("Delay") ? (TimeSpan)unboundValues["Delay"] : TimeSpan.Zero);
			}
			set
			{
				if (v2Trigger != null)
					((V2Interop.ILogonTrigger)v2Trigger).Delay = Task.TimeSpanToString(value);
				else if (v1Trigger != null)
					throw new NotV1SupportedException();
				else
					unboundValues["Delay"] = value;
			}
		}

		/// <summary>
		/// <para>Gets or sets The identifier of the user. For example, "MyDomain\MyName" or for a local account, "Administrator".</para>
		/// <para>This property can be in one of the following formats:</para>
		/// <para>• User name or SID: The task is started when the user logs on to the computer.</para>
		/// <para>• NULL: The task is started when any user logs on to the computer.</para>
		/// </summary>
		/// <remarks>If you want a task to be triggered when any member of a group logs on to the computer rather than when a specific user logs on, then do not assign a value to the LogonTrigger.UserId property. Instead, create a logon trigger with an empty LogonTrigger.UserId property and assign a value to the principal for the task using the Principal.GroupId property.</remarks>
		/// <exception cref="NotV1SupportedException">Not supported under Task Scheduler 1.0.</exception>
		[DefaultValue((string)null)]
		[XmlIgnore]
		public string UserId
		{
			get
			{
				if (v2Trigger != null)
					return ((V2Interop.ILogonTrigger)v2Trigger).UserId;
				if (v1Trigger != null)
					throw new NotV1SupportedException();
				return (unboundValues.ContainsKey("UserId") ? (string)unboundValues["UserId"] : null);
			}
			set
			{
				if (v2Trigger != null)
					((V2Interop.ILogonTrigger)v2Trigger).UserId = value;
				else if (v1Trigger != null)
					throw new NotV1SupportedException();
				else
					unboundValues["UserId"] = value;
			}
		}

		/// <summary>
		/// Gets the non-localized trigger string for V2 triggers.
		/// </summary>
		/// <returns>String describing the trigger.</returns>
		protected override string V2GetTriggerString()
		{
			string user = string.IsNullOrEmpty(this.UserId) ? Properties.Resources.TriggerAnyUser : this.UserId;
			return string.Format(Properties.Resources.TriggerLogon1, user);
		}
	}

	/// <summary>
	/// Represents a trigger that starts a task on a monthly day-of-week schedule. For example, the task starts on every first Thursday, May through October.
	/// </summary>
	[XmlRoot("CalendarTrigger", Namespace = TaskDefinition.tns, IsNullable = false)]
	public sealed class MonthlyDOWTrigger : Trigger, ICalendarTrigger, ITriggerDelay, IXmlSerializable
	{
		/// <summary>
		/// Creates an unbound instance of a <see cref="MonthlyDOWTrigger"/>.
		/// </summary>
		/// <param name="daysOfWeek">The days of the week.</param>
		/// <param name="monthsOfYear">The months of the year.</param>
		/// <param name="weeksOfMonth">The weeks of the month.</param>
		public MonthlyDOWTrigger(DaysOfTheWeek daysOfWeek = DaysOfTheWeek.Sunday, MonthsOfTheYear monthsOfYear = MonthsOfTheYear.AllMonths, WhichWeek weeksOfMonth = WhichWeek.FirstWeek)
			: base(TaskTriggerType.MonthlyDOW)
		{
			this.DaysOfWeek = daysOfWeek;
			this.MonthsOfYear = monthsOfYear;
			this.WeeksOfMonth = weeksOfMonth;
		}

		internal MonthlyDOWTrigger(V1Interop.ITaskTrigger iTrigger)
			: base(iTrigger, V1Interop.TaskTriggerType.RunMonthlyDOW)
		{
			if (v1TriggerData.Data.monthlyDOW.Months == 0)
				v1TriggerData.Data.monthlyDOW.Months = MonthsOfTheYear.AllMonths;
			if (v1TriggerData.Data.monthlyDOW.DaysOfTheWeek == 0)
				v1TriggerData.Data.monthlyDOW.DaysOfTheWeek = DaysOfTheWeek.Sunday;
		}

		internal MonthlyDOWTrigger(V2Interop.ITrigger iTrigger)
			: base(iTrigger)
		{
		}

		/// <summary>
		/// Gets or sets the days of the week during which the task runs.
		/// </summary>
		[DefaultValue(0)]
		public DaysOfTheWeek DaysOfWeek
		{
			get
			{
				if (v2Trigger != null)
					return (DaysOfTheWeek)((V2Interop.IMonthlyDOWTrigger)v2Trigger).DaysOfWeek;
				return (DaysOfTheWeek)v1TriggerData.Data.monthlyDOW.DaysOfTheWeek;
			}
			set
			{
				if (v2Trigger != null)
					((V2Interop.IMonthlyDOWTrigger)v2Trigger).DaysOfWeek = (short)value;
				else
				{
					v1TriggerData.Data.monthlyDOW.DaysOfTheWeek = value;
					if (v1Trigger != null)
						SetV1TriggerData();
					else
						unboundValues["DaysOfWeek"] = (short)value;
				}
			}
		}

		/// <summary>
		/// Gets or sets the months of the year during which the task runs.
		/// </summary>
		[DefaultValue(0)]
		public MonthsOfTheYear MonthsOfYear
		{
			get
			{
				if (v2Trigger != null)
					return (MonthsOfTheYear)((V2Interop.IMonthlyDOWTrigger)v2Trigger).MonthsOfYear;
				return (MonthsOfTheYear)v1TriggerData.Data.monthlyDOW.Months;
			}
			set
			{
				if (v2Trigger != null)
					((V2Interop.IMonthlyDOWTrigger)v2Trigger).MonthsOfYear = (short)value;
				else
				{
					v1TriggerData.Data.monthlyDOW.Months = value;
					if (v1Trigger != null)
						SetV1TriggerData();
					else
						unboundValues["MonthsOfYear"] = (short)value;
				}
			}
		}

		/// <summary>
		/// Gets or sets a delay time that is randomly added to the start time of the trigger.
		/// </summary>
		/// <exception cref="NotV1SupportedException">Not supported under Task Scheduler 1.0.</exception>
		[DefaultValue(typeof(TimeSpan), "00:00:00")]
		[XmlIgnore]
		public TimeSpan RandomDelay
		{
			get
			{
				if (v2Trigger != null)
					return Task.StringToTimeSpan(((V2Interop.IMonthlyDOWTrigger)v2Trigger).RandomDelay);
				if (v1Trigger != null)
					throw new NotV1SupportedException();
				return (unboundValues.ContainsKey("RandomDelay") ? (TimeSpan)unboundValues["RandomDelay"] : TimeSpan.Zero);
			}
			set
			{
				if (v2Trigger != null)
					((V2Interop.IMonthlyDOWTrigger)v2Trigger).RandomDelay = Task.TimeSpanToString(value);
				else if (v1Trigger != null)
					throw new NotV1SupportedException();
				else
					unboundValues["RandomDelay"] = value;
			}
		}

		/// <summary>
		/// Gets or sets a Boolean value that indicates that the task runs on the last week of the month.
		/// </summary>
		/// <exception cref="NotV1SupportedException">Not supported under Task Scheduler 1.0.</exception>
		[DefaultValue(false)]
		[XmlIgnore]
		public bool RunOnLastWeekOfMonth
		{
			get
			{
				if (v2Trigger != null)
					return ((V2Interop.IMonthlyDOWTrigger)v2Trigger).RunOnLastWeekOfMonth;
				else if (v1Trigger != null)
					throw new NotV1SupportedException();
				else
					return (unboundValues.ContainsKey("RunOnLastWeekOfMonth") ? (bool)unboundValues["RunOnLastWeekOfMonth"] : false);
			}
			set
			{
				if (v2Trigger != null)
					((V2Interop.IMonthlyDOWTrigger)v2Trigger).RunOnLastWeekOfMonth = value;
				else if (v1Trigger != null)
					throw new NotV1SupportedException();
				else
					unboundValues["RunOnLastWeekOfMonth"] = value;
			}
		}

		/// <summary>
		/// Gets or sets the weeks of the month during which the task runs.
		/// </summary>
		[DefaultValue(0)]
		public WhichWeek WeeksOfMonth
		{
			get
			{
				if (v2Trigger != null)
				{
					WhichWeek ww = (WhichWeek)((V2Interop.IMonthlyDOWTrigger)v2Trigger).WeeksOfMonth;
					// Following addition give accurate results for confusing RunOnLastWeekOfMonth property (thanks kbergeron)
					if (((V2Interop.IMonthlyDOWTrigger)v2Trigger).RunOnLastWeekOfMonth)
						ww |= WhichWeek.LastWeek;
					return ww;
				}
				else if (v1Trigger != null)
					return v1TriggerData.Data.monthlyDOW.V2WhichWeek;
				else
					return (unboundValues.ContainsKey("WeeksOfMonth") ? (WhichWeek)unboundValues["WeeksOfMonth"] : WhichWeek.FirstWeek);
			}
			set
			{
				if (v2Trigger != null)
					((V2Interop.IMonthlyDOWTrigger)v2Trigger).WeeksOfMonth = (short)value;
				else
				{
					try
					{
						v1TriggerData.Data.monthlyDOW.V2WhichWeek = value;
					}
					catch (NotV1SupportedException)
					{
						if (v1Trigger != null) throw;
					}
					if (v1Trigger != null)
						SetV1TriggerData();
					else
						unboundValues["WeeksOfMonth"] = (short)value;
				}
			}
		}

		/// <summary>
		/// Gets or sets a value that indicates the amount of time before the task is started.
		/// </summary>
		/// <value>The delay duration.</value>
		TimeSpan ITriggerDelay.Delay
		{
			get { return this.RandomDelay; }
			set { this.RandomDelay = value; }
		}

		/// <summary>
		/// Copies the properties from another <see cref="Trigger"/> the current instance. This will not copy any properties associated with any derived triggers except those supporting the <see cref="ITriggerDelay"/> interface.
		/// </summary>
		/// <param name="sourceTrigger">The source <see cref="Trigger"/>.</param>
		public override void CopyProperties(Trigger sourceTrigger)
		{
			base.CopyProperties(sourceTrigger);
			if (sourceTrigger.GetType() == this.GetType())
			{
				this.DaysOfWeek = ((MonthlyDOWTrigger)sourceTrigger).DaysOfWeek;
				this.MonthsOfYear = ((MonthlyDOWTrigger)sourceTrigger).MonthsOfYear;
				try { this.RunOnLastWeekOfMonth = ((MonthlyDOWTrigger)sourceTrigger).RunOnLastWeekOfMonth; } catch { }
				this.WeeksOfMonth = ((MonthlyDOWTrigger)sourceTrigger).WeeksOfMonth;
			}
		}

		/// <summary>
		/// Indicates whether the current object is equal to another object of the same type.
		/// </summary>
		/// <param name="other">An object to compare with this object.</param>
		/// <returns>
		/// true if the current object is equal to the <paramref name="other" /> parameter; otherwise, false.
		/// </returns>
		public override bool Equals(Trigger other)
		{
			return base.Equals(other) && this.DaysOfWeek == ((MonthlyDOWTrigger)other).DaysOfWeek &&
				this.MonthsOfYear == ((MonthlyDOWTrigger)other).MonthsOfYear && this.WeeksOfMonth == ((MonthlyDOWTrigger)other).WeeksOfMonth &&
				(this.v1Trigger == null && this.RunOnLastWeekOfMonth == ((MonthlyDOWTrigger)other).RunOnLastWeekOfMonth);
		}

		/// <summary>
		/// Reads the subclass XML for V1 streams.
		/// </summary>
		/// <param name="reader">The reader.</param>
		void ReadMyXml(System.Xml.XmlReader reader)
		{
			reader.ReadStartElement("ScheduleByMonthDayOfWeek");
			while (reader.MoveToContent() == System.Xml.XmlNodeType.Element)
			{
				switch (reader.LocalName)
				{
					case "Weeks":
						reader.Read();
						while (reader.MoveToContent() == System.Xml.XmlNodeType.Element)
						{
							if (reader.LocalName == "Week")
							{
								string wk = reader.ReadElementContentAsString();
								if (wk == "Last")
									this.WeeksOfMonth = WhichWeek.LastWeek;
								else
								{
									switch (Int32.Parse(wk))
									{
										case 1:
											this.WeeksOfMonth = WhichWeek.FirstWeek;
											break;
										case 2:
											this.WeeksOfMonth = WhichWeek.SecondWeek;
											break;
										case 3:
											this.WeeksOfMonth = WhichWeek.ThirdWeek;
											break;
										case 4:
											this.WeeksOfMonth = WhichWeek.FourthWeek;
											break;
										default:
											throw new System.Xml.XmlException("Week element must contain a 1-4 or Last as content.");
									}
								}
							}
						}
						reader.ReadEndElement();
						break;
					case "DaysOfWeek":
						reader.Read();
						this.DaysOfWeek = 0;
						while (reader.MoveToContent() == System.Xml.XmlNodeType.Element)
						{
							try
							{
								this.DaysOfWeek |= (DaysOfTheWeek)Enum.Parse(typeof(DaysOfTheWeek), reader.LocalName);
							}
							catch
							{
								throw new System.Xml.XmlException("Invalid days of the week element.");
							}
							reader.Read();
						}
						reader.ReadEndElement();
						break;
					case "Months":
						reader.Read();
						this.MonthsOfYear = 0;
						while (reader.MoveToContent() == System.Xml.XmlNodeType.Element)
						{
							try
							{
								this.MonthsOfYear |= (MonthsOfTheYear)Enum.Parse(typeof(MonthsOfTheYear), reader.LocalName);
							}
							catch
							{
								throw new System.Xml.XmlException("Invalid months of the year element.");
							}
							reader.Read();
						}
						reader.ReadEndElement();
						break;
					default:
						reader.Skip();
						break;
				}
			}
			reader.ReadEndElement();
		}

		/// <summary>
		/// Gets the non-localized trigger string for V2 triggers.
		/// </summary>
		/// <returns>String describing the trigger.</returns>
		protected override string V2GetTriggerString()
		{
			string ww = TaskEnumGlobalizer.GetString(this.WeeksOfMonth);
			string days = TaskEnumGlobalizer.GetString(this.DaysOfWeek);
			string months = TaskEnumGlobalizer.GetString(this.MonthsOfYear);
			return string.Format(Properties.Resources.TriggerMonthlyDOW1, AdjustToLocal(this.StartBoundary), ww, days, months);
		}

		/// <summary>
		/// Writes the subclass XML for V1 streams.
		/// </summary>
		/// <param name="writer">The writer.</param>
		void WriteMyXml(System.Xml.XmlWriter writer)
		{
			writer.WriteStartElement("ScheduleByMonthDayOfWeek");

			writer.WriteStartElement("Weeks");
			if ((this.WeeksOfMonth & WhichWeek.FirstWeek) == WhichWeek.FirstWeek)
				writer.WriteElementString("Week", "1");
			if ((this.WeeksOfMonth & WhichWeek.SecondWeek) == WhichWeek.SecondWeek)
				writer.WriteElementString("Week", "2");
			if ((this.WeeksOfMonth & WhichWeek.ThirdWeek) == WhichWeek.ThirdWeek)
				writer.WriteElementString("Week", "3");
			if ((this.WeeksOfMonth & WhichWeek.FourthWeek) == WhichWeek.FourthWeek)
				writer.WriteElementString("Week", "4");
			if ((this.WeeksOfMonth & WhichWeek.LastWeek) == WhichWeek.LastWeek)
				writer.WriteElementString("Week", "Last");
			writer.WriteEndElement();

			writer.WriteStartElement("DaysOfWeek");
			foreach (DaysOfTheWeek e in Enum.GetValues(typeof(DaysOfTheWeek)))
				if (e != DaysOfTheWeek.AllDays && (this.DaysOfWeek & e) == e)
					writer.WriteElementString(e.ToString(), null);
			writer.WriteEndElement();

			writer.WriteStartElement("Months");
			foreach (MonthsOfTheYear e in Enum.GetValues(typeof(MonthsOfTheYear)))
				if (e != MonthsOfTheYear.AllMonths && (this.MonthsOfYear & e) == e)
					writer.WriteElementString(e.ToString(), null);
			writer.WriteEndElement();

			writer.WriteEndElement();
		}

		System.Xml.Schema.XmlSchema IXmlSerializable.GetSchema()
		{
			return null;
		}

		void IXmlSerializable.ReadXml(System.Xml.XmlReader reader)
		{
			CalendarTrigger.ReadXml(reader, this, this.ReadMyXml);
		}

		void IXmlSerializable.WriteXml(System.Xml.XmlWriter writer)
		{
			CalendarTrigger.WriteXml(writer, this, this.WriteMyXml);
		}
	}

	/// <summary>
	/// Represents a trigger that starts a job based on a monthly schedule. For example, the task starts on specific days of specific months.
	/// </summary>
	[XmlRoot("CalendarTrigger", Namespace = TaskDefinition.tns, IsNullable = false)]
	public sealed class MonthlyTrigger : Trigger, ICalendarTrigger, ITriggerDelay, IXmlSerializable
	{
		/// <summary>
		/// Creates an unbound instance of a <see cref="MonthlyTrigger"/>.
		/// </summary>
		/// <param name="dayOfMonth">The day of the month.</param>
		/// <param name="monthsOfYear">The months of the year.</param>
		public MonthlyTrigger(int dayOfMonth = 1, MonthsOfTheYear monthsOfYear = MonthsOfTheYear.AllMonths)
			: base(TaskTriggerType.Monthly)
		{
			this.DaysOfMonth = new int[] { dayOfMonth };
			this.MonthsOfYear = monthsOfYear;
		}

		internal MonthlyTrigger(V1Interop.ITaskTrigger iTrigger)
			: base(iTrigger, V1Interop.TaskTriggerType.RunMonthly)
		{
			if (v1TriggerData.Data.monthlyDate.Months == 0)
				v1TriggerData.Data.monthlyDate.Months = MonthsOfTheYear.AllMonths;
			if (v1TriggerData.Data.monthlyDate.Days == 0)
				v1TriggerData.Data.monthlyDate.Days = 1;
		}

		internal MonthlyTrigger(V2Interop.ITrigger iTrigger)
			: base(iTrigger)
		{
		}

		/// <summary>
		/// Gets or sets the days of the month during which the task runs.
		/// </summary>
		public int[] DaysOfMonth
		{
			get
			{
				if (v2Trigger != null)
					return MaskToIndices(((V2Interop.IMonthlyTrigger)v2Trigger).DaysOfMonth);
				return MaskToIndices((int)v1TriggerData.Data.monthlyDate.Days);
			}
			set
			{
				int mask = IndicesToMask(value);
				if (v2Trigger != null)
					((V2Interop.IMonthlyTrigger)v2Trigger).DaysOfMonth = mask;
				else
				{
					v1TriggerData.Data.monthlyDate.Days = (uint)mask;
					if (v1Trigger != null)
						SetV1TriggerData();
					else
						unboundValues["DaysOfMonth"] = mask;
				}
			}
		}

		/// <summary>
		/// Gets or sets the months of the year during which the task runs.
		/// </summary>
		[DefaultValue(0)]
		public MonthsOfTheYear MonthsOfYear
		{
			get
			{
				if (v2Trigger != null)
					return (MonthsOfTheYear)((V2Interop.IMonthlyTrigger)v2Trigger).MonthsOfYear;
				return (MonthsOfTheYear)v1TriggerData.Data.monthlyDOW.Months;
			}
			set
			{
				if (v2Trigger != null)
					((V2Interop.IMonthlyTrigger)v2Trigger).MonthsOfYear = (short)value;
				else
				{
					v1TriggerData.Data.monthlyDOW.Months = value;
					if (v1Trigger != null)
						SetV1TriggerData();
					else
						unboundValues["MonthsOfYear"] = (short)value;
				}
			}
		}

		/// <summary>
		/// Gets or sets a delay time that is randomly added to the start time of the trigger.
		/// </summary>
		/// <exception cref="NotV1SupportedException">Not supported under Task Scheduler 1.0.</exception>
		[DefaultValue(typeof(TimeSpan), "00:00:00")]
		[XmlIgnore]
		public TimeSpan RandomDelay
		{
			get
			{
				if (v2Trigger != null)
					return Task.StringToTimeSpan(((V2Interop.IMonthlyTrigger)v2Trigger).RandomDelay);
				if (v1Trigger != null)
					throw new NotV1SupportedException();
				return (unboundValues.ContainsKey("RandomDelay") ? (TimeSpan)unboundValues["RandomDelay"] : TimeSpan.Zero);
			}
			set
			{
				if (v2Trigger != null)
					((V2Interop.IMonthlyTrigger)v2Trigger).RandomDelay = Task.TimeSpanToString(value);
				else if (v1Trigger != null)
					throw new NotV1SupportedException();
				else
					unboundValues["RandomDelay"] = value;
			}
		}

		/// <summary>
		/// Gets or sets a Boolean value that indicates that the task runs on the last day of the month.
		/// </summary>
		/// <exception cref="NotV1SupportedException">Not supported under Task Scheduler 1.0.</exception>
		[DefaultValue(false)]
		[XmlIgnore]
		public bool RunOnLastDayOfMonth
		{
			get
			{
				if (v2Trigger != null)
					return ((V2Interop.IMonthlyTrigger)v2Trigger).RunOnLastDayOfMonth;
				if (v1Trigger != null)
					throw new NotV1SupportedException();
				return (unboundValues.ContainsKey("RunOnLastDayOfMonth") ? (bool)unboundValues["RunOnLastDayOfMonth"] : false);
			}
			set
			{
				if (v2Trigger != null)
					((V2Interop.IMonthlyTrigger)v2Trigger).RunOnLastDayOfMonth = value;
				else if (v1Trigger != null)
					throw new NotV1SupportedException();
				else
					unboundValues["RunOnLastDayOfMonth"] = value;
			}
		}

		/// <summary>
		/// Gets or sets a value that indicates the amount of time before the task is started.
		/// </summary>
		/// <value>The delay duration.</value>
		TimeSpan ITriggerDelay.Delay
		{
			get { return this.RandomDelay; }
			set { this.RandomDelay = value; }
		}

		/// <summary>
		/// Copies the properties from another <see cref="Trigger"/> the current instance. This will not copy any properties associated with any derived triggers except those supporting the <see cref="ITriggerDelay"/> interface.
		/// </summary>
		/// <param name="sourceTrigger">The source <see cref="Trigger"/>.</param>
		public override void CopyProperties(Trigger sourceTrigger)
		{
			base.CopyProperties(sourceTrigger);
			if (sourceTrigger.GetType() == this.GetType())
			{
				this.DaysOfMonth = ((MonthlyTrigger)sourceTrigger).DaysOfMonth;
				this.MonthsOfYear = ((MonthlyTrigger)sourceTrigger).MonthsOfYear;
				try { this.RunOnLastDayOfMonth = ((MonthlyTrigger)sourceTrigger).RunOnLastDayOfMonth; } catch { }
			}
		}

		/// <summary>
		/// Indicates whether the current object is equal to another object of the same type.
		/// </summary>
		/// <param name="other">An object to compare with this object.</param>
		/// <returns>
		/// true if the current object is equal to the <paramref name="other" /> parameter; otherwise, false.
		/// </returns>
		public override bool Equals(Trigger other)
		{
			return base.Equals(other) && this.DaysOfMonth == ((MonthlyTrigger)other).DaysOfMonth && this.MonthsOfYear == ((MonthlyTrigger)other).MonthsOfYear &&
				(this.v1Trigger == null && this.RunOnLastDayOfMonth == ((MonthlyTrigger)other).RunOnLastDayOfMonth);
		}

		/// <summary>
		/// Converts an array of bit indices into a mask with bits  turned ON at every index
		/// contained in the array.  Indices must be from 1 to 32 and bits are numbered the same.
		/// </summary>
		/// <param name="indices">An array with an element for each bit of the mask which is ON.</param>
		/// <returns>An interger to be interpreted as a mask.</returns>
		internal static int IndicesToMask(int[] indices)
		{
			int mask = 0;
			foreach (int index in indices)
			{
				if (index < 1 || index > 31) throw new ArgumentException("Days must be in the range 1..31");
				mask = mask | 1 << (index - 1);
			}
			return mask;
		}

		/// <summary>
		/// Convert an integer representing a mask to an array where each element contains the index
		/// of a bit that is ON in the mask.  Bits are considered to number from 1 to 32.
		/// </summary>
		/// <param name="mask">An interger to be interpreted as a mask.</param>
		/// <returns>An array with an element for each bit of the mask which is ON.</returns>
		internal static int[] MaskToIndices(int mask)
		{
			//count bits in mask
			int cnt = 0;
			for (int i = 0; (mask >> i) > 0; i++)
				cnt = cnt + (1 & (mask >> i));
			//allocate return array with one entry for each bit
			int[] indices = new int[cnt];
			//fill array with bit indices
			cnt = 0;
			for (int i = 0; (mask >> i) > 0; i++)
				if ((1 & (mask >> i)) == 1)
					indices[cnt++] = i + 1;
			return indices;
		}

		/// <summary>
		/// Reads the subclass XML for V1 streams.
		/// </summary>
		/// <param name="reader">The reader.</param>
		void ReadMyXml(System.Xml.XmlReader reader)
		{
			reader.ReadStartElement("ScheduleByMonth");
			while (reader.MoveToContent() == System.Xml.XmlNodeType.Element)
			{
				switch (reader.LocalName)
				{
					case "DaysOfMonth":
						reader.Read();
						List<int> days = new List<int>();
						while (reader.MoveToContent() == System.Xml.XmlNodeType.Element)
						{
							if (reader.LocalName == "Day")
							{
								string sday = reader.ReadElementContentAsString();
								if (!sday.Equals("Last", StringComparison.InvariantCultureIgnoreCase))
								{
									int day = Int32.Parse(sday);
									if (day >= 1 && day <= 31)
										days.Add(day);
								}
							}
						}
						this.DaysOfMonth = days.ToArray();
						reader.ReadEndElement();
						break;
					case "Months":
						reader.Read();
						this.MonthsOfYear = 0;
						while (reader.MoveToContent() == System.Xml.XmlNodeType.Element)
						{
							try
							{
								this.MonthsOfYear |= (MonthsOfTheYear)Enum.Parse(typeof(MonthsOfTheYear), reader.LocalName);
							}
							catch
							{
								throw new System.Xml.XmlException("Invalid months of the year element.");
							}
							reader.Read();
						}
						reader.ReadEndElement();
						break;
					default:
						reader.Skip();
						break;
				}
			}
			reader.ReadEndElement();
		}

		/// <summary>
		/// Gets the non-localized trigger string for V2 triggers.
		/// </summary>
		/// <returns>String describing the trigger.</returns>
		protected override string V2GetTriggerString()
		{
			string days = string.Join(Properties.Resources.ListSeparator, Array.ConvertAll(this.DaysOfMonth, delegate(int i) { return i.ToString(); }));
			string months = TaskEnumGlobalizer.GetString(this.MonthsOfYear);
			return string.Format(Properties.Resources.TriggerMonthly1, AdjustToLocal(this.StartBoundary), days, months);
		}

		void WriteMyXml(System.Xml.XmlWriter writer)
		{
			writer.WriteStartElement("ScheduleByMonth");

			writer.WriteStartElement("DaysOfMonth");
			foreach (var day in this.DaysOfMonth)
				writer.WriteElementString("Day", day.ToString());
			writer.WriteEndElement();

			writer.WriteStartElement("Months");
			foreach (MonthsOfTheYear e in Enum.GetValues(typeof(MonthsOfTheYear)))
				if (e != MonthsOfTheYear.AllMonths && (this.MonthsOfYear & e) == e)
					writer.WriteElementString(e.ToString(), null);
			writer.WriteEndElement();

			writer.WriteEndElement();
		}

		System.Xml.Schema.XmlSchema IXmlSerializable.GetSchema()
		{
			return null;
		}

		void IXmlSerializable.ReadXml(System.Xml.XmlReader reader)
		{
			CalendarTrigger.ReadXml(reader, this, this.ReadMyXml);
		}

		void IXmlSerializable.WriteXml(System.Xml.XmlWriter writer)
		{
			CalendarTrigger.WriteXml(writer, this, this.WriteMyXml);
		}
	}

	/// <summary>
	/// Represents a trigger that starts a task when the task is registered or updated. Not available on Task Scheduler 1.0.
	/// </summary>
	[XmlType(IncludeInSchema = false)]
	public sealed class RegistrationTrigger : Trigger, ITriggerDelay
	{
		/// <summary>
		/// Creates an unbound instance of a <see cref="RegistrationTrigger"/>.
		/// </summary>
		public RegistrationTrigger()
			: base(TaskTriggerType.Registration)
		{
		}

		internal RegistrationTrigger(V2Interop.ITrigger iTrigger)
			: base(iTrigger)
		{
		}

		/// <summary>
		/// Gets or sets a value that indicates the amount of time between when the system is booted and when the task is started.
		/// </summary>
		/// <exception cref="NotV1SupportedException">Not supported under Task Scheduler 1.0.</exception>
		[DefaultValue(typeof(TimeSpan), "00:00:00")]
		[XmlIgnore]
		public TimeSpan Delay
		{
			get
			{
				if (v2Trigger != null)
					return Task.StringToTimeSpan(((V2Interop.IRegistrationTrigger)v2Trigger).Delay);
				if (v1Trigger != null)
					throw new NotV1SupportedException();
				return (unboundValues.ContainsKey("Delay") ? (TimeSpan)unboundValues["Delay"] : TimeSpan.Zero);
			}
			set
			{
				if (v2Trigger != null)
					((V2Interop.IRegistrationTrigger)v2Trigger).Delay = Task.TimeSpanToString(value);
				else if (v1Trigger != null)
					throw new NotV1SupportedException();
				else
					unboundValues["Delay"] = value;
			}
		}

		/// <summary>
		/// Gets the non-localized trigger string for V2 triggers.
		/// </summary>
		/// <returns>String describing the trigger.</returns>
		protected override string V2GetTriggerString()
		{
			return Properties.Resources.TriggerRegistration1;
		}
	}

	/// <summary>
	/// Defines how often the task is run and how long the repetition pattern is repeated after the task is started.
	/// </summary>
	[XmlRoot("Repetition", Namespace = TaskDefinition.tns, IsNullable = true)]
	public sealed class RepetitionPattern : IDisposable, IXmlSerializable, IEquatable<RepetitionPattern>
	{
		private Trigger pTrigger;
		private V2Interop.IRepetitionPattern v2Pattern = null;

		internal RepetitionPattern(Trigger parent)
		{
			pTrigger = parent;
			if (pTrigger != null && pTrigger.v2Trigger != null)
				v2Pattern = pTrigger.v2Trigger.Repetition;
		}

		/// <summary>
		/// Gets or sets how long the pattern is repeated.
		/// </summary>
		/// <value>
		/// The duration that the pattern is repeated. The minimum time allowed is one minute. If <c>TimeSpan.Zero</c> is specified, the pattern is repeated indefinitely.
		/// </value>
		/// <remarks>If you specify a repetition duration for a task, you must also specify the repetition interval.</remarks>
		[DefaultValue(typeof(TimeSpan), "00:00:00")]
		public TimeSpan Duration
		{
			get
			{
				if (v2Pattern != null)
					return Task.StringToTimeSpan(v2Pattern.Duration);
				if (pTrigger != null)
					return TimeSpan.FromMinutes(pTrigger.v1TriggerData.MinutesDuration);
				return TimeSpan.Zero;
			}
			set
			{
				if (v2Pattern != null)
				{
					if (value != TimeSpan.Zero && value < TimeSpan.FromMinutes(1))
						throw new ArgumentOutOfRangeException();
					v2Pattern.Duration = Task.TimeSpanToString(value);
				}
				else if (pTrigger != null)
				{
					pTrigger.v1TriggerData.MinutesDuration = (uint)value.TotalMinutes;
					Bind();
				}
			}
		}

		/// <summary>
		/// Gets or sets the amount of time between each restart of the task.
		/// </summary>
		/// <value>
		/// The amount of time between each restart of the task. The maximum time allowed is 31 days, and the minimum time allowed is 1 minute.
		/// </value>
		/// <remarks>If you specify a repetition duration for a task, you must also specify the repetition interval.</remarks>
		/// <exception cref="System.ArgumentOutOfRangeException">The maximum time allowed is 31 days, and the minimum time allowed is 1 minute.</exception>
		[DefaultValue(typeof(TimeSpan), "00:00:00")]
		public TimeSpan Interval
		{
			get
			{
				if (v2Pattern != null)
					return Task.StringToTimeSpan(v2Pattern.Interval);
				if (pTrigger != null)
					return TimeSpan.FromMinutes(pTrigger.v1TriggerData.MinutesInterval);
				return TimeSpan.Zero;
			}
			set
			{
				if (v2Pattern != null)
				{
					if (value != TimeSpan.Zero && (value < TimeSpan.FromMinutes(1) || value > TimeSpan.FromDays(31)))
						throw new ArgumentOutOfRangeException();
					v2Pattern.Interval = Task.TimeSpanToString(value);
				}
				else if (pTrigger != null)
				{
					pTrigger.v1TriggerData.MinutesInterval = (uint)value.TotalMinutes;
					Bind();
				}
			}
		}

		/// <summary>
		/// Gets or sets a Boolean value that indicates if a running instance of the task is stopped at the end of repetition pattern duration.
		/// </summary>
		public bool StopAtDurationEnd
		{
			get
			{
				if (v2Pattern != null)
					return v2Pattern.StopAtDurationEnd;
				if (pTrigger != null)
					return (pTrigger.v1TriggerData.Flags & V1Interop.TaskTriggerFlags.KillAtDurationEnd) == V1Interop.TaskTriggerFlags.KillAtDurationEnd;
				return false;
			}
			set
			{
				if (v2Pattern != null)
					v2Pattern.StopAtDurationEnd = value;
				else if (pTrigger != null)
				{
					if (value)
						pTrigger.v1TriggerData.Flags |= V1Interop.TaskTriggerFlags.KillAtDurationEnd;
					else
						pTrigger.v1TriggerData.Flags &= ~V1Interop.TaskTriggerFlags.KillAtDurationEnd;
					Bind();
				}
			}
		}

		/// <summary>
		/// Releases all resources used by this class.
		/// </summary>
		public void Dispose()
		{
			if (v2Pattern != null) Marshal.ReleaseComObject(v2Pattern);
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
			if (obj is RepetitionPattern)
				return Equals((RepetitionPattern)obj);
			return base.Equals(obj);
		}

		/// <summary>
		/// Indicates whether the current object is equal to another object of the same type.
		/// </summary>
		/// <param name="other">An object to compare with this object.</param>
		/// <returns>
		/// true if the current object is equal to the <paramref name="other" /> parameter; otherwise, false.
		/// </returns>
		public bool Equals(RepetitionPattern other)
		{
			return this.Duration == other.Duration && this.Interval == other.Interval && this.StopAtDurationEnd == other.StopAtDurationEnd;
		}

		/// <summary>
		/// Returns a hash code for this instance.
		/// </summary>
		/// <returns>
		/// A hash code for this instance, suitable for use in hashing algorithms and data structures like a hash table. 
		/// </returns>
		public override int GetHashCode()
		{
			return new { A = this.Duration, B = this.Interval, C = this.StopAtDurationEnd }.GetHashCode();
		}

		/// <summary>
		/// Determines whether any properties for this <see cref="RepetitionPattern"/> have been set.
		/// </summary>
		/// <returns>
		///   <c>true</c> if properties have been set; otherwise, <c>false</c>.
		/// </returns>
		public bool IsSet()
		{
			if (v2Pattern != null)
				return v2Pattern.StopAtDurationEnd || !string.IsNullOrEmpty(v2Pattern.Duration) || !string.IsNullOrEmpty(v2Pattern.Interval);
			else if (pTrigger != null)
				return (pTrigger.v1TriggerData.Flags & V1Interop.TaskTriggerFlags.KillAtDurationEnd) == V1Interop.TaskTriggerFlags.KillAtDurationEnd ||
					pTrigger.v1TriggerData.MinutesDuration > 0 || pTrigger.v1TriggerData.MinutesInterval > 0;
			return false;
		}

		internal void Bind()
		{
			if (pTrigger.v1Trigger != null)
				pTrigger.SetV1TriggerData();
			else if (pTrigger.v2Trigger != null)
			{
				if (pTrigger.v1TriggerData.MinutesInterval != 0)
					v2Pattern.Interval = string.Format("PT{0}M", pTrigger.v1TriggerData.MinutesInterval);
				if (pTrigger.v1TriggerData.MinutesDuration != 0)
					v2Pattern.Duration = string.Format("PT{0}M", pTrigger.v1TriggerData.MinutesDuration);
				v2Pattern.StopAtDurationEnd = (pTrigger.v1TriggerData.Flags & V1Interop.TaskTriggerFlags.KillAtDurationEnd) == V1Interop.TaskTriggerFlags.KillAtDurationEnd;
			}
		}

		System.Xml.Schema.XmlSchema IXmlSerializable.GetSchema()
		{
			return null;
		}

		bool ReadXmlConverter(System.Reflection.PropertyInfo pi, Object obj, ref Object value)
		{
			if (pi.Name == "Interval")
			{
				if (value is TimeSpan && !((TimeSpan)value).Equals(TimeSpan.Zero) && this.Duration <= (TimeSpan)value)
					this.Duration = ((TimeSpan)value).Add(TimeSpan.FromMinutes(1));
			}
			return false;
		}

		void IXmlSerializable.ReadXml(System.Xml.XmlReader reader)
		{
			if (!reader.IsEmptyElement)
			{
				reader.ReadStartElement("Repetition", TaskDefinition.tns);
				XmlSerializationHelper.ReadObjectProperties(reader, this, this.ReadXmlConverter);
				reader.ReadEndElement();
			}
			else
				reader.Skip();
		}

		void IXmlSerializable.WriteXml(System.Xml.XmlWriter writer)
		{
			XmlSerializationHelper.WriteObjectProperties(writer, this);
		}
	}

	/// <summary>
	/// Triggers tasks for console connect or disconnect, remote connect or disconnect, or workstation lock or unlock notifications.
	/// </summary>
	[XmlType(IncludeInSchema = false)]
	public sealed class SessionStateChangeTrigger : Trigger, ITriggerDelay, ITriggerUserId
	{
		/// <summary>
		/// Creates an unbound instance of a <see cref="SessionStateChangeTrigger"/>.
		/// </summary>
		public SessionStateChangeTrigger()
			: base(TaskTriggerType.SessionStateChange)
		{
		}

		/// <summary>
		/// Initializes a new instance of the <see cref="SessionStateChangeTrigger"/> class.
		/// </summary>
		/// <param name="stateChange">The state change.</param>
		public SessionStateChangeTrigger(TaskSessionStateChangeType stateChange) : this()
		{
			this.StateChange = stateChange;
		}

		internal SessionStateChangeTrigger(V2Interop.ITrigger iTrigger)
			: base(iTrigger)
		{
		}

		/// <summary>
		/// Gets or sets a value that indicates the amount of time between when the system is booted and when the task is started.
		/// </summary>
		[DefaultValue(typeof(TimeSpan), "00:00:00")]
		public TimeSpan Delay
		{
			get
			{
				if (v2Trigger != null)
					return Task.StringToTimeSpan(((V2Interop.ISessionStateChangeTrigger)v2Trigger).Delay);
				return (unboundValues.ContainsKey("Delay") ? (TimeSpan)unboundValues["Delay"] : TimeSpan.Zero);
			}
			set
			{
				if (v2Trigger != null)
					((V2Interop.ISessionStateChangeTrigger)v2Trigger).Delay = Task.TimeSpanToString(value);
				else
					unboundValues["Delay"] = value;
			}
		}

		/// <summary>
		/// Gets or sets the kind of Terminal Server session change that would trigger a task launch.
		/// </summary>
		[DefaultValue(1)]
		public TaskSessionStateChangeType StateChange
		{
			get
			{
				if (v2Trigger != null)
					return ((V2Interop.ISessionStateChangeTrigger)v2Trigger).StateChange;
				return (unboundValues.ContainsKey("StateChange") ? (TaskSessionStateChangeType)unboundValues["StateChange"] : TaskSessionStateChangeType.ConsoleConnect);
			}
			set
			{
				if (v2Trigger != null)
					((V2Interop.ISessionStateChangeTrigger)v2Trigger).StateChange = value;
				else
					unboundValues["StateChange"] = value;
			}
		}

		/// <summary>
		/// Gets or sets the user for the Terminal Server session. When a session state change is detected for this user, a task is started.
		/// </summary>
		[DefaultValue((string)null)]
		public string UserId
		{
			get
			{
				if (v2Trigger != null)
					return ((V2Interop.ISessionStateChangeTrigger)v2Trigger).UserId;
				return (unboundValues.ContainsKey("UserId") ? (string)unboundValues["UserId"] : null);
			}
			set
			{
				if (v2Trigger != null)
					((V2Interop.ISessionStateChangeTrigger)v2Trigger).UserId = value;
				else
					unboundValues["UserId"] = value;
			}
		}

		/// <summary>
		/// Copies the properties from another <see cref="Trigger"/> the current instance. This will not copy any properties associated with any derived triggers except those supporting the <see cref="ITriggerDelay"/> interface.
		/// </summary>
		/// <param name="sourceTrigger">The source <see cref="Trigger"/>.</param>
		public override void CopyProperties(Trigger sourceTrigger)
		{
			base.CopyProperties(sourceTrigger);
			if (sourceTrigger.GetType() == this.GetType() && !this.StateChangeIsSet())
				this.StateChange = ((SessionStateChangeTrigger)sourceTrigger).StateChange;
		}

		/// <summary>
		/// Indicates whether the current object is equal to another object of the same type.
		/// </summary>
		/// <param name="other">An object to compare with this object.</param>
		/// <returns>
		/// true if the current object is equal to the <paramref name="other" /> parameter; otherwise, false.
		/// </returns>
		public override bool Equals(Trigger other)
		{
			return base.Equals(other) && this.StateChange == ((SessionStateChangeTrigger)other).StateChange;
		}

		/// <summary>
		/// Gets the non-localized trigger string for V2 triggers.
		/// </summary>
		/// <returns>String describing the trigger.</returns>
		protected override string V2GetTriggerString()
		{
			string str = Properties.Resources.ResourceManager.GetString("TriggerSession" + this.StateChange.ToString());
			string user = string.IsNullOrEmpty(this.UserId) ? Properties.Resources.TriggerAnyUser : this.UserId;
			if (this.StateChange != TaskSessionStateChangeType.SessionLock && this.StateChange != TaskSessionStateChangeType.SessionUnlock)
				user = string.Format(Properties.Resources.TriggerSessionUserSession, user);
			return string.Format(str, user);
		}

		/// <summary>
		/// Returns a value indicating if the StateChange property has been set.
		/// </summary>
		/// <returns>StateChange property has been set.</returns>
		internal bool StateChangeIsSet()
		{
			return (v2Trigger != null || unboundValues.ContainsKey("StateChange"));
		}
	}

	/// <summary>
	/// Represents a trigger that starts a task at a specific date and time.
	/// </summary>
	public sealed class TimeTrigger : Trigger, ITriggerDelay
	{
		/// <summary>
		/// Creates an unbound instance of a <see cref="TimeTrigger"/>.
		/// </summary>
		public TimeTrigger()
			: base(TaskTriggerType.Time)
		{
		}

		/// <summary>
		/// Creates an unbound instance of a <see cref="TimeTrigger"/> and assigns the execution time.
		/// </summary>
		/// <param name="startBoundary">Date and time for the trigger to fire.</param>
		public TimeTrigger(DateTime startBoundary)
			: base(TaskTriggerType.Time)
		{
			this.StartBoundary = startBoundary;
		}

		internal TimeTrigger(V1Interop.ITaskTrigger iTrigger)
			: base(iTrigger, V1Interop.TaskTriggerType.RunOnce)
		{
		}

		internal TimeTrigger(V2Interop.ITrigger iTrigger)
			: base(iTrigger)
		{
		}

		/// <summary>
		/// Gets or sets a delay time that is randomly added to the start time of the trigger.
		/// </summary>
		/// <exception cref="NotV1SupportedException">Not supported under Task Scheduler 1.0.</exception>
		[DefaultValue(typeof(TimeSpan), "00:00:00")]
		[XmlIgnore]
		public TimeSpan RandomDelay
		{
			get
			{
				if (v2Trigger != null)
					return Task.StringToTimeSpan(((V2Interop.ITimeTrigger)v2Trigger).RandomDelay);
				if (v1Trigger != null)
					throw new NotV1SupportedException();
				return (unboundValues.ContainsKey("RandomDelay") ? (TimeSpan)unboundValues["RandomDelay"] : TimeSpan.Zero);
			}
			set
			{
				if (v2Trigger != null)
					((V2Interop.ITimeTrigger)v2Trigger).RandomDelay = Task.TimeSpanToString(value);
				else if (v1Trigger != null)
					throw new NotV1SupportedException();
				else
					unboundValues["RandomDelay"] = value;
			}
		}

		/// <summary>
		/// Gets or sets a value that indicates the amount of time before the task is started.
		/// </summary>
		/// <value>The delay duration.</value>
		TimeSpan ITriggerDelay.Delay
		{
			get { return this.RandomDelay; }
			set { this.RandomDelay = value; }
		}

		/// <summary>
		/// Gets the non-localized trigger string for V2 triggers.
		/// </summary>
		/// <returns>String describing the trigger.</returns>
		protected override string V2GetTriggerString()
		{
			return string.Format(Properties.Resources.TriggerTime1, AdjustToLocal(this.StartBoundary));
		}
	}

	/// <summary>
	/// Represents a trigger that starts a task based on a weekly schedule. For example, the task starts at 8:00 A.M. on a specific day of the week every week or every other week.
	/// </summary>
	[XmlRoot("CalendarTrigger", Namespace = TaskDefinition.tns, IsNullable = false)]
	public sealed class WeeklyTrigger : Trigger, ICalendarTrigger, ITriggerDelay, IXmlSerializable
	{
		/// <summary>
		/// Creates an unbound instance of a <see cref="WeeklyTrigger"/>.
		/// </summary>
		/// <param name="daysOfWeek">The days of the week.</param>
		/// <param name="weeksInterval">The interval between the weeks in the schedule.</param>
		public WeeklyTrigger(DaysOfTheWeek daysOfWeek = DaysOfTheWeek.Sunday, short weeksInterval = 1)
			: base(TaskTriggerType.Weekly)
		{
			this.DaysOfWeek = daysOfWeek;
			this.WeeksInterval = weeksInterval;
		}

		internal WeeklyTrigger(V1Interop.ITaskTrigger iTrigger)
			: base(iTrigger, V1Interop.TaskTriggerType.RunWeekly)
		{
			if (v1TriggerData.Data.weekly.DaysOfTheWeek == 0)
				v1TriggerData.Data.weekly.DaysOfTheWeek = DaysOfTheWeek.Sunday;
			if (v1TriggerData.Data.weekly.WeeksInterval == 0)
				v1TriggerData.Data.weekly.WeeksInterval = 1;
		}

		internal WeeklyTrigger(V2Interop.ITrigger iTrigger)
			: base(iTrigger)
		{
		}

		/// <summary>
		/// Gets or sets the days of the week on which the task runs.
		/// </summary>
		[DefaultValue(0)]
		public DaysOfTheWeek DaysOfWeek
		{
			get
			{
				if (v2Trigger != null)
					return (DaysOfTheWeek)((V2Interop.IWeeklyTrigger)v2Trigger).DaysOfWeek;
				return (DaysOfTheWeek)v1TriggerData.Data.weekly.DaysOfTheWeek;
			}
			set
			{
				if (v2Trigger != null)
					((V2Interop.IWeeklyTrigger)v2Trigger).DaysOfWeek = (short)value;
				else
				{
					v1TriggerData.Data.weekly.DaysOfTheWeek = value;
					if (v1Trigger != null)
						SetV1TriggerData();
					else
						unboundValues["DaysOfWeek"] = (short)value;
				}
			}
		}

		/// <summary>
		/// Gets or sets a delay time that is randomly added to the start time of the trigger.
		/// </summary>
		/// <exception cref="NotV1SupportedException">Not supported under Task Scheduler 1.0.</exception>
		[DefaultValue(typeof(TimeSpan), "00:00:00")]
		[XmlIgnore]
		public TimeSpan RandomDelay
		{
			get
			{
				if (v2Trigger != null)
					return Task.StringToTimeSpan(((V2Interop.IWeeklyTrigger)v2Trigger).RandomDelay);
				if (v1Trigger != null)
					throw new NotV1SupportedException();
				return (unboundValues.ContainsKey("RandomDelay") ? (TimeSpan)unboundValues["RandomDelay"] : TimeSpan.Zero);
			}
			set
			{
				if (v2Trigger != null)
					((V2Interop.IWeeklyTrigger)v2Trigger).RandomDelay = Task.TimeSpanToString(value);
				else if (v1Trigger != null)
					throw new NotV1SupportedException();
				else
					unboundValues["RandomDelay"] = value;
			}
		}

		/// <summary>
		/// Gets or sets the interval between the weeks in the schedule.
		/// </summary>
		[DefaultValue(1)]
		public short WeeksInterval
		{
			get
			{
				if (v2Trigger != null)
					return ((V2Interop.IWeeklyTrigger)v2Trigger).WeeksInterval;
				return (short)v1TriggerData.Data.weekly.WeeksInterval;
			}
			set
			{
				if (v2Trigger != null)
					((V2Interop.IWeeklyTrigger)v2Trigger).WeeksInterval = value;
				else
				{
					v1TriggerData.Data.weekly.WeeksInterval = (ushort)value;
					if (v1Trigger != null)
						SetV1TriggerData();
					else
						unboundValues["WeeksInterval"] = value;
				}
			}
		}

		/// <summary>
		/// Gets or sets a value that indicates the amount of time before the task is started.
		/// </summary>
		/// <value>The delay duration.</value>
		TimeSpan ITriggerDelay.Delay
		{
			get { return this.RandomDelay; }
			set { this.RandomDelay = value; }
		}

		/// <summary>
		/// Copies the properties from another <see cref="Trigger"/> the current instance. This will not copy any properties associated with any derived triggers except those supporting the <see cref="ITriggerDelay"/> interface.
		/// </summary>
		/// <param name="sourceTrigger">The source <see cref="Trigger"/>.</param>
		public override void CopyProperties(Trigger sourceTrigger)
		{
			base.CopyProperties(sourceTrigger);
			if (sourceTrigger.GetType() == this.GetType())
			{
				this.DaysOfWeek = ((WeeklyTrigger)sourceTrigger).DaysOfWeek;
				this.WeeksInterval = ((WeeklyTrigger)sourceTrigger).WeeksInterval;
			}
		}

		/// <summary>
		/// Indicates whether the current object is equal to another object of the same type.
		/// </summary>
		/// <param name="other">An object to compare with this object.</param>
		/// <returns>
		/// true if the current object is equal to the <paramref name="other" /> parameter; otherwise, false.
		/// </returns>
		public override bool Equals(Trigger other)
		{
			return base.Equals(other) && this.DaysOfWeek == ((WeeklyTrigger)other).DaysOfWeek && this.WeeksInterval == ((WeeklyTrigger)other).WeeksInterval;
		}

		/// <summary>
		/// Reads the subclass XML for V1 streams.
		/// </summary>
		/// <param name="reader">The reader.</param>
		void ReadMyXml(System.Xml.XmlReader reader)
		{
			reader.ReadStartElement("ScheduleByWeek");
			while (reader.MoveToContent() == System.Xml.XmlNodeType.Element)
			{
				switch (reader.LocalName)
				{
					case "WeeksInterval":
						this.WeeksInterval = (short)reader.ReadElementContentAsInt();
						break;
					case "DaysOfWeek":
						reader.Read();
						this.DaysOfWeek = 0;
						while (reader.MoveToContent() == System.Xml.XmlNodeType.Element)
						{
							try
							{
								this.DaysOfWeek |= (DaysOfTheWeek)Enum.Parse(typeof(DaysOfTheWeek), reader.LocalName);
							}
							catch
							{
								throw new System.Xml.XmlException("Invalid days of the week element.");
							}
							reader.Read();
						}
						reader.ReadEndElement();
						break;
					default:
						reader.Skip();
						break;
				}
			}
			reader.ReadEndElement();
		}

		/// <summary>
		/// Gets the non-localized trigger string for V2 triggers.
		/// </summary>
		/// <returns>String describing the trigger.</returns>
		protected override string V2GetTriggerString()
		{
			string days = TaskEnumGlobalizer.GetString(this.DaysOfWeek);
			return string.Format(this.WeeksInterval == 1 ? Properties.Resources.TriggerWeekly1Week : Properties.Resources.TriggerWeeklyMultWeeks,
				AdjustToLocal(this.StartBoundary), days, this.WeeksInterval);
		}

		/// <summary>
		/// Writes the subclass XML for V1 streams.
		/// </summary>
		/// <param name="writer">The writer.</param>
		void WriteMyXml(System.Xml.XmlWriter writer)
		{
			writer.WriteStartElement("ScheduleByWeek");

			if (this.WeeksInterval != 1)
				writer.WriteElementString("WeeksInterval", this.WeeksInterval.ToString());

			writer.WriteStartElement("DaysOfWeek");
			foreach (DaysOfTheWeek e in Enum.GetValues(typeof(DaysOfTheWeek)))
				if (e != DaysOfTheWeek.AllDays && (this.DaysOfWeek & e) == e)
					writer.WriteElementString(e.ToString(), null);
			writer.WriteEndElement();

			writer.WriteEndElement();
		}

		System.Xml.Schema.XmlSchema IXmlSerializable.GetSchema()
		{
			return null;
		}

		void IXmlSerializable.ReadXml(System.Xml.XmlReader reader)
		{
			CalendarTrigger.ReadXml(reader, this, this.ReadMyXml);
		}

		void IXmlSerializable.WriteXml(System.Xml.XmlWriter writer)
		{
			CalendarTrigger.WriteXml(writer, this, this.WriteMyXml);
		}
	}
}