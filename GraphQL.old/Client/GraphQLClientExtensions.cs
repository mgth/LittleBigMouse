using System.IO;
using System.Threading.Tasks;
using GraphQL.Common.Request;
using GraphQL.Common.Response;

namespace GraphQL.Client {

	/// <summary>
	/// Extension Methods for <see cref="GraphQLClient"/>
	/// </summary>
	public static class GraphQLClientExtensions {

		private static readonly GraphQLRequest IntrospectionQuery = new GraphQLRequest {
			Query = @"
				query IntrospectionQuery {
					__schema {
						queryType {
							name
						},
						mutationType {
							name
						},
						subscriptionType {
							name
						},
						types {
							...FullType
						},
						directives {
							name,
							description,
							args {
								...InputValue
							},
							onOperation,
							onFragment,
							onField
						}
					}
				}

				fragment FullType on __Type {
					kind,
					name,
					description,
					fields(includeDeprecated: true) {
						name,
						description,
						args {
							...InputValue
						},
						type {
							...TypeRef
						},
						isDeprecated,
						deprecationReason
					},
					inputFields {
						...InputValue
					},
					interfaces {
						...TypeRef
					},
					enumValues(includeDeprecated: true) {
						name,
						description,
						isDeprecated,
						deprecationReason
					},
					possibleTypes {
						...TypeRef
					}
				}

				fragment InputValue on __InputValue {
					name,
					description,
					type {
						...TypeRef
					},
					defaultValue
				}

				fragment TypeRef on __Type {
					kind,
					name,
					ofType {
						kind,
						name,
						ofType {
							kind,
							name,
							ofType {
								kind,
								name
							}
						}
					}
				}".Replace("\t", "").Replace("\n", "").Replace("\r", ""),
			Variables = null
		};

		/// <summary>
		/// Send an IntrospectionQuery via GET
		/// </summary>
		/// <param name="graphQLClient">The GraphQLClient</param>
		/// <returns>The GraphQLResponse</returns>
		public static async Task<GraphQLResponse> GetIntrospectionQueryAsync(this GraphQLClient graphQLClient) =>
			await graphQLClient.GetAsync(IntrospectionQuery).ConfigureAwait(false);

		/// <summary>
		/// Send an IntrospectionQuery via POST
		/// </summary>
		/// <param name="graphQLClient">The GraphQLClient</param>
		/// <returns>The GraphQLResponse</returns>
		public static async Task<GraphQLResponse> PostIntrospectionQueryAsync(this GraphQLClient graphQLClient) =>
			await graphQLClient.PostAsync(IntrospectionQuery).ConfigureAwait(false);

		/// <summary>
		/// Send the Query defined in a file via GET
		/// </summary>
		/// <param name="graphQLClient">The GraphQLClient</param>
		/// <param name="filePath">The Path of the File</param>
		/// <returns>The GraphQLResponse</returns>
		public static async Task<GraphQLResponse> GetFromFile(this GraphQLClient graphQLClient, string filePath) =>
			await graphQLClient.GetQueryAsync(File.ReadAllText(filePath)).ConfigureAwait(false);

		/// <summary>
		/// Send the Query defined in a file via POST
		/// </summary>
		/// <param name="graphQLClient">The GraphQLClient</param>
		/// <param name="filePath">The Path of the File</param>
		/// <returns>The GraphQLResponse</returns>
		public static async Task<GraphQLResponse> PostFromFile(this GraphQLClient graphQLClient, string filePath) =>
			await graphQLClient.PostQueryAsync(File.ReadAllText(filePath)).ConfigureAwait(false);

	}

}
