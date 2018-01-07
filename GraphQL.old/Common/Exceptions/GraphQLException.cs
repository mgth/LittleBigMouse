using System;
using GraphQL.Common.Response;

namespace GraphQL.Common.Exceptions {

	/// <summary>
	/// An exception that contains a <see cref="Response.GraphQLError"/>
	/// </summary>
	public class GraphQLException : Exception {

		/// <summary>
		/// The GraphQLError
		/// </summary>
		public GraphQLError GraphQLError { get; }

		/// <summary>
		/// Constructor for a GraphQLException
		/// </summary>
		/// <param name="graphQLError">The GraphQL Error</param>
		public GraphQLException(GraphQLError graphQLError):base(graphQLError.Message) {
			this.GraphQLError = graphQLError;
		}

	}

}
