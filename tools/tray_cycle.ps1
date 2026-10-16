# Hide to tray (WM_CLOSE with close-to-tray on) and restore (tray left click message), N times.
param([int]$Cycles = 5, [string]$Exe = "target\debug\JoystickGobloon.exe")
Add-Type @'
using System; using System.Runtime.InteropServices; using System.Text;
public class TC {
 public delegate bool EnumProc(IntPtr h, IntPtr l);
 [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc p, IntPtr l);
 [DllImport("user32.dll")] public static extern int GetWindowThreadProcessId(IntPtr h, out int pid);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr h, StringBuilder s, int n);
 [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
 [DllImport("user32.dll")] public static extern IntPtr SendMessageTimeout(IntPtr h, uint m, IntPtr w, IntPtr l, uint f, uint t, out IntPtr r);
 public static IntPtr main, tray;
 public static void Find(int pid) { main = IntPtr.Zero; tray = IntPtr.Zero; EnumWindows((h,l)=>{ int p; GetWindowThreadProcessId(h,out p); if(p==pid){ var t=new StringBuilder(256); GetWindowText(h,t,256); var c=new StringBuilder(256); GetClassName(h,c,256); if(t.ToString().StartsWith("Joystick Gobloon")) main=h; if(c.ToString()=="tray_icon_app") tray=h; } return true; }, IntPtr.Zero); }
 public static bool Responds(IntPtr h) { IntPtr r; return SendMessageTimeout(h, 0, IntPtr.Zero, IntPtr.Zero, 2, 2000, out r) != IntPtr.Zero; }
}
'@
$p = Start-Process -FilePath $Exe -PassThru
Start-Sleep -Seconds 5
[TC]::Find($p.Id)
"main=$([TC]::main) tray=$([TC]::tray)"
for ($i = 1; $i -le $Cycles; $i++) {
  [TC]::PostMessage([TC]::main, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null   # WM_CLOSE
  Start-Sleep -Milliseconds 800
  $hidden = -not [TC]::IsWindowVisible([TC]::main)
  [TC]::PostMessage([TC]::tray, 6002, [IntPtr]::Zero, [IntPtr]0x0202) | Out-Null     # tray left button up
  Start-Sleep -Milliseconds 800
  $shown = [TC]::IsWindowVisible([TC]::main)
  "cycle $i : hidden=$hidden shown=$shown responds=$([TC]::Responds([TC]::main))"
}
$p.Refresh()
"alive=$(-not $p.HasExited)"
if (-not $p.HasExited) { Stop-Process -Id $p.Id }
