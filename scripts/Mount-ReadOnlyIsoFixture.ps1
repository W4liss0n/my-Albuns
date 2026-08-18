param(
    [Parameter(Mandatory = $true)]
    [string] $SourceDirectory,

    [Parameter(Mandatory = $true)]
    [string] $ImagePath
)

$ErrorActionPreference = 'Stop'

Add-Type -TypeDefinition @'
using System;
using System.IO;
using System.Runtime.InteropServices;

public static class ComStreamWriter
{
    [UnmanagedFunctionPointer(CallingConvention.StdCall)]
    private delegate int ReadDelegate(
        IntPtr self,
        [Out] byte[] buffer,
        int count,
        IntPtr bytesRead);

    public static void Write(object source, string destination)
    {
        IntPtr unknown = IntPtr.Zero;
        IntPtr stream = IntPtr.Zero;
        try
        {
            unknown = Marshal.GetIUnknownForObject(source);
            Guid iid = new Guid("0000000c-0000-0000-C000-000000000046");
            int query = Marshal.QueryInterface(unknown, ref iid, out stream);
            if (query != 0)
            {
                Marshal.ThrowExceptionForHR(query);
            }

            IntPtr vtable = Marshal.ReadIntPtr(stream);
            IntPtr readAddress = Marshal.ReadIntPtr(vtable, IntPtr.Size * 3);
            var read = (ReadDelegate)Marshal.GetDelegateForFunctionPointer(
                readAddress,
                typeof(ReadDelegate));
            using (var file = File.Open(
                destination,
                FileMode.CreateNew,
                FileAccess.Write,
                FileShare.None))
            {
                var buffer = new byte[2048];
                IntPtr countPointer = Marshal.AllocHGlobal(sizeof(int));
                try
                {
                    while (true)
                    {
                        Marshal.WriteInt32(countPointer, 0);
                        int result = read(stream, buffer, buffer.Length, countPointer);
                        if (result < 0)
                        {
                            Marshal.ThrowExceptionForHR(result);
                        }
                        int count = Marshal.ReadInt32(countPointer);
                        if (count == 0)
                        {
                            break;
                        }
                        file.Write(buffer, 0, count);
                    }
                }
                finally
                {
                    Marshal.FreeHGlobal(countPointer);
                }
            }
        }
        finally
        {
            if (stream != IntPtr.Zero)
            {
                Marshal.Release(stream);
            }
            if (unknown != IntPtr.Zero)
            {
                Marshal.Release(unknown);
            }
        }
    }
}
'@

$source = [System.IO.Path]::GetFullPath($SourceDirectory)
$imagePath = [System.IO.Path]::GetFullPath($ImagePath)
if (-not [System.IO.Directory]::Exists($source)) {
    throw "The ISO source directory does not exist: $source"
}
if ([System.IO.File]::Exists($imagePath)) {
    throw "The ISO destination is already occupied: $imagePath"
}

$image = New-Object -ComObject IMAPI2FS.MsftFileSystemImage
$image.FileSystemsToCreate = 3
$image.VolumeName = 'MYALBUNS_READ_ONLY'
$image.ChooseImageDefaultsForMediaType(12)
$image.Root.AddTree($source, $false)
$result = $image.CreateResultImage()
[ComStreamWriter]::Write($result.ImageStream, $imagePath)

$mounted = Mount-DiskImage -ImagePath $imagePath -PassThru
try {
    $volume = $mounted |
        Get-Volume |
        Where-Object { $null -ne $_.DriveLetter } |
        Select-Object -First 1
    if ($null -eq $volume) {
        throw 'The mounted read-only image did not receive a drive letter.'
    }
    $driveRoot = [string]::Concat($volume.DriveLetter, ':\')
    $probePath = Join-Path $driveRoot '.myalbuns-write-probe'
    $unexpectedlyWritable = $false
    try {
        [System.IO.File]::WriteAllBytes($probePath, [byte[]]@(1))
        $unexpectedlyWritable = $true
    }
    catch {
        # The mounted optical filesystem must refuse the write.
    }
    if ($unexpectedlyWritable) {
        [System.IO.File]::Delete($probePath)
        throw 'The mounted ISO fixture unexpectedly accepted a write.'
    }
    [Console]::Out.WriteLine($driveRoot)
    [Console]::Out.Flush()
    $release = [Console]::In.ReadLine()
    if ($release -ne 'release') {
        throw 'The mounted read-only image did not receive its correlated release.'
    }
}
finally {
    Dismount-DiskImage -ImagePath $imagePath | Out-Null
}
