using HLab.Sys.Windows.Monitors;
using HLab.Sys.Windows.Monitors.Factory;
using Microsoft.AspNetCore.Mvc;

namespace LittleBigMouse.Server.Controllers;

[ApiController]
[Route("[controller]")]
public class DisplayDeviceController(ILogger<DisplayDeviceController> logger) : ControllerBase
{
   readonly ILogger<DisplayDeviceController> _logger = logger;

   [HttpGet(Name = "GetDisplayDevice")]
   [Produces("application/xml")]
   public DisplayDevice Get() => MonitorDeviceHelper.GetDisplayDevices();
}