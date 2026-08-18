if (-not ('Issue45OwnedProcessJob' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Diagnostics;
using System.Runtime.InteropServices;

public sealed class Issue45OwnedProcessJob : IDisposable
{
    private const uint JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE = 0x00002000;
    private const int JobObjectBasicProcessIdList = 3;
    private const int JobObjectExtendedLimitInformation = 9;
    private const int ProcessCapacity = 8192;

    [StructLayout(LayoutKind.Sequential)]
    private struct JOBOBJECT_BASIC_LIMIT_INFORMATION
    {
        public long PerProcessUserTimeLimit;
        public long PerJobUserTimeLimit;
        public uint LimitFlags;
        public UIntPtr MinimumWorkingSetSize;
        public UIntPtr MaximumWorkingSetSize;
        public uint ActiveProcessLimit;
        public UIntPtr Affinity;
        public uint PriorityClass;
        public uint SchedulingClass;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct IO_COUNTERS
    {
        public ulong ReadOperationCount;
        public ulong WriteOperationCount;
        public ulong OtherOperationCount;
        public ulong ReadTransferCount;
        public ulong WriteTransferCount;
        public ulong OtherTransferCount;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct JOBOBJECT_EXTENDED_LIMIT_INFORMATION
    {
        public JOBOBJECT_BASIC_LIMIT_INFORMATION BasicLimitInformation;
        public IO_COUNTERS IoInfo;
        public UIntPtr ProcessMemoryLimit;
        public UIntPtr JobMemoryLimit;
        public UIntPtr PeakProcessMemoryUsed;
        public UIntPtr PeakJobMemoryUsed;
    }

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern IntPtr CreateJobObject(IntPtr securityAttributes, string name);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool SetInformationJobObject(
        IntPtr job,
        int informationClass,
        ref JOBOBJECT_EXTENDED_LIMIT_INFORMATION information,
        uint informationLength);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool AssignProcessToJobObject(IntPtr job, IntPtr process);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool QueryInformationJobObject(
        IntPtr job,
        int informationClass,
        IntPtr information,
        uint informationLength,
        out uint returnLength);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool TerminateJobObject(IntPtr job, uint exitCode);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool CloseHandle(IntPtr handle);

    private IntPtr handle;

    public Issue45OwnedProcessJob()
    {
        handle = CreateJobObject(IntPtr.Zero, null);
        if (handle == IntPtr.Zero)
        {
            throw new Win32Exception(Marshal.GetLastWin32Error());
        }
        var limits = new JOBOBJECT_EXTENDED_LIMIT_INFORMATION();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if (!SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                ref limits,
                (uint)Marshal.SizeOf(typeof(JOBOBJECT_EXTENDED_LIMIT_INFORMATION))))
        {
            var error = new Win32Exception(Marshal.GetLastWin32Error());
            CloseHandle(handle);
            handle = IntPtr.Zero;
            throw error;
        }
    }

    public void Assign(Process process)
    {
        ThrowIfDisposed();
        if (!AssignProcessToJobObject(handle, process.Handle))
        {
            throw new Win32Exception(Marshal.GetLastWin32Error());
        }
    }

    public uint[] ProcessIds()
    {
        ThrowIfDisposed();
        var bytes = checked(8 + ProcessCapacity * IntPtr.Size);
        var buffer = Marshal.AllocHGlobal(bytes);
        try
        {
            uint returned;
            if (!QueryInformationJobObject(
                    handle,
                    JobObjectBasicProcessIdList,
                    buffer,
                    (uint)bytes,
                    out returned))
            {
                throw new Win32Exception(Marshal.GetLastWin32Error());
            }
            var count = Marshal.ReadInt32(buffer, 4);
            if (count < 0 || count > ProcessCapacity)
            {
                throw new InvalidOperationException("The owned Job process list exceeded its fixed safety bound.");
            }
            var processIds = new List<uint>(count);
            for (var index = 0; index < count; index++)
            {
                var offset = 8 + index * IntPtr.Size;
                var value = IntPtr.Size == 8
                    ? unchecked((ulong)Marshal.ReadInt64(buffer, offset))
                    : unchecked((uint)Marshal.ReadInt32(buffer, offset));
                if (value == 0 || value > uint.MaxValue)
                {
                    throw new InvalidOperationException("The owned Job reported an invalid process identifier.");
                }
                processIds.Add((uint)value);
            }
            return processIds.ToArray();
        }
        finally
        {
            Marshal.FreeHGlobal(buffer);
        }
    }

    public void Terminate()
    {
        ThrowIfDisposed();
        if (!TerminateJobObject(handle, 1))
        {
            throw new Win32Exception(Marshal.GetLastWin32Error());
        }
    }

    private void ThrowIfDisposed()
    {
        if (handle == IntPtr.Zero)
        {
            throw new ObjectDisposedException("Issue45OwnedProcessJob");
        }
    }

    public void Dispose()
    {
        if (handle != IntPtr.Zero)
        {
            CloseHandle(handle);
            handle = IntPtr.Zero;
        }
        GC.SuppressFinalize(this);
    }

    ~Issue45OwnedProcessJob()
    {
        Dispose();
    }
}
'@
}
