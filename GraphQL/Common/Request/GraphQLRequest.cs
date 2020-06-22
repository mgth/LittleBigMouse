namespace GraphQL.Common.Request {

	/// <summary>
	/// Represents a Query that can be fetched to a GraphQL Server.
	/// For more information <see href="http://graphql.org/learn/serving-over-http/#post-request"/>
	/// </summary>
	public class GraphQLRequest {

		/// <summary>
		/// The Query
		/// </summary>
		public string query { get; set; }

		/// <summary>
		/// If the provided <see cref="Query"/> contains multiple named operations, this specifies which operation should be executed.
		/// </summary>
		public string operationName { get; set; }

		/// <summary>
		/// The Variables
		/// </summary>
		public dynamic variables { get; set; }

	}
}
