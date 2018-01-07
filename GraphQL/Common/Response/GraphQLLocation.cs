namespace GraphQL.Common.Response {

	/// <summary>
	/// Represents the location where the <see cref="GraphQLError"/> has been found
	/// </summary>
	public class GraphQLLocation {

		/// <summary>
		/// The Column
		/// </summary>
		public uint Column { get; set; }

		/// <summary>
		/// The Line
		/// </summary>
		public uint Line { get; set; }

	}

}
