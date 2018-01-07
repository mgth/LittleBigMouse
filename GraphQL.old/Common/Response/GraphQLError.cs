namespace GraphQL.Common.Response {

	/// <summary>
	/// Represents the error of a <see cref="GraphQLResponse"/>
	/// </summary>
	public class GraphQLError {

		/// <summary>
		/// The error message
		/// </summary>
		public string Message { get; set; }

		/// <summary>
		/// The Location of an error
		/// </summary>
		public GraphQLLocation[] Locations { get; set; }

	}

}
