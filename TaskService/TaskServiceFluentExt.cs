using System;

namespace Microsoft.Win32.TaskScheduler
{
	public sealed partial class TaskService
	{
		/// <summary>
		/// Initial call for a Fluent model of creating a task.
		/// </summary>
		/// <param name="path">The path of the program to run.</param>
		/// <returns>An <see cref="Fluent.ActionBuilder"/> instance.</returns>
		public Fluent.ActionBuilder Execute(string path)
		{
			return new Fluent.ActionBuilder(new Fluent.BuilderInfo(this), path);
		}
	}

	namespace Fluent
	{
		/// <summary>
		/// Fluent helper class. Not intended for use.
		/// </summary>
		internal sealed class BuilderInfo
		{
			public TaskService ts;
			public TaskDefinition td;

			public BuilderInfo(TaskService taskSvc)
			{
				ts = taskSvc;
				td = ts.NewTask();
			}
		}

		/// <summary>
		/// Fluent helper class. Not intended for use.
		/// </summary>
		public abstract class BaseBuilder
		{
			internal BuilderInfo tb;

			internal BaseBuilder(BuilderInfo taskBuilder)
			{
				tb = taskBuilder;
			}

			internal TaskDefinition TaskDef { get { return tb.td; } }
		}

		/// <summary>
		/// Fluent helper class. Not intended for use.
		/// </summary>
		public class ActionBuilder : BaseBuilder
		{
			internal ActionBuilder(BuilderInfo taskBuilder, string path)
				: base(taskBuilder)
			{
				TaskDef.Actions.Add(new ExecAction(path));
			}

			/// <summary>
			/// Adds arguments to the <see cref="ExecAction"/>.
			/// </summary>
			/// <param name="args">The arguments.</param>
			/// <returns><see cref="ActionBuilder"/> instance.</returns>
			public ActionBuilder WithArguments(string args)
			{
				((ExecAction)TaskDef.Actions[0]).Arguments = args;
				return this;
			}

			/// <summary>
			/// Adds a working directory to the <see cref="ExecAction" />.
			/// </summary>
			/// <param name="dir">The directory.</param>
			/// <returns><see cref="ActionBuilder" /> instance.</returns>
			public ActionBuilder InWorkingDirectory(string dir)
			{
				((ExecAction)TaskDef.Actions[0]).WorkingDirectory = dir;
				return this;
			}

			/// <summary>
			/// Adds a trigger that executes every day or week.
			/// </summary>
			/// <param name="num">The interval of days or weeks.</param>
			/// <returns><see cref="IntervalTriggerBuilder" /> instance.</returns>
			public IntervalTriggerBuilder Every(short num)
			{
				return new IntervalTriggerBuilder(tb, num);
			}

			/// <summary>
			/// Adds a trigger that executes monthly on certain days of the week.
			/// </summary>
			/// <param name="dow">The days of the week on which to run.</param>
			/// <returns><see cref="MonthlyDOWTriggerBuilder" /> instance.</returns>
			public MonthlyDOWTriggerBuilder OnAll(DaysOfTheWeek dow)
			{
				return new MonthlyDOWTriggerBuilder(tb, dow);
			}

			/// <summary>
			/// Adds a trigger that executes monthly on specific days.
			/// </summary>
			/// <param name="moy">The months of the year in which to run.</param>
			/// <returns><see cref="MonthlyTriggerBuilder" /> instance.</returns>
			public MonthlyTriggerBuilder InTheMonthOf(MonthsOfTheYear moy)
			{
				return new MonthlyTriggerBuilder(tb, moy);
			}

			/// <summary>
			/// Adds a trigger that executes once at a specific time.
			/// </summary>
			/// <returns><see cref="TriggerBuilder" /> instance.</returns>
			public TriggerBuilder Once()
			{
				return new TriggerBuilder(tb, TaskTriggerType.Time);
			}

			/// <summary>
			/// Adds a trigger that executes at system startup.
			/// </summary>
			/// <returns><see cref="TriggerBuilder" /> instance.</returns>
			public TriggerBuilder OnBoot()
			{
				return new TriggerBuilder(tb, TaskTriggerType.Boot);
			}

			/// <summary>
			/// Adds a trigger that executes when system is idle.
			/// </summary>
			/// <returns><see cref="TriggerBuilder" /> instance.</returns>
			public TriggerBuilder OnIdle()
			{
				return new TriggerBuilder(tb, TaskTriggerType.Idle);
			}

			/// <summary>
			/// Adds a trigger that executes once at specified state change.
			/// </summary>
			/// <param name="changeType">Type of the change.</param>
			/// <returns>
			///   <see cref="TriggerBuilder" /> instance.
			/// </returns>
			public TriggerBuilder OnStateChange(TaskSessionStateChangeType changeType)
			{
				var b = new TriggerBuilder(tb, TaskTriggerType.SessionStateChange);
				((SessionStateChangeTrigger)b.trigger).StateChange = changeType;
				return b;
			}

			/// <summary>
			/// Adds a trigger that executes at logon of all users.
			/// </summary>
			/// <returns><see cref="TriggerBuilder" /> instance.</returns>
			public TriggerBuilder AtLogon()
			{
				return new TriggerBuilder(tb, TaskTriggerType.Logon);
			}

			/// <summary>
			/// Adds a trigger that executes at logon of a specific user.
			/// </summary>
			/// <param name="userId">The user id.</param>
			/// <returns><see cref="TriggerBuilder" /> instance.</returns>
			public TriggerBuilder AtLogonOf(string userId)
			{
				var b = new TriggerBuilder(tb, TaskTriggerType.Logon);
				((LogonTrigger)b.trigger).UserId = userId;
				return b;
			}

			/// <summary>
			/// Adds a trigger that executes at task registration.
			/// </summary>
			/// <returns><see cref="TriggerBuilder" /> instance.</returns>
			public TriggerBuilder AtTaskRegistration()
			{
				return new TriggerBuilder(tb, TaskTriggerType.Registration);
			}
		}

		/// <summary>
		/// Fluent helper class. Not intended for use.
		/// </summary>
		public class MonthlyTriggerBuilder : BaseBuilder
		{
			private TriggerBuilder trb;

			internal MonthlyTriggerBuilder(BuilderInfo taskBuilder, MonthsOfTheYear moy)
				: base(taskBuilder)
			{
				this.trb = new TriggerBuilder(taskBuilder, moy);
			}

			/// <summary>
			/// Updates a monthly trigger to specify the days of the month on which it will run.
			/// </summary>
			/// <param name="days">The days.</param>
			/// <returns>
			///   <see cref="TriggerBuilder" /> instance.
			/// </returns>
			public TriggerBuilder OnTheDays(params int[] days)
			{
				((MonthlyTrigger)trb.trigger).DaysOfMonth = days;
				return trb;
			}
		}

		/// <summary>
		/// Fluent helper class. Not intended for use.
		/// </summary>
		public class MonthlyDOWTriggerBuilder : BaseBuilder
		{
			private TriggerBuilder trb;

			internal MonthlyDOWTriggerBuilder(BuilderInfo taskBuilder, DaysOfTheWeek dow)
				: base(taskBuilder)
			{
				this.trb = new TriggerBuilder(taskBuilder, dow);
			}

			/// <summary>
			/// Updates a monthly trigger to specify in which weeks of the month it will run.
			/// </summary>
			/// <param name="ww">The week.</param>
			/// <returns>
			///   <see cref="MonthlyDOWTriggerBuilder" /> instance.
			/// </returns>
			public MonthlyDOWTriggerBuilder In(WhichWeek ww)
			{
				((MonthlyDOWTrigger)trb.trigger).WeeksOfMonth = ww;
				return this;
			}

			/// <summary>
			/// Updates a monthly trigger to specify the months of the year in which it will run.
			/// </summary>
			/// <param name="moy">The month of the year.</param>
			/// <returns>
			///   <see cref="TriggerBuilder" /> instance.
			/// </returns>
			public TriggerBuilder Of(MonthsOfTheYear moy)
			{
				((MonthlyDOWTrigger)trb.trigger).MonthsOfYear = moy;
				return trb;
			}
		}

		/// <summary>
		/// Fluent helper class. Not intended for use.
		/// </summary>
		public class WeeklyTriggerBuilder : TriggerBuilder
		{
			internal WeeklyTriggerBuilder(BuilderInfo taskBuilder, short interval)
				: base(taskBuilder)
			{
				TaskDef.Triggers.Add(trigger = new WeeklyTrigger() { WeeksInterval = interval });
			}

			/// <summary>
			/// Updates a weekly trigger to specify the days of the week on which it will run.
			/// </summary>
			/// <param name="dow">The days of the week.</param>
			/// <returns>
			///   <see cref="TriggerBuilder" /> instance.
			/// </returns>
			public TriggerBuilder On(DaysOfTheWeek dow)
			{
				((WeeklyTrigger)trigger).DaysOfWeek = dow;
				return this as TriggerBuilder;
			}
		}

		/// <summary>
		/// Fluent helper class. Not intended for use.
		/// </summary>
		public class IntervalTriggerBuilder : BaseBuilder
		{
			internal short interval = 0;

			internal IntervalTriggerBuilder(BuilderInfo taskBuilder, short interval)
				: base(taskBuilder)
			{
				this.interval = interval;
			}

			/// <summary>
			/// Specifies that an Every target uses days as the interval.
			/// </summary>
			/// <returns><see cref="TriggerBuilder" /> instance.</returns>
			public TriggerBuilder Days()
			{
				return new TriggerBuilder(tb) { trigger = TaskDef.Triggers.Add(new DailyTrigger(this.interval)) };
			}

			/// <summary>
			/// Specifies that an Every target uses weeks as the interval.
			/// </summary>
			/// <returns><see cref="WeeklyTriggerBuilder" /> instance.</returns>
			public WeeklyTriggerBuilder Weeks()
			{
				return new WeeklyTriggerBuilder(tb, interval);
			}
		}

		/// <summary>
		/// Fluent helper class. Not intended for use.
		/// </summary>
		public class TriggerBuilder : BaseBuilder
		{
			internal Trigger trigger;

			internal TriggerBuilder(BuilderInfo taskBuilder)
				: base(taskBuilder)
			{
			}

			internal TriggerBuilder(BuilderInfo taskBuilder, DaysOfTheWeek dow)
				: this(taskBuilder)
			{
				TaskDef.Triggers.Add(trigger = new MonthlyDOWTrigger(dow));
			}

			internal TriggerBuilder(BuilderInfo taskBuilder, MonthsOfTheYear moy)
				: this(taskBuilder)
			{
				TaskDef.Triggers.Add(trigger = new MonthlyTrigger() { MonthsOfYear = moy });
			}

			internal TriggerBuilder(BuilderInfo taskBuilder, TaskTriggerType taskTriggerType)
				: this(taskBuilder)
			{
				TaskDef.Triggers.Add(trigger = Trigger.CreateTrigger(taskTriggerType));
			}

			/// <summary>
			/// Specifies a date on which a trigger will start.
			/// </summary>
			/// <param name="year">The year.</param>
			/// <param name="month">The month.</param>
			/// <param name="day">The day.</param>
			/// <returns>
			///   <see cref="TriggerBuilder" /> instance.
			/// </returns>
			public TriggerBuilder Starting(int year, int month, int day)
			{
				trigger.StartBoundary = new DateTime(year, month, day, trigger.StartBoundary.Hour, trigger.StartBoundary.Minute, trigger.StartBoundary.Second);
				return this;
			}

			/// <summary>
			/// Specifies a date and time on which a trigger will start.
			/// </summary>
			/// <param name="year">The year.</param>
			/// <param name="month">The month.</param>
			/// <param name="day">The day.</param>
			/// <param name="hour">The hour.</param>
			/// <param name="min">The min.</param>
			/// <param name="sec">The sec.</param>
			/// <returns>
			///   <see cref="TriggerBuilder" /> instance.
			/// </returns>
			public TriggerBuilder Starting(int year, int month, int day, int hour, int min, int sec)
			{
				trigger.StartBoundary = new DateTime(year, month, day, hour, min, sec);
				return this;
			}

			/// <summary>
			/// Specifies a date and time on which a trigger will start.
			/// </summary>
			/// <param name="dt">A string representing a DateTime and parsable via <see cref="DateTime.Parse(string)"/>.</param>
			/// <returns>
			///   <see cref="TriggerBuilder" /> instance.
			/// </returns>
			public TriggerBuilder Starting(string dt)
			{
				trigger.StartBoundary = DateTime.Parse(dt);
				return this;
			}

			/// <summary>
			/// Specifies a date and time on which a trigger will start.
			/// </summary>
			/// <param name="dt">The DateTime value.</param>
			/// <returns>
			///   <see cref="TriggerBuilder" /> instance.
			/// </returns>
			public TriggerBuilder Starting(DateTime dt)
			{
				trigger.StartBoundary = dt;
				return this;
			}

			/// <summary>
			/// Specifies a date on which a trigger will no longer run.
			/// </summary>
			/// <param name="year">The year.</param>
			/// <param name="month">The month.</param>
			/// <param name="day">The day.</param>
			/// <returns>
			///   <see cref="TriggerBuilder" /> instance.
			/// </returns>
			public TriggerBuilder Ending(int year, int month, int day)
			{
				trigger.EndBoundary = new DateTime(year, month, day, trigger.StartBoundary.Hour, trigger.StartBoundary.Minute, trigger.StartBoundary.Second);
				return this;
			}

			/// <summary>
			/// Specifies a date and time on which a trigger will no longer run.
			/// </summary>
			/// <param name="year">The year.</param>
			/// <param name="month">The month.</param>
			/// <param name="day">The day.</param>
			/// <param name="hour">The hour.</param>
			/// <param name="min">The min.</param>
			/// <param name="sec">The sec.</param>
			/// <returns>
			///   <see cref="TriggerBuilder" /> instance.
			/// </returns>
			public TriggerBuilder Ending(int year, int month, int day, int hour, int min, int sec)
			{
				trigger.EndBoundary = new DateTime(year, month, day, hour, min, sec);
				return this;
			}

			/// <summary>
			/// Specifies a date and time on which a trigger will no longer run.
			/// </summary>
			/// <param name="dt">A string representing a DateTime and parsable via <see cref="DateTime.Parse(string)"/>.</param>
			/// <returns>
			///   <see cref="TriggerBuilder" /> instance.
			/// </returns>
			public TriggerBuilder Ending(string dt)
			{
				trigger.EndBoundary = DateTime.Parse(dt);
				return this;
			}

			/// <summary>
			/// Specifies a date and time on which a trigger will no longer run.
			/// </summary>
			/// <param name="dt">The DateTime value.</param>
			/// <returns>
			///   <see cref="TriggerBuilder" /> instance.
			/// </returns>
			public TriggerBuilder Ending(DateTime dt)
			{
				trigger.EndBoundary = dt;
				return this;
			}

			/// <summary>
			/// Specifies a repetion interval for the trigger.
			/// </summary>
			/// <param name="span">The interval span.</param>
			/// <returns><see cref="TriggerBuilder" /> instance.</returns>
			public TriggerBuilder RepeatingEvery(TimeSpan span)
			{
				trigger.Repetition.Interval = span;
				return this;
			}

			/// <summary>
			/// Specifies a repetion interval for the trigger.
			/// </summary>
			/// <param name="span">The interval span string. Must be parsable by <see cref="TimeSpan.Parse(string)"/>.</param>
			/// <returns><see cref="TriggerBuilder" /> instance.</returns>
			public TriggerBuilder RepeatingEvery(string span)
			{
				trigger.Repetition.Interval = TimeSpan.Parse(span);
				return this;
			}

			/// <summary>
			/// Specifies the maximum amount of time to repeat the execution of a trigger.
			/// </summary>
			/// <param name="span">The duration span.</param>
			/// <returns><see cref="TriggerBuilder" /> instance.</returns>
			public TriggerBuilder RunningAtMostFor(TimeSpan span)
			{
				trigger.Repetition.Duration = span;
				return this;
			}

			/// <summary>
			/// Specifies the maximum amount of time to repeat the execution of a trigger.
			/// </summary>
			/// <param name="span">The duration span string. Must be parsable by <see cref="TimeSpan.Parse(string)"/>.</param>
			/// <returns><see cref="TriggerBuilder" /> instance.</returns>
			public TriggerBuilder RunningAtMostFor(string span)
			{
				trigger.Repetition.Duration = TimeSpan.Parse(span);
				return this;
			}

			/// <summary>
			/// Assigns the name of the task and registers it.
			/// </summary>
			/// <param name="name">The name.</param>
			/// <returns>A registerd <see cref="Task"/> instance.</returns>
			public Task AsTask(string name)
			{
				return tb.ts.RootFolder.RegisterTaskDefinition(name, TaskDef);
			}
		}
	}
}
