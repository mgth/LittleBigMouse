/*
  LittleBigMouse.Screen.Config
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.Screen.Config.

    LittleBigMouse.Screen.Config is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.Screen.Config is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using System;
using System.IO;
using System.Net;
using System.Text;
using System.Text.RegularExpressions;

namespace LittleBigMouse.DisplayLayout;

public static class HtmlHelper
{
    public static string GetPnpName1(string pnpcode)
    {
        var html = GetHtml("http://listing.driveragent.com/c/pnp/" + pnpcode);

        if (html == null) return "";

        var match = Regex.Match(html, "<span class=\"title2\">(.*?)</span>", RegexOptions.Singleline);
        if (!match.Success) return "";

        var result = match.Groups[1].Value;
        if (result.Contains("Drivers")) result = result.Replace("Drivers", "");

        var match2 = Regex.Match(result, @"\((.*?)\)", RegexOptions.Singleline);

        for (var i = 1; i < match2.Groups.Count; i++)
        {
            result = result.Replace("(" + match2.Groups[i].Value + ")", "");
        }

        result = result.Trim();

        return result;
    }

    public static string GetPnpName2(string pnpcode)
    {
        var html = GetHtml("http://www.driversdownloader.com/hardware-id/monitor/" + pnpcode.ToLower());

        if (html == null) return "";

        var match = Regex.Match(html, "<b><p>(.*?)</p></b>", RegexOptions.Singleline);
        if (!match.Success) return "";

        var result = match.Groups[1].Value;
        if (result.Contains("Drivers")) result = result.Replace("Drivers", "");

        var match2 = Regex.Match(result, @"\((.*?)\)", RegexOptions.Singleline);

        for (var i = 1; i < match2.Groups.Count; i++)
        {
            result = result.Replace("(" + match2.Groups[i].Value + ")", "");
        }

        result = result.Trim();

        return result;
    }

    public static string GetPnpName(string pnpcode)
    {
        var result = GetPnpName("https://driverlookup.com/hardware-id/monitor/", "<p><span><a href=.*?>(.*?)</a></span>", pnpcode);

        if (string.IsNullOrEmpty(result))
            result = GetPnpName("http://listing.driveragent.com/c/pnp/", "<span class=\"title2\">(.*?)</span>", pnpcode);

        if (string.IsNullOrEmpty(result))
            result = GetPnpName("http://www.driversdownloader.com/hardware-id/monitor/", "<b><p>(.*?)</p></b>", pnpcode);

        return result;
    }

    public static string CleanupPnpName(string result)
    {
        if (result.Contains("Drivers")) result = result.Replace("Drivers", "");

        var match2 = Regex.Match(result, @"\((.*?)\)", RegexOptions.Singleline);

        for (var i = 1; i < match2.Groups.Count; i++)
        {
            result = result.Replace("(" + match2.Groups[i].Value + ")", "");
        }

        return result.Trim();
    }

    public static string GetPnpName(string url, string regex, string pnpcode)
    {
        var html = GetHtml(url + pnpcode.ToLower());

        if (html == null) return "";

        var match = Regex.Match(html, regex, RegexOptions.Singleline);
        if (!match.Success) return "";

        var result = match.Groups[1].Value;

        return CleanupPnpName(result);
    }

    public static string GetHtml(string url, string post = "", string referer = "")
    {

        try
        {
            var cc = new CookieContainer();
            if (referer != null && referer != "")
            {
                /*
                                    HttpWebRequest reqReferer1 = (HttpWebRequest)HttpWebRequest.Create("http://www.realtek.com/downloads/downloadsView.aspx?Langid=1&PNid=14&PFid=24&Level=4&Conn=3");
                                    reqReferer1.CookieContainer = cc;
                                    reqReferer1.Method = "GET";
                                    reqReferer1.UserAgent = "Mozilla/5.0 (compatible; MSIE 10.0; Windows NT 6.1; Trident/6.0)";
                                    reqReferer1.GetResponse();
                */
                HttpWebRequest reqReferer = (HttpWebRequest)WebRequest.Create(referer);
                //                    reqReferer.Referer = reqReferer1.RequestUri.ToString();
                reqReferer.CookieContainer = cc;
                reqReferer.Method = "GET";
                //reqReferer.UserAgent = "Mozilla/5.0 (compatible; MSIE 10.0; Windows NT 6.1; Trident/6.0)";
                reqReferer.UserAgent = "Mozilla/5.0 (Windows NT 6.3; WOW64; rv:24.0) Gecko/20100101 Firefox/24.0";
                var responseRef = (HttpWebResponse)reqReferer.GetResponse();
                var websrcref = new StreamReader(responseRef.GetResponseStream());
                var srcRef = websrcref.ReadToEnd();
                /*
                                    Match match = Regex.Match(srcRef, "__VIEWSTATE\" value=\"(.*?)\"", RegexOptions.Singleline);
                                    if (match.Success)
                                    {
                                        ViewState = match.Groups[1].ToString();
                                    }*/
            }

            var request = (HttpWebRequest)WebRequest.Create(url);
            request.CookieContainer = cc;


            //                request.UserAgent = "Mozilla/5.0 (compatible; MSIE 10.0; Windows NT 6.1; Trident/6.0)";
            //request.UserAgent = "Mozilla/5.0 (Windows NT 6.3; WOW64; rv:24.0) Gecko/20100101 Firefox/24.0";
            request.UserAgent = "Mozilla/4.0 (compatible; MSIE 5.01; Windows NT 5.0)";
            //SetHeader(request, "User-Agent", "Mozilla/5.0 (compatible; MSIE 10.0; Windows NT 6.1; Trident/6.0)");
            request.AllowAutoRedirect = true;
            //request.PreAuthenticate = true;
            //request.Credentials = CredentialCache.DefaultCredentials;

            request.KeepAlive = true;
            request.Headers["Cache-Control"] = "max-age=0";
            //request.Headers.Add("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8");
            request.Accept = "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8";
            request.Headers["Accept-Language"] = "fr,fr-fr;q=0.8,en-us;q=0.5,en;q=0.3";
            request.AutomaticDecompression = DecompressionMethods.GZip;

            if (referer != null && referer != "")
                request.Referer = referer;
            /*                else
                                request.Referer = URL;
                            */
            if (post != "" && post != null)
            {
                request.Method = "POST";
                var array = Encoding.UTF8.GetBytes(post /*+ "&__VIEWSTATE=" + ViewState*/);
                request.ContentLength = array.Length;
                request.ContentType = "application/x-www-form-urlencoded";
                var data = request.GetRequestStream();
                data.Write(array, 0, array.Length);
                data.Close();
            }
            else
                request.Method = "GET";

            // make request for web page
            var response = (HttpWebResponse)request.GetResponse();
            var websrc = new StreamReader(response.GetResponseStream(), Encoding.GetEncoding("iso-8859-1"));
            var html = websrc.ReadToEnd();
            response.Close();

            return html;
        }
        catch (UriFormatException)
        {
            return "URL invalide";
        }
        catch (WebException ex)
        {
            return ex.Status == WebExceptionStatus.ProtocolError ? "" : "";
        }
        catch (IOException)
        {
            return
            "unavailable";
        }
        //catch (Exception ex)
        //{
        //    Status = "Unknown";
        //    Complement = ex.ToString();
        //}
        return null;
    }
}
