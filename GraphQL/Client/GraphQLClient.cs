using System;
using System.Net.Http;
using System.Net.Http.Headers;
using System.Text;
using System.Threading.Tasks;
using GraphQL.Common.Request;
using GraphQL.Common.Response;
using Newtonsoft.Json;

namespace GraphQL.Client {

	/// <summary>
	/// A Client to access GraphQL EndPoints
	/// </summary>
	public partial class GraphQLClient : IDisposable {

		#region Properties

		/// <summary>
		/// Gets the headers which should be sent with each request.
		/// </summary>
		public HttpRequestHeaders DefaultRequestHeaders =>
			this.httpClient.DefaultRequestHeaders;

		/// <summary>
		/// The GraphQL EndPoint to be used
		/// </summary>
		public Uri EndPoint {
			get => this.Options.EndPoint;
			set => this.Options.EndPoint = value;
		}

		/// <summary>
		/// The Options	to be used
		/// </summary>
		public GraphQLClientOptions Options { get; set; }

		#endregion

		private readonly HttpClient httpClient;

		#region Constructors

		/// <summary>
		/// Initializes a new instance
		/// </summary>
		/// <param name="endPoint">The EndPoint to be used</param>
		public GraphQLClient(string endPoint) : this(new Uri(endPoint)) { }

		/// <summary>
		/// Initializes a new instance
		/// </summary>
		/// <param name="endPoint">The EndPoint to be used</param>
		public GraphQLClient(Uri endPoint) : this(new GraphQLClientOptions { EndPoint = endPoint }) { }

		/// <summary>
		/// Initializes a new instance
		/// </summary>
		/// <param name="endPoint">The EndPoint to be used</param>
		/// <param name="options">The Options to be used</param>
		public GraphQLClient(string endPoint, GraphQLClientOptions options) : this(new Uri(endPoint), options) { }

		/// <summary>
		/// Initializes a new instance
		/// </summary>
		/// <param name="endPoint">The EndPoint to be used</param>
		/// <param name="options">The Options to be used</param>
		public GraphQLClient(Uri endPoint, GraphQLClientOptions options) {
			this.Options = options ?? throw new ArgumentNullException(nameof(options));
			this.Options.EndPoint = endPoint ?? throw new ArgumentNullException(nameof(endPoint));

			if (this.Options.JsonSerializerSettings == null) { throw new ArgumentNullException(nameof(this.Options.JsonSerializerSettings)); }
			if (this.Options.HttpMessageHandler == null) { throw new ArgumentNullException(nameof(this.Options.HttpMessageHandler)); }
			if (this.Options.MediaType == null) { throw new ArgumentNullException(nameof(this.Options.MediaType)); }

			this.httpClient = new HttpClient(this.Options.HttpMessageHandler);
		}

		/// <summary>
		/// Initializes a new instance
		/// </summary>
		/// <param name="options">The Options to be used</param>
		public GraphQLClient(GraphQLClientOptions options) {
			this.Options = options ?? throw new ArgumentNullException(nameof(options));

			if (this.Options.EndPoint == null) { throw new ArgumentNullException(nameof(this.Options.EndPoint)); }
			if (this.Options.JsonSerializerSettings == null) { throw new ArgumentNullException(nameof(this.Options.JsonSerializerSettings)); }
			if (this.Options.HttpMessageHandler == null) { throw new ArgumentNullException(nameof(this.Options.HttpMessageHandler)); }
			if (this.Options.MediaType == null) { throw new ArgumentNullException(nameof(this.Options.MediaType)); }

			this.httpClient = new HttpClient(this.Options.HttpMessageHandler);
		}

		#endregion

		/// <summary>
		/// Send a query via GET
		/// </summary>
		/// <param name="query">The Request</param>
		/// <returns>The Response</returns>
		public async Task<GraphQLResponse> GetQueryAsync(string query) {
			if (query == null) { throw new ArgumentNullException(nameof(query)); }

			return await this.GetAsync(new GraphQLRequest { Query = query }).ConfigureAwait(false);
		}

		/// <summary>
		/// Send a <see cref="GraphQLRequest"/> via GET
		/// </summary>
		/// <param name="request">The Request</param>
		/// <returns>The Response</returns>
		public async Task<GraphQLResponse> GetAsync(GraphQLRequest request) {
			if (request == null) { throw new ArgumentNullException(nameof(request)); }
			if (request.Query == null) { throw new ArgumentNullException(nameof(request.Query)); }

			var queryParamsBuilder = new StringBuilder($"query={request.Query}", 3);
			if (request.OperationName != null) { queryParamsBuilder.Append($"&operationName={request.OperationName}"); }
			if (request.Variables != null) { queryParamsBuilder.Append($"&variables={JsonConvert.SerializeObject(request.Variables)}"); }
			var httpResponseMessage = await this.httpClient.GetAsync($"{this.Options.EndPoint}?{queryParamsBuilder.ToString()}").ConfigureAwait(false);
			return await this.ReadHttpResponseMessageAsync(httpResponseMessage).ConfigureAwait(false);
		}

		/// <summary>
		/// Send a query via POST
		/// </summary>
		/// <param name="query">The Request</param>
		/// <returns>The Response</returns>
		public async Task<GraphQLResponse> PostQueryAsync(string query) {
			if (query == null) { throw new ArgumentNullException(nameof(query)); }

			return await this.PostAsync(new GraphQLRequest { Query = query }).ConfigureAwait(false);
		}

		/// <summary>
		/// Send a <see cref="GraphQLRequest"/> via POST
		/// </summary>
		/// <param name="request">The Request</param>
		/// <returns>The Response</returns>
		public async Task<GraphQLResponse> PostAsync(GraphQLRequest request) {
			if (request == null) { throw new ArgumentNullException(nameof(request)); }
			if (request.Query == null) { throw new ArgumentNullException(nameof(request.Query)); }

			var graphQLString = JsonConvert.SerializeObject(request, this.Options.JsonSerializerSettings);
			var httpContent = new StringContent(graphQLString, Encoding.UTF8, this.Options.MediaType.MediaType);

            httpClient.DefaultRequestHeaders.Add("Connection", "close");

            var httpResponseMessage = await this.httpClient.PostAsync(this.EndPoint, httpContent).ConfigureAwait(false);
			return await this.ReadHttpResponseMessageAsync(httpResponseMessage).ConfigureAwait(false);
		}

		/// <summary>
		/// Releases unmanaged resources
		/// </summary>
		public void Dispose() =>
			this.httpClient.Dispose();

		/// <summary>
		/// Reads the <see cref="HttpResponseMessage"/>
		/// </summary>
		/// <param name="httpResponseMessage">The Response</param>
		/// <returns>The GrahQLResponse</returns>
		private async Task<GraphQLResponse> ReadHttpResponseMessageAsync(HttpResponseMessage httpResponseMessage) {
			var resultString = await httpResponseMessage.Content.ReadAsStringAsync().ConfigureAwait(false);
			return JsonConvert.DeserializeObject<GraphQLResponse>(resultString, this.Options.JsonSerializerSettings);
		}

	}

}
