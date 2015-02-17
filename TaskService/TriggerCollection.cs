using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Xml.Serialization;

namespace Microsoft.Win32.TaskScheduler
{
	/// <summary>
	/// Provides the methods that are used to add to, remove from, and get the triggers of a task.
	/// </summary>
	[XmlRoot("Triggers", Namespace = TaskDefinition.tns, IsNullable = false)]
	public sealed class TriggerCollection : IList<Trigger>, IDisposable, IXmlSerializable
	{
		private V1Interop.ITask v1Task = null;
		private V2Interop.ITaskDefinition v2Def = null;
		private V2Interop.ITriggerCollection v2Coll = null;

		internal TriggerCollection(V1Interop.ITask iTask)
		{
			v1Task = iTask;
		}

		internal TriggerCollection(V2Interop.ITaskDefinition iTaskDef)
		{
			v2Def = iTaskDef;
			v2Coll = v2Def.Triggers;
		}

		/// <summary>
		/// Releases all resources used by this class.
		/// </summary>
		public void Dispose()
		{
			if (v2Coll != null) Marshal.ReleaseComObject(v2Coll);
			v2Def = null;
			v1Task = null;
		}

		/// <summary>
		/// Gets the collection enumerator for this collection.
		/// </summary>
		/// <returns>The <see cref="IEnumerator{T}"/> for this collection.</returns>
		public IEnumerator<Trigger> GetEnumerator()
		{
			if (v1Task != null)
				return new V1TriggerEnumerator(v1Task);
			return new V2TriggerEnumerator(v2Coll, v2Def);
		}

		System.Collections.IEnumerator System.Collections.IEnumerable.GetEnumerator()
		{
			return this.GetEnumerator();
		}

		internal sealed class V1TriggerEnumerator : IEnumerator<Trigger>
		{
			private V1Interop.ITask iTask;
			private short curItem = -1;

			internal V1TriggerEnumerator(V1Interop.ITask task)
			{
				iTask = task;
			}

			public Trigger Current
			{
				get
				{
					return Trigger.CreateTrigger(iTask.GetTrigger((ushort)curItem));
				}
			}

			/// <summary>
			/// Releases all resources used by this class.
			/// </summary>
			public void Dispose()
			{
				iTask = null;
			}

			object System.Collections.IEnumerator.Current
			{
				get { return this.Current; }
			}

			public bool MoveNext()
			{
				if (++curItem >= iTask.GetTriggerCount())
					return false;
				return true;
			}

			public void Reset()
			{
				curItem = -1;
			}
		}

		internal sealed class V2TriggerEnumerator : IEnumerator<Trigger>
		{
			private System.Collections.IEnumerator iEnum;
			private V2Interop.ITaskDefinition v2Def = null;

			internal V2TriggerEnumerator(V2Interop.ITriggerCollection iColl, V2Interop.ITaskDefinition iDef)
			{
				iEnum = iColl.GetEnumerator();
				v2Def = iDef;
			}

			#region IEnumerator<Trigger> Members

			public Trigger Current
			{
				get
				{
					return Trigger.CreateTrigger((V2Interop.ITrigger)iEnum.Current, v2Def);
				}
			}

			#endregion

			#region IDisposable Members

			/// <summary>
			/// Releases all resources used by this class.
			/// </summary>
			public void Dispose()
			{
				iEnum = null;
			}

			#endregion

			#region IEnumerator Members

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

			#endregion
		}

		/// <summary>
		/// Gets the number of triggers in the collection.
		/// </summary>
		public int Count
		{
			get
			{
				if (v2Coll != null)
					return v2Coll.Count;
				return (int)v1Task.GetTriggerCount();
			}
		}

		/// <summary>
		/// Add an unbound <see cref="Trigger"/> to the task.
		/// </summary>
		/// <param name="unboundTrigger"><see cref="Trigger"/> derivative to add to the task.</param>
		/// <returns>Bound trigger.</returns>
		/// <exception cref="System.ArgumentNullException"><c>unboundTrigger</c> is <c>null</c>.</exception>
		public Trigger Add(Trigger unboundTrigger)
		{
			if (unboundTrigger == null)
				throw new ArgumentNullException("unboundTrigger");
			if (v2Def != null)
				unboundTrigger.Bind(v2Def);
			else
				unboundTrigger.Bind(v1Task);
			return unboundTrigger;
		}

		/// <summary>
		/// Add a new trigger to the collections of triggers for the task.
		/// </summary>
		/// <param name="taskTriggerType">The type of trigger to create.</param>
		/// <returns>A <see cref="Trigger"/> instance of the specified type.</returns>
		public Trigger AddNew(TaskTriggerType taskTriggerType)
		{
			if (v1Task != null)
			{
				ushort idx;
				return Trigger.CreateTrigger(v1Task.CreateTrigger(out idx), Trigger.ConvertToV1TriggerType(taskTriggerType));
			}

			return Trigger.CreateTrigger(v2Coll.Create(taskTriggerType));
		}

		/// <summary>
		/// Adds a collection of unbound triggers to the end of the <see cref="TriggerCollection"/>.
		/// </summary>
		/// <param name="triggers">The triggers to be added to the end of the <see cref="TriggerCollection"/>. The collection itself cannot be <c>null</c> and cannot contain <c>null</c> elements.</param>
		/// <exception cref="System.ArgumentNullException"><c>triggers</c> is <c>null</c>.</exception>
		public void AddRange(IEnumerable<Trigger> triggers)
		{
			if (triggers == null)
				throw new ArgumentNullException("triggers");
			foreach (var item in triggers)
				this.Add(item);
		}

		internal void Bind()
		{
			foreach (Trigger t in this)
				t.SetV1TriggerData();
		}

		/// <summary>
		/// Clears all triggers from the task.
		/// </summary>
		public void Clear()
		{
			if (v2Coll != null)
				v2Coll.Clear();
			else
			{
				for (int i = this.Count - 1; i >= 0; i--)
					RemoveAt(i);
			}
		}

		/// <summary>
		/// Determines whether the <see cref="T:System.Collections.Generic.ICollection`1" /> contains a specific value.
		/// </summary>
		/// <param name="item">The object to locate in the <see cref="T:System.Collections.Generic.ICollection`1" />.</param>
		/// <returns>
		/// true if <paramref name="item" /> is found in the <see cref="T:System.Collections.Generic.ICollection`1" />; otherwise, false.
		/// </returns>
		public bool Contains(Trigger item)
		{
			return IndexOf(item) >= 0;
		}

		/// <summary>
		/// Determines whether the specified trigger type is contained in this collection.
		/// </summary>
		/// <param name="triggerType">Type of the trigger.</param>
		/// <returns>
		///   <c>true</c> if the specified trigger type is contained in this collection; otherwise, <c>false</c>.
		/// </returns>
		public bool ContainsType(Type triggerType)
		{
			foreach (Trigger t in this)
				if (t.GetType() == triggerType)
					return true;
			return false;
		}

		/// <summary>
		/// Copies the elements of the <see cref="T:System.Collections.Generic.ICollection`1" /> to an <see cref="Array"/>, starting at a particular <see cref="Array"/> index.
		/// </summary>
		/// <param name="array">The one-dimensional <see cref="Array"/> that is the destination of the elements copied from <see cref="T:System.Collections.Generic.ICollection`1" />. The <see cref="Array"/> must have zero-based indexing.</param>
		/// <param name="arrayIndex">The zero-based index in <paramref name="array"/> at which copying begins.</param>
		/// <exception cref="System.ArgumentNullException"><paramref name="array"/> is null.</exception>
		/// <exception cref="System.ArgumentOutOfRangeException"><paramref name="arrayIndex"/> is less than 0.</exception>
		/// <exception cref="System.ArgumentException">The number of elements in the source <see cref="T:System.Collections.Generic.ICollection`1" /> is greater than the available space from <paramref name="arrayIndex"/> to the end of the destination <paramref name="array"/>.</exception>
		public void CopyTo(Trigger[] array, int arrayIndex)
		{
			if (array == null)
				throw new ArgumentNullException();
			if (arrayIndex < 0)
				throw new ArgumentOutOfRangeException();
			if (this.Count > (array.Length - arrayIndex))
				throw new ArgumentException();
			for (int i = 0; i < this.Count; i++)
				array[arrayIndex + i] = (Trigger)this[i].Clone();
		}

		/// <summary>
		/// Gets a specified trigger from the collection.
		/// </summary>
		/// <param name="index">The index of the trigger to be retrieved.</param>
		/// <returns>Specialized <see cref="Trigger"/> instance.</returns>
		public Trigger this[int index]
		{
			get
			{
				if (v2Coll != null)
					return Trigger.CreateTrigger(v2Coll[++index], this.v2Def);
				return Trigger.CreateTrigger(v1Task.GetTrigger((ushort)index));
			}
			set
			{
				if (this.Count <= index)
					throw new ArgumentOutOfRangeException("index", index, "Index is not a valid index in the TriggerCollection");
				RemoveAt(index);
				Insert(index, value);
			}
		}

		/// <summary>
		/// Gets or sets a specified trigger from the collection.
		/// </summary>
		/// <value>
		/// The <see cref="Trigger"/>.
		/// </value>
		/// <param name="triggerId">The id (<see cref="Trigger.Id" />) of the trigger to be retrieved.</param>
		/// <returns>
		/// Specialized <see cref="Trigger" /> instance.
		/// </returns>
		/// <exception cref="System.ArgumentNullException"></exception>
		/// <exception cref="System.ArgumentOutOfRangeException"></exception>
		/// <exception cref="System.NullReferenceException"></exception>
		/// <exception cref="System.InvalidOperationException">Mismatching Id for trigger and lookup.</exception>
		public Trigger this[string triggerId]
		{
			get
			{
				if (string.IsNullOrEmpty(triggerId))
					throw new ArgumentNullException(triggerId);
				foreach (Trigger t in this)
					if (string.Equals(t.Id, triggerId))
						return t;
				throw new ArgumentOutOfRangeException(triggerId);
			}
			set
			{
				if (value == null)
					throw new NullReferenceException();
				if (string.IsNullOrEmpty(triggerId))
					throw new ArgumentNullException(triggerId);
				if (triggerId != value.Id)
					throw new InvalidOperationException("Mismatching Id for trigger and lookup.");
				int index = IndexOf(triggerId);
				if (index >= 0)
				{
					RemoveAt(index);
					Insert(index, value);
				}
				else
					Add(value);
			}
		}

		/// <summary>
		/// Determines the index of a specific item in the <see cref="T:System.Collections.Generic.IList`1" />.
		/// </summary>
		/// <param name="item">The object to locate in the <see cref="T:System.Collections.Generic.IList`1" />.</param>
		/// <returns>
		/// The index of <paramref name="item" /> if found in the list; otherwise, -1.
		/// </returns>
		public int IndexOf(Trigger item)
		{
			for (int i = 0; i < this.Count; i++)
			{
				if (this[i].Equals(item))
					return i;
			}
			return -1;
		}

		/// <summary>
		/// Determines the index of a specific item in the <see cref="T:System.Collections.Generic.IList`1" />.
		/// </summary>
		/// <param name="triggerId">The id (<see cref="Trigger.Id"/>) of the trigger to be retrieved.</param>
		/// <returns>
		/// The index of <paramref name="triggerId" /> if found in the list; otherwise, -1.
		/// </returns>
		public int IndexOf(string triggerId)
		{
			if (string.IsNullOrEmpty(triggerId))
				throw new ArgumentNullException(triggerId);
			for (int i = 0; i < this.Count; i++)
			{
				if (string.Equals(this[i].Id, triggerId))
					return i;
			}
			return -1;
		}

		/// <summary>
		/// Inserts an trigger at the specified index.
		/// </summary>
		/// <param name="index">The zero-based index at which trigger should be inserted.</param>
		/// <param name="trigger">The trigger to insert into the list.</param>
		public void Insert(int index, Trigger trigger)
		{
			Trigger[] pushItems = new Trigger[this.Count - index];
			for (int i = index; i < this.Count; i++)
				pushItems[i - index] = (Trigger)this[i].Clone();
			for (int j = this.Count - 1; j >= index; j--)
				RemoveAt(j);
			Add(trigger);
			for (int k = 0; k < pushItems.Length; k++)
				Add(pushItems[k]);
		}

		/// <summary>
		/// Removes the first occurrence of a specific object from the <see cref="T:System.Collections.Generic.ICollection`1" />.
		/// </summary>
		/// <param name="item">The object to remove from the <see cref="T:System.Collections.Generic.ICollection`1" />.</param>
		/// <returns>
		/// true if <paramref name="item" /> was successfully removed from the <see cref="T:System.Collections.Generic.ICollection`1" />; otherwise, false. This method also returns false if <paramref name="item" /> is not found in the original <see cref="T:System.Collections.Generic.ICollection`1" />.
		/// </returns>
		public bool Remove(Trigger item)
		{
			int idx = IndexOf(item);
			if (idx != -1)
			{
				try
				{
					RemoveAt(idx);
					return true;
				}
				catch { }
			}
			return false;
		}

		/// <summary>
		/// Removes the trigger at a specified index.
		/// </summary>
		/// <param name="index">Index of trigger to remove.</param>
		/// <exception cref="ArgumentOutOfRangeException">Index out of range.</exception>
		public void RemoveAt(int index)
		{
			if (index >= this.Count)
				throw new ArgumentOutOfRangeException("index", index, "Failed to remove Trigger. Index out of range.");
			if (v2Coll != null)
				v2Coll.Remove(++index);
			else
				v1Task.DeleteTrigger((ushort)index); //Remove the trigger from the Task Scheduler
		}

		/// <summary>
		/// Returns a <see cref="System.String"/> that represents the triggers in this collection.
		/// </summary>
		/// <returns>
		/// A <see cref="System.String"/> that represents the triggers in this collection.
		/// </returns>
		public override string ToString()
		{
			if (this.Count == 1)
				return this[0].ToString();
			if (this.Count > 1)
				return Properties.Resources.MultipleTriggers;
			return string.Empty;
		}

		System.Xml.Schema.XmlSchema IXmlSerializable.GetSchema()
		{
			return null;
		}

		void IXmlSerializable.ReadXml(System.Xml.XmlReader reader)
		{
			reader.ReadStartElement("Triggers", TaskDefinition.tns);
			while (reader.MoveToContent() == System.Xml.XmlNodeType.Element)
			{
				switch (reader.LocalName)
				{
					case "BootTrigger":
						XmlSerializationHelper.ReadObject(reader, this.AddNew(TaskTriggerType.Boot));
						break;
					case "IdleTrigger":
						XmlSerializationHelper.ReadObject(reader, this.AddNew(TaskTriggerType.Idle));
						break;
					case "TimeTrigger":
						XmlSerializationHelper.ReadObject(reader, this.AddNew(TaskTriggerType.Time));
						break;
					case "LogonTrigger":
						XmlSerializationHelper.ReadObject(reader, this.AddNew(TaskTriggerType.Logon));
						break;
					case "CalendarTrigger":
						this.Add(CalendarTrigger.GetTriggerFromXml(reader));
						break;
					default:
						reader.Skip();
						break;
				}
			}
			reader.ReadEndElement();
		}

		void IXmlSerializable.WriteXml(System.Xml.XmlWriter writer)
		{
			foreach (var t in this)
				XmlSerializationHelper.WriteObject(writer, t);
		}

		void ICollection<Trigger>.Add(Trigger item)
		{
			this.Add(item);
		}

		bool ICollection<Trigger>.IsReadOnly
		{
			get { return false; }
		}
	}
}
