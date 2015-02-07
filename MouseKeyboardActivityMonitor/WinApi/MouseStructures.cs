using System;
using System.Runtime.InteropServices;

namespace MouseKeyboardActivityMonitor.WinApi
{

    /// <summary>
    /// The <see cref="MouseStruct"/> structure contains information about a mouse input event.
    /// </summary>
    /// <remarks>
    /// See full documentation at http://globalmousekeyhook.codeplex.com/wikipage?title=MouseStruct
    /// </remarks>
    [StructLayout(LayoutKind.Explicit)]
    internal struct MouseStruct
    {
        /// <summary>
        /// Specifies a Point structure that contains the X- and Y-coordinates of the cursor, in screen coordinates. 
        /// </summary>
        [FieldOffset(0x00)]
        public Point Point;

        /// <summary>
        /// Specifies information associated with the message.
        /// </summary>
        /// <remarks>
        /// The possible values are:
        /// <list type="bullet">
        /// <item>
        /// <description>0 - No Information</description>
        /// </item>
        /// <item>
        /// <description>1 - X-Button1 Click</description>
        /// </item>
        /// <item>
        /// <description>2 - X-Button2 Click</description>
        /// </item>
        /// <item>
        /// <description>120 - Mouse Scroll Away from User</description>
        /// </item>
        /// <item>
        /// <description>-120 - Mouse Scroll Toward User</description>
        /// </item>
        /// </list>
        /// </remarks>
        [FieldOffset(0x0A)]
        public Int16 MouseData;

        /// <summary>
        /// Returns a Timestamp associated with the input, in System Ticks.
        /// </summary>
        [FieldOffset(0x10)]
        public Int32 Timestamp;
    }

    /// <summary>
    /// The AppMouseStruct structure contains information about a application-level mouse input event.
    /// </summary>
    /// <remarks>
    /// See full documentation at http://globalmousekeyhook.codeplex.com/wikipage?title=MouseStruct
    /// </remarks>
    [StructLayout(LayoutKind.Explicit)]
    internal struct AppMouseStruct
    {

        /// <summary>
        /// Specifies a Point structure that contains the X- and Y-coordinates of the cursor, in screen coordinates. 
        /// </summary>
        [FieldOffset(0x00)]
        public Point Point;

        /// <summary>
        /// Specifies information associated with the message.
        /// </summary>
        /// <remarks>
        /// The possible values are:
        /// <list type="bullet">
        /// <item>
        /// <description>0 - No Information</description>
        /// </item>
        /// <item>
        /// <description>1 - X-Button1 Click</description>
        /// </item>
        /// <item>
        /// <description>2 - X-Button2 Click</description>
        /// </item>
        /// <item>
        /// <description>120 - Mouse Scroll Away from User</description>
        /// </item>
        /// <item>
        /// <description>-120 - Mouse Scroll Toward User</description>
        /// </item>
        /// </list>
        /// </remarks>
#if IS_X64
        [FieldOffset(0x22)]
#else
        [FieldOffset(0x16)]
#endif
        public Int16 MouseData;

        /// <summary>
        /// Converts the current <see cref="AppMouseStruct"/> into a <see cref="MouseStruct"/>.
        /// </summary>
        /// <returns></returns>
        /// <remarks>
        /// The AppMouseStruct does not have a timestamp, thus one is generated at the time of this call.
        /// </remarks>
        public MouseStruct ToMouseStruct()
        {
            MouseStruct tmp = new MouseStruct();
            tmp.Point = this.Point;
            tmp.MouseData = this.MouseData;
            tmp.Timestamp = Environment.TickCount;
            return tmp;
        }
    }

}
