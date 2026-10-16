# Starts the app, hides it to the tray (or minimizes it), starts the exe again and checks that
# the second start exits while the first window comes back. Needs "close button hides to tray".
param([string]$Exe = "target\debug\JoystickGobloon.exe")
Add-Type @'
using System; using System.Runtime.InteropServices; using System.Text;
public class SI {
 public delegate bool EnumProc(IntPtr h, IntPtr l);
 [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc p, IntPtr l);
 [DllImport("user32.dll")] public static extern int GetWindowThreadProcessId(IntPtr h, out int pid);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
 [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
 [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
 [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
 [DllImport("user32.dll")] public static extern IntPtr SendMessageTimeout(IntPtr h, uint m, IntPtr w, IntPtr l, uint f, uint t, out IntPtr r);
 public static IntPtr Main(int pid) { IntPtr found = IntPtr.Zero; EnumWindows((h,l)=>{ int p; GetWindowThreadProcessId(h,out p); if(p==pid){ var t=new StringBuilder(256); GetWindowText(h,t,256); if(t.ToString().StartsWith("Joystick Gobloon")) found=h; } return true; }, IntPtr.Zero); return found; }
 public static bool Responds(IntPtr h) { IntPtr r; return SendMessageTimeout(h, 0, IntPtr.Zero, IntPtr.Zero, 2, 2000, out r) != IntPtr.Zero; }
}
'@
$first = Start-Process -FilePath $Exe -PassThru
Start-Sleep -Seconds 5
$h = [SI]::Main($first.Id)
foreach ($how in "tray", "minimized") {
  if ($how -eq "tray") { [SI]::PostMessage($h, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null }  # WM_CLOSE -> tray
  else { [SI]::ShowWindow($h, 6) | Out-Null }                                                          # SW_MINIMIZE
  Start-Sleep -Milliseconds 800
  $before = "visible=$([SI]::IsWindowVisible($h)) minimized=$([SI]::IsIconic($h))"
  $second = Start-Process -FilePath $Exe -PassThru
  $exited = $second.WaitForExit(5000)
  Start-Sleep -Milliseconds 800
  $after = "visible=$([SI]::IsWindowVisible($h)) minimized=$([SI]::IsIconic($h)) front=$([SI]::GetForegroundWindow() -eq $h)"
  "$how : before $before | second start exited=$exited | after $after responds=$([SI]::Responds($h))"
  if (-not $exited) { Stop-Process -Id $second.Id }
}
Stop-Process -Id $first.Id
