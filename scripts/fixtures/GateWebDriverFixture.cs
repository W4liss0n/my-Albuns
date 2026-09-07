using System;
using System.IO;
using System.Net;
using System.Net.Sockets;
using System.Text;
using System.Threading;

public static class GateWebDriverFixture
{
    public static void Main(string[] arguments)
    {
        int port = int.Parse(Array.Find(arguments, value => value.StartsWith("--port=")).Substring(7));
        var listener = new TcpListener(IPAddress.Loopback, port);
        listener.Start();
        File.WriteAllText("driver-port.txt", port.ToString());
        while (true)
        {
            using (var client = listener.AcceptTcpClient())
            using (var stream = client.GetStream())
            {
                var reader = new StreamReader(stream, Encoding.UTF8);
                string request = reader.ReadLine();
                int contentLength = 0;
                string header;
                while (!string.IsNullOrEmpty(header = reader.ReadLine()))
                {
                    if (header.StartsWith("Content-Length:", StringComparison.OrdinalIgnoreCase))
                        contentLength = int.Parse(header.Substring(15).Trim());
                }
                for (int index = 0; index < contentLength; ++index) reader.Read();
                bool session = request.StartsWith("POST /session HTTP");
                if (session && File.ReadAllText("failure-mode.txt") == "session-timeout")
                    Thread.Sleep(60000);
                bool rejected = request.Contains("/timeouts ");
                string body = rejected
                    ? "{\"value\":{\"error\":\"unknown error\",\"message\":\"fixture rejected timeouts\"}}"
                    : session ? "{\"value\":{\"sessionId\":\"fixture-session\"}}" : "{\"value\":{}}";
                byte[] bytes = Encoding.UTF8.GetBytes(body);
                string response = "HTTP/1.1 " + (rejected ? "500 Error" : "200 OK") + "\r\nContent-Type: application/json\r\nContent-Length: " + bytes.Length + "\r\nConnection: close\r\n\r\n";
                byte[] headers = Encoding.ASCII.GetBytes(response);
                stream.Write(headers, 0, headers.Length);
                stream.Write(bytes, 0, bytes.Length);
            }
        }
    }
}
