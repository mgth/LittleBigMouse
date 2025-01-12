using HLab.Sys.Windows.Monitors;
using HLab.Sys.Windows.Monitors.Factory;
using Microsoft.AspNetCore.Mvc;

namespace LittleBigMouse.Server.Controllers;

[ApiController]
[Route("[controller]")]
public class DisplayDeviceController : ControllerBase
{
   private readonly ILogger<DisplayDeviceController> _logger;

   public DisplayDeviceController(ILogger<DisplayDeviceController> logger)
   {
      _logger = logger;
   }

   [HttpGet(Name = "GetDisplayDevice")]
   [Produces("application/xml")]
   public DisplayDevice Get()
   {
      return MonitorDeviceHelper.GetDisplayDevices();
   }
}