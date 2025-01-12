using System.Text;
using ExtendedXmlSerializer;
using ExtendedXmlSerializer.Configuration;
using Microsoft.AspNetCore.Mvc.Formatters;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Net.Http.Headers;

var builder = WebApplication.CreateBuilder(args);

// Configurer les options de Kestrel pour augmenter la taille du tampon de réponse
builder.WebHost.ConfigureKestrel(options =>
{
   options.Limits.MaxResponseBufferSize = 104857600; // 100 Mo
});

builder.Services.Configure<Microsoft.AspNetCore.Http.Features.FormOptions>(options =>
{
   options.BufferBody = true;
   options.MemoryBufferThreshold = 104857600; // 100 Mo
});



// Add services to the container.

builder.Services.AddControllers(options =>
{
   options.OutputFormatters.Clear();
   //options.OutputFormatters.Insert(0, new XmlSerializerOutputFormatter());
   //options.FormatterMappings.SetMediaTypeMappingForFormat("xml", "application/xml");
   //options.FormatterMappings.SetMediaTypeMappingForFormat("default", "application/xml");
   //options.FormatterMappings.SetMediaTypeMappingForFormat("text/html", "application/xml");
   //options.OutputFormatters.Add(new ExtendedXmlSerializerOutputFormatter());
})
//.AddJsonOptions(options =>
//{
//   options.JsonSerializerOptions.NumberHandling = System.Text.Json.Serialization.JsonNumberHandling.AllowNamedFloatingPointLiterals;
//})

 .AddXmlSerializerFormatters();// Ajout du formatteur XML
;
// Learn more about configuring OpenAPI at https://aka.ms/aspnet/openapi
builder.Services.AddOpenApi();

var app = builder.Build();

// Configure the HTTP request pipeline.
if (app.Environment.IsDevelopment())
{
   app.MapOpenApi();
}

app.UseHttpsRedirection();

app.UseAuthorization();

app.MapControllers();

app.Run();


public class ExtendedXmlSerializerOutputFormatter : TextOutputFormatter
{
   private readonly IExtendedXmlSerializer _serializer;

   public ExtendedXmlSerializerOutputFormatter()
   {
      _serializer = new ConfigurationContainer().EnableReferences().Create();

      SupportedMediaTypes.Add(MediaTypeHeaderValue.Parse("application/xml"));
      SupportedMediaTypes.Add(MediaTypeHeaderValue.Parse("text/xml"));

      SupportedEncodings.Add(Encoding.UTF8);
      SupportedEncodings.Add(Encoding.Unicode);
   }

   protected override bool CanWriteType(Type type)
   {
      if (type == null)
      {
         throw new ArgumentNullException(nameof(type));
      }

      return true;
   }

   public override async Task WriteResponseBodyAsync(OutputFormatterWriteContext context, Encoding selectedEncoding)
   {
      var response = context.HttpContext.Response;
      response.ContentType = "application/xml";
      response.Headers["Content-Type"] = "application/xml; charset=" + selectedEncoding.WebName;

      using var memoryStream = new MemoryStream();
      await using var writer = new StreamWriter(memoryStream, selectedEncoding);

      _serializer.Serialize(writer, context.Object);
      await writer.FlushAsync();

      memoryStream.Position = 0;
      response.ContentLength = memoryStream.Length; // Définir la longueur du contenu
      await memoryStream.CopyToAsync(response.Body);
   }
}