using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text.RegularExpressions;

namespace Microsoft.Win32.TaskScheduler
{
	/// <summary>
	/// Contains all the tasks that are registered.
	/// </summary>
	/// <remarks>Potentially breaking change in 1.6.2 and later where under V1 the list previously included the '.job' extension on the task name. This has been removed so that it is consistent with V2.</remarks>
	public sealed class TaskCollection : IEnumerable<Task>, IDisposable
	{
		private TaskService svc;
		private Regex filter;
		private TaskFolder fld;
		private V1Interop.ITaskScheduler v1TS = null;
		private V2Interop.IRegisteredTaskCollection v2Coll = null;

		internal TaskCollection(TaskService svc, Regex filter = null)
		{
			this.svc = svc;
			this.Filter = filter;
			v1TS = svc.v1TaskScheduler;
		}

		internal TaskCollection(TaskFolder folder, V2Interop.IRegisteredTaskCollection iTaskColl, Regex filter = null)
		{
			this.svc = folder.TaskService;
			this.Filter = filter;
			fld = folder;
			v2Coll = iTaskColl;
		}

		/// <summary>
		/// Releases all resources used by this class.
		/// </summary>
		public void Dispose()
		{
			v1TS = null;
			if (v2Coll != null)
				Marshal.ReleaseComObject(v2Coll);
		}

		/// <summary>
		/// Gets the collection enumerator for the register task collection.
		/// </summary>
		/// <returns>An <see cref="System.Collections.IEnumerator"/> for this collection.</returns>
		public IEnumerator<Task> GetEnumerator()
		{
			if (v1TS != null)
				return new V1TaskEnumerator(svc, filter);
			return new V2TaskEnumerator(fld, v2Coll, filter);
		}

		System.Collections.IEnumerator System.Collections.IEnumerable.GetEnumerator()
		{
			return this.GetEnumerator();
		}

		internal class V1TaskEnumerator : IEnumerator<Task>, IDisposable
		{
			private TaskService svc;
			private V1Interop.IEnumWorkItems wienum = null;
			private V1Interop.ITaskScheduler m_ts = null;
			private string curItem = null;
			private Regex filter;

			/// <summary>
			/// Internal constructor
			/// </summary>
			/// <param name="svc">TaskService instance</param>
			/// <param name="filter">The filter.</param>
			internal V1TaskEnumerator(TaskService svc, Regex filter = null)
			{
				this.svc = svc;
				this.filter = filter;
				m_ts = svc.v1TaskScheduler;
				wienum = m_ts.Enum();
				Reset();
			}

			/// <summary>
			/// Retrieves the current task.  See <see cref="System.Collections.IEnumerator.Current"/> for more information.
			/// </summary>
			public Microsoft.Win32.TaskScheduler.Task Current
			{
				get { return new Task(svc, this.ICurrent); }
			}

			internal V1Interop.ITask ICurrent
			{
				get { return TaskService.GetTask(m_ts, curItem); }
			}

			/// <summary>
			/// Releases all resources used by this class.
			/// </summary>
			public void Dispose()
			{
				if (wienum != null) Marshal.ReleaseComObject(wienum);
				m_ts = null;
			}

			object System.Collections.IEnumerator.Current
			{
				get { return this.Current; }
			}

			/// <summary>
			/// Moves to the next task. See MoveNext for more information.
			/// </summary>
			/// <returns>true if next task found, false if no more tasks.</returns>
			public bool MoveNext()
			{
				IntPtr names = IntPtr.Zero;
				bool valid = false;
				do
				{
					curItem = null;
					uint uFetched = 0;
					try
					{
						wienum.Next(1, out names, out uFetched);
						if (uFetched != 1)
							break;
						using (V1Interop.CoTaskMemString name = new V1Interop.CoTaskMemString(Marshal.ReadIntPtr(names)))
							curItem = name.ToString();
						if (curItem.EndsWith(".job", StringComparison.InvariantCultureIgnoreCase))
							curItem = curItem.Remove(curItem.Length - 4);
					}
					catch { }
					finally { Marshal.FreeCoTaskMem(names); names = IntPtr.Zero; }

					// If name doesn't match filter, look for next item
					if (filter != null)
					{
						if (!filter.IsMatch(curItem))
							continue;
					}

					V1Interop.ITask itask = null;
					try { itask = this.ICurrent; valid = true; }
					catch { valid = false; }
					finally { itask = null; }
				} while (!valid);

				return (curItem != null);
			}

			/// <summary>
			/// Reset task enumeration. See Reset for more information.
			/// </summary>
			public void Reset()
			{
				curItem = null;
				wienum.Reset();
			}

			internal int Count
			{
				get
				{
					int i = 0;
					Reset();
					while (this.MoveNext())
						i++;
					Reset();
					return i;
				}
			}
		}

		internal class V2TaskEnumerator : IEnumerator<Task>, IDisposable
		{
			private System.Collections.IEnumerator iEnum;
			private TaskFolder fld;
			private Regex filter;

			internal V2TaskEnumerator(TaskFolder folder, TaskScheduler.V2Interop.IRegisteredTaskCollection iTaskColl, Regex filter = null)
			{
				this.fld = folder;
				this.iEnum = iTaskColl.GetEnumerator();
				this.filter = filter;
			}

			public Task Current
			{
				get { return Task.CreateTask(fld.TaskService, (TaskScheduler.V2Interop.IRegisteredTask)iEnum.Current); }
			}

			/// <summary>
			/// Releases all resources used by this class.
			/// </summary>
			public void Dispose()
			{
				iEnum = null;
			}

			object System.Collections.IEnumerator.Current
			{
				get { return this.Current; }
			}

			public bool MoveNext()
			{
				bool hasNext = iEnum.MoveNext();
				while (hasNext)
				{
					if (filter == null || filter.IsMatch(((TaskScheduler.V2Interop.IRegisteredTask)iEnum.Current).Name))
						break;
					hasNext = iEnum.MoveNext();
				}
				return hasNext;
			}

			public void Reset()
			{
				iEnum.Reset();
			}
		}

		/// <summary>
		/// Gets the number of registered tasks in the collection.
		/// </summary>
		public int Count
		{
			get
			{
				int i = 0;
				if (v2Coll != null)
				{
					V2TaskEnumerator v2te = new V2TaskEnumerator(this.fld, this.v2Coll, this.filter);
					while (v2te.MoveNext())
						i++;
				}
				else
				{
					V1TaskEnumerator v1te = new V1TaskEnumerator(this.svc, this.filter);
					return v1te.Count;
				}
				return i;
			}
		}

		/// <summary>
		/// Gets or sets the regular expression filter for task names.
		/// </summary>
		/// <value>The regular expression filter.</value>
		private Regex Filter
		{
			get
			{
				return filter;
			}
			set
			{
				string sfilter = value == null ? string.Empty : value.ToString().TrimStart('^').TrimEnd('$');
				if (sfilter == string.Empty || sfilter == "*")
					filter = null;
				else
				{
					if (value.ToString().TrimEnd('$').EndsWith("\\.job", StringComparison.InvariantCultureIgnoreCase))
						filter = new Regex(value.ToString().Replace("\\.job", ""));
					else
						filter = value;
				}
			}
		}

		/// <summary>
		/// Gets the specified registered task from the collection.
		/// </summary>
		/// <param name="index">The index of the registered task to be retrieved.</param>
		/// <returns>A <see cref="Task"/> instance that contains the requested context.</returns>
		public Task this[int index]
		{
			get
			{
				int i = 0;
				var te = GetEnumerator();
				while (te.MoveNext())
					if (i++ == index)
						return te.Current;
				throw new ArgumentOutOfRangeException();
			}
		}

		/// <summary>
		/// Gets the named registered task from the collection.
		/// </summary>
		/// <param name="name">The name of the registered task to be retrieved.</param>
		/// <returns>A <see cref="Task"/> instance that contains the requested context.</returns>
		public Task this[string name]
		{
			get
			{
				if (v2Coll != null)
					return Task.CreateTask(svc, v2Coll[name]);

				Task v1Task = svc.GetTask(name);
				if (v1Task != null)
					return v1Task;

				throw new ArgumentOutOfRangeException();
			}
		}

		/// <summary>
		/// Determines whether the specified task exists.
		/// </summary>
		/// <param name="taskName">The name of the task.</param>
		/// <returns>true if task exists; otherwise, false.</returns>
		public bool Exists(string taskName)
		{
			try
			{
				if (v2Coll != null)
					return v2Coll[taskName] != null;

				return svc.GetTask(taskName) != null;
			}
			catch { }
			return false;
		}
	}

	/// <summary>
	/// Collection of running tasks.
	/// </summary>
	public sealed class RunningTaskCollection : IEnumerable<RunningTask>, IDisposable
	{
		private TaskService svc;
		private V1Interop.ITaskScheduler v1TS = null;
		private V2Interop.ITaskService v2Svc = null;
		private V2Interop.IRunningTaskCollection v2Coll = null;

		internal RunningTaskCollection(TaskService svc)
		{
			this.svc = svc;
			v1TS = svc.v1TaskScheduler;
		}

		internal RunningTaskCollection(TaskService svc, V2Interop.IRunningTaskCollection iTaskColl)
		{
			this.svc = svc;
			v2Svc = svc.v2TaskService;
			v2Coll = iTaskColl;
		}

		/// <summary>
		/// Releases all resources used by this class.
		/// </summary>
		public void Dispose()
		{
			v1TS = null;
			v2Svc = null;
			if (v2Coll != null)
				Marshal.ReleaseComObject(v2Coll);
		}

		/// <summary>
		/// Gets an IEnumerator instance for this collection.
		/// </summary>
		/// <returns>An enumerator.</returns>
		public IEnumerator<RunningTask> GetEnumerator()
		{
			if (v2Coll != null)
				return new RunningTaskEnumerator(svc, v2Coll);
			return new V1RunningTaskEnumerator(svc);
		}

		System.Collections.IEnumerator System.Collections.IEnumerable.GetEnumerator()
		{
			return this.GetEnumerator();
		}

		internal class V1RunningTaskEnumerator : IEnumerator<RunningTask>
		{
			private TaskService svc;
			private TaskCollection.V1TaskEnumerator tEnum;

			internal V1RunningTaskEnumerator(TaskService svc)
			{
				this.svc = svc;
				tEnum = new TaskCollection.V1TaskEnumerator(svc);
			}

			public bool MoveNext()
			{
				if (tEnum.MoveNext())
				{
					if (tEnum.Current.State == TaskState.Running)
						return true;
					return this.MoveNext();
				}
				return false;
			}

			public RunningTask Current
			{
				get { return new RunningTask(svc, tEnum.ICurrent); }
			}

			/// <summary>
			/// Releases all resources used by this class.
			/// </summary>
			public void Dispose()
			{
				tEnum.Dispose();
			}

			object System.Collections.IEnumerator.Current
			{
				get { return this.Current; }
			}

			public void Reset()
			{
				tEnum.Reset();
			}
		}

		internal class RunningTaskEnumerator : IEnumerator<RunningTask>, IDisposable
		{
			private TaskService svc;
			private V2Interop.ITaskService v2Svc = null;
			private System.Collections.IEnumerator iEnum;

			internal RunningTaskEnumerator(TaskService svc, V2Interop.IRunningTaskCollection iTaskColl)
			{
				this.svc = svc;
				v2Svc = svc.v2TaskService;
				iEnum = iTaskColl.GetEnumerator();
			}

			public RunningTask Current
			{
				get
				{
					V2Interop.IRunningTask irt = (V2Interop.IRunningTask)iEnum.Current;
					V2Interop.IRegisteredTask task = null;
					try { task = TaskService.GetTask(v2Svc, irt.Path); } catch { }
					if (task == null) return null;
					return new RunningTask(svc, task, irt);
				}
			}

			/// <summary>
			/// Releases all resources used by this class.
			/// </summary>
			public void Dispose()
			{
				v2Svc = null;
				iEnum = null;
			}

			object System.Collections.IEnumerator.Current
			{
				get { return this.Current; }
			}

			public bool MoveNext()
			{
				return iEnum.MoveNext();
			}

			public void Reset()
			{
				iEnum.Reset();
			}
		}

		/// <summary>
		/// Gets the number of registered tasks in the collection.
		/// </summary>
		public int Count
		{
			get
			{
				if (v2Coll != null)
					return v2Coll.Count;
				int i = 0;
				V1RunningTaskEnumerator v1te = new V1RunningTaskEnumerator(svc);
				while (v1te.MoveNext())
					i++;
				return i;
			}
		}

		/// <summary>
		/// Gets the specified running task from the collection.
		/// </summary>
		/// <param name="index">The index of the running task to be retrieved.</param>
		/// <returns>A <see cref="RunningTask"/> instance.</returns>
		public RunningTask this[int index]
		{
			get
			{
				if (v2Coll != null)
				{
					V2Interop.IRunningTask irt = v2Coll[++index];
					return new RunningTask(svc, TaskService.GetTask(svc.v2TaskService, irt.Path), irt);
				}

				int i = 0;
				V1RunningTaskEnumerator v1te = new V1RunningTaskEnumerator(svc);
				while (v1te.MoveNext())
					if (i++ == index)
						return v1te.Current;
				throw new ArgumentOutOfRangeException();
			}
		}
	}

}
