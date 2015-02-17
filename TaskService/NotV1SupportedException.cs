using System;
using System.Diagnostics;
using System.Reflection;

namespace Microsoft.Win32.TaskScheduler
{
	/// <summary>
	/// Abstract class for throwing a method specific exception.
	/// </summary>
	[System.Diagnostics.DebuggerStepThrough]
	public abstract class TSNotSupportedException : Exception
	{
		/// <summary>Defines the minimum supported version for the action not allowed by this exception.</summary>
		protected TaskCompatibility min;
		private string myMessage;

		internal TSNotSupportedException(TaskCompatibility minComp)
		{
			min = minComp;
			StackTrace stackTrace = new StackTrace();
			StackFrame stackFrame = stackTrace.GetFrame(2);
			MethodBase methodBase = stackFrame.GetMethod();
			myMessage = string.Format("{0}.{1} is not supported on {2}", methodBase.DeclaringType.Name, methodBase.Name, this.LibName);
		}

		internal TSNotSupportedException(string message, TaskCompatibility minComp)
		{
			myMessage = message;
			min = minComp;
		}

		/// <summary>
		/// Gets a message that describes the current exception.
		/// </summary>
		public override string Message
		{
			get { return myMessage; }
		}

		/// <summary>
		/// Gets the minimum supported TaskScheduler version required for this method or property.
		/// </summary>
		public TaskCompatibility MinimumSupportedVersion { get { return min;  } }

		internal abstract string LibName { get; }
	}

	/// <summary>
	/// Thrown when the calling method is not supported by Task Scheduler 1.0.
	/// </summary>
	[System.Diagnostics.DebuggerStepThrough]
	public class NotV1SupportedException : TSNotSupportedException
	{
		internal NotV1SupportedException() : base(TaskCompatibility.V2) { }
		internal NotV1SupportedException(string message) : base(message, TaskCompatibility.V2) { }
		internal override string LibName { get { return "Task Scheduler 1.0"; } }
	}

	/// <summary>
	/// Thrown when the calling method is not supported by Task Scheduler 2.0.
	/// </summary>
	[System.Diagnostics.DebuggerStepThrough]
	public class NotV2SupportedException : TSNotSupportedException
	{
		internal NotV2SupportedException() : base(TaskCompatibility.V1) { }
		internal NotV2SupportedException(string message) : base(message, TaskCompatibility.V1) { }
		internal override string LibName { get { return "Task Scheduler 2.0 (1.2)"; } }
	}


	/// <summary>
	/// Thrown when the calling method is not supported by Task Scheduler versions prior to the one specified.
	/// </summary>
	[System.Diagnostics.DebuggerStepThrough]
	public class NotSupportedPriorToException : TSNotSupportedException
	{
		internal NotSupportedPriorToException(TaskCompatibility supportedVersion) : base(supportedVersion) { }
		internal override string LibName { get { return string.Format("Task Scheduler versions prior to 2.{0} (1.{1})", ((int)min) - 2, (int)min); } }
	}

}
